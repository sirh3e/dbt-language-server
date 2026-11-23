//! dbt.previewCte's run: prepare (blocking), then `dbt show --inline
//! <query> --limit N --output json` as a cancellable job of the registry
//! (dbt.cancel <token>, $/progress), then answer the request with the
//! table and open it as a markdown document. The answer waits for dbt;
//! the job's token is in the progress title and in dbt.doctor.

use std::sync::Arc;

use cogitator_analysis::features::{self, CtePreview};
use cogitator_error::guidance::Guidance;
use cogitator_lsp::{error_codes, message_type};

use super::client_link::ClientLink;
use super::derived_cache::DerivedCache;
use super::handlers::preview_cte::{prepare, Prepared, PreviewAsk};
use super::jobs::DbtJobs;
use super::snapshot::Snapshot;

pub struct PreviewJob {
    pub id: i64,
    pub ask: PreviewAsk,
    pub binary: String,
    pub snap: Snapshot,
    pub derived: Arc<DerivedCache>,
}

/// Answers the request exactly once: with the result, an error, or —
/// when dbt.cancel aborts the task and drops it — "cancelled".
struct Answer {
    link: ClientLink,
    id: i64,
    sent: bool,
}

impl Answer {
    fn ok(mut self, preview: &CtePreview) {
        self.sent = true;
        self.link.respond(self.id, preview);
    }

    fn fail(mut self, guidance: Guidance) {
        self.sent = true;
        self.link
            .respond_error(self.id, error_codes::REQUEST_FAILED, guidance.to_message());
    }
}

impl Drop for Answer {
    fn drop(&mut self) {
        if !self.sent {
            self.link.respond_error(
                self.id,
                error_codes::REQUEST_CANCELLED,
                "the preview was cancelled.".to_string(),
            );
        }
    }
}

pub fn spawn(link: ClientLink, jobs: Arc<DbtJobs>, job: PreviewJob) {
    let token = jobs.next_token();
    let label = format!(
        "dbt show: {}",
        features::model_name_from_uri(&job.ask.uri).unwrap_or_default()
    );
    let span = tracing::info_span!("dbt_preview_cte", token, exit = tracing::field::Empty);
    use tracing::Instrument as _;
    let handle = tokio::spawn(
        async move {
            let answer = Answer {
                link: link.clone(),
                id: job.id,
                sent: false,
            };
            let PreviewJob {
                ask,
                binary,
                snap,
                derived,
                ..
            } = job;
            let limit = ask.limit;
            let profiles = snap.profiles.clone();
            let prepared =
                tokio::task::spawn_blocking(move || prepare(&snap, &derived, &ask)).await;
            let prepared = match prepared {
                Ok(Ok(prepared)) => prepared,
                Ok(Err(guidance)) => return answer.fail(guidance),
                Err(err) => {
                    return answer.fail(
                        Guidance::new("The preview could not be prepared.")
                            .because(format!("{err}."))
                            .fix("check the server log."),
                    )
                }
            };
            let what = prepared.cte.as_deref().unwrap_or("final select");
            let progress = link.progress_begin(format!(
                "dbt show: {}.{what} [job {token}]",
                prepared.model
            ));
            let mut extra = vec![
                "--inline".to_string(),
                prepared.sql.clone(),
                "--limit".to_string(),
                (limit + 1).to_string(),
                "--output".to_string(),
                "json".to_string(),
            ];
            if let Some(vars) = profiles.dbt_vars_arg() {
                extra.extend(["--vars".to_string(), vars]);
            }
            let nonempty = |s: &String| (!s.is_empty()).then(|| s.clone());
            let args = super::commands::dbt_args(
                "show",
                None,
                nonempty(&profiles.target).as_deref(),
                nonempty(&profiles.dir_override).as_deref(),
                nonempty(&profiles.profile).as_deref(),
                &extra,
            );
            let output = tokio::process::Command::new(&binary)
                .args(&args)
                .current_dir(&prepared.project_root)
                .kill_on_drop(true)
                .output()
                .await;
            let stdout = match output {
                Err(err) => {
                    crate::metrics::record_dbt("show", "launch-failed");
                    progress.end("dbt show failed to launch");
                    return answer.fail(
                        Guidance::new(format!("Failed to launch '{binary}'."))
                            .because(format!("{err}."))
                            .fix("install dbt, or start cogitator with --fusion <path-to-dbt-binary>."),
                    );
                }
                Ok(out) if !out.status.success() => {
                    crate::metrics::record_dbt("show", "failed");
                    tracing::Span::current().record("exit", out.status.code().unwrap_or(-1));
                    progress.end(format!("dbt show failed for {}", prepared.model));
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let detail = if stderr.trim().is_empty() { stdout } else { stderr };
                    return answer.fail(
                        Guidance::new(format!(
                            "dbt show failed for '{}' (exit {}).",
                            prepared.model,
                            out.status.code().unwrap_or(-1)
                        ))
                        .because(super::commands::tail(&detail, 400))
                        .fix(format!("the query was: {}", prepared.sql)),
                    );
                }
                Ok(out) => out.stdout,
            };
            crate::metrics::record_dbt("show", "succeeded");
            progress.end(format!("dbt show succeeded for {}", prepared.model));
            match features::parse_show(&String::from_utf8_lossy(&stdout), limit) {
                Ok(table) => {
                    let preview = land(&link, prepared, table, limit).await;
                    answer.ok(&preview);
                }
                Err(guidance) => answer.fail(guidance),
            }
        }
        .instrument(span),
    );
    jobs.register(token, label, handle.abort_handle());
}

/// The answer, with its markdown written under
/// target/.cogitator/preview/ and opened; a write failure only warns.
async fn land(
    link: &ClientLink,
    prepared: Prepared,
    table: features::ShowTable,
    limit: usize,
) -> CtePreview {
    let mut preview = CtePreview {
        model: prepared.model,
        cte: prepared.cte,
        columns: table.columns,
        rows: table.rows,
        truncated: table.truncated,
        limit,
        sql: prepared.sql,
        source: prepared.source,
        uri: None,
        table: String::new(),
    };
    preview.table = features::render_markdown(&preview);
    let dir = prepared.project_root.join("target/.cogitator/preview");
    let path = dir.join(features::preview_file_name(
        &preview.model,
        preview.cte.as_deref(),
    ));
    let _ = tokio::fs::create_dir_all(&dir).await;
    match super::commands::write_atomic(&path, preview.table.as_bytes()).await {
        Ok(()) => {
            let uri = cogitator_lsp::fs_path_to_uri(&path);
            link.show_document(uri.clone(), None);
            preview.uri = Some(uri);
        }
        Err(err) => link.show_message(
            message_type::WARNING,
            Guidance::new("The preview ran but its table could not be saved.")
                .at(path.display().to_string())
                .because(format!("{err}."))
                .fix("check that the target/ directory is writable.")
                .to_message(),
        ),
    }
    preview
}

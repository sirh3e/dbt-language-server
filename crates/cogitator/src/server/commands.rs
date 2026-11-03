//! dbt CLI execution behind the code lenses and commands: per-model
//! `dbt run/test/compile` plus project-wide `dbt parse`. Every failure
//! path reports through window/showMessage with a reason and a suggested
//! solution (user-feedback requirement).

use std::path::{Path, PathBuf};

use cogitator_lsp::message_type;
use tokio::sync::mpsc;

use super::client_link::ClientLink;

pub struct DbtInvocation {
    pub kind: &'static str, // run | test | compile | parse
    pub binary: String,
    pub project_root: PathBuf,
    pub project_name: String,
    /// `--select` value; `None` runs project-wide (dbt parse).
    pub selector: Option<String>,
    pub model_path: PathBuf,
    /// Active target (dbt.setTarget / --target); appended as `--target`.
    pub target: Option<String>,
    /// Active overrides (dbt.setProfilesDir / --profiles-dir, dbt.setTarget
    /// profile / initializationOptions): the subprocess must resolve the
    /// same connection the server does.
    pub profiles_dir: Option<String>,
    pub profile: Option<String>,
    /// Context refresh to trigger on success (sender + workspace root):
    /// dbt.parse refreshes the project context, run/test/compile do not.
    pub refresh: Option<(mpsc::UnboundedSender<String>, String)>,
}

/// The dbt CLI argument list for one invocation — pure, so the exact
/// command line is provable without spawning dbt.
fn dbt_args(
    kind: &str,
    selector: Option<&str>,
    target: Option<&str>,
    profiles_dir: Option<&str>,
    profile: Option<&str>,
) -> Vec<String> {
    // "source freshness" is one kind, two argv words
    let mut args: Vec<String> = kind.split_whitespace().map(str::to_string).collect();
    if let Some(selector) = selector {
        args.push("--select".to_string());
        args.push(selector.to_string());
    }
    if let Some(target) = target {
        args.push("--target".to_string());
        args.push(target.to_string());
    }
    if let Some(dir) = profiles_dir {
        args.push("--profiles-dir".to_string());
        args.push(dir.to_string());
    }
    if let Some(profile) = profile {
        args.push("--profile".to_string());
        args.push(profile.to_string());
    }
    args
}

/// Spawn `dbt <kind> [--select <selector>]` and report the outcome; a
/// successful compile opens the compiled SQL in the editor, a successful
/// parse triggers a context refresh.
pub fn spawn_dbt(link: ClientLink, invocation: DbtInvocation) {
    let span = tracing::info_span!(
        "dbt_invocation",
        kind = invocation.kind,
        selector = invocation.selector.as_deref().unwrap_or("*"),
        exit = tracing::field::Empty,
    );
    use tracing::Instrument as _;
    tokio::spawn(
        async move {
            let DbtInvocation {
                kind,
                binary,
                project_root,
                project_name,
                selector,
                model_path,
                target,
                profiles_dir,
                profile,
                refresh,
            } = invocation;
            // what the messages call the invocation: the model selector, or the
            // project for selector-less kinds (parse)
            let label = selector.clone().unwrap_or_else(|| project_name.clone());
            let target_suffix = target
                .as_deref()
                .map(|t| format!(" (target {t})"))
                .unwrap_or_default();
            // progress-capable clients watch $/progress; others get messages
            let progress = link.progress_begin(format!("dbt {kind}: {label}{target_suffix}"));
            if !progress.active() {
                link.show_message(
                    message_type::INFO,
                    format!("dbt {kind} started for '{label}'{target_suffix}…"),
                );
            }
            let args = dbt_args(
                kind,
                selector.as_deref(),
                target.as_deref(),
                profiles_dir.as_deref(),
                profile.as_deref(),
            );
            let mut command = tokio::process::Command::new(&binary);
            command.args(&args).current_dir(&project_root);
            tracing::debug!(%binary, ?args, "dbt subprocess starting");
            let result = command.output().await;

            let output = match result {
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    crate::metrics::record_dbt(kind, "binary-missing");
                    progress.end(format!("dbt {kind} failed: '{binary}' not found"));
                    link.show_message(
                        message_type::ERROR,
                        cogitator_error::guidance::Guidance::new(format!(
                            "Failed to launch '{binary}'."
                        ))
                        .because("the executable was not found on PATH.")
                        .fix("install dbt, or start cogitator with --fusion <path-to-dbt-binary>.")
                        .to_message(),
                    );
                    return;
                }
                Err(err) => {
                    crate::metrics::record_dbt(kind, "launch-failed");
                    progress.end(format!("dbt {kind} failed to launch"));
                    link.show_message(
                        message_type::ERROR,
                        cogitator_error::guidance::Guidance::new(format!(
                            "Failed to launch '{binary}'."
                        ))
                        .because(format!("{err}."))
                        .fix(
                            "check that the binary is executable and the project \
                             directory is readable.",
                        )
                        .to_message(),
                    );
                    return;
                }
                Ok(output) => output,
            };

            tracing::Span::current().record("exit", output.status.code().unwrap_or(-1));
            if !output.status.success() {
                crate::metrics::record_dbt(kind, "failed");
                tracing::warn!(code = output.status.code(), "dbt subprocess failed");
                progress.end(format!("dbt {kind} failed for '{label}'"));
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let detail = tail(
                    if stderr.trim().is_empty() {
                        &stdout
                    } else {
                        &stderr
                    },
                    400,
                );
                let retry = format!("{binary} {}", args.join(" "));
                link.show_message(
                    message_type::ERROR,
                    cogitator_error::guidance::Guidance::new(format!(
                        "dbt {kind} failed for '{label}' (exit {}).",
                        output.status.code().unwrap_or(-1)
                    ))
                    .because(detail)
                    .fix(format!(
                        "run `{retry}` in {} for the full output.",
                        project_root.display()
                    ))
                    .to_message(),
                );
                return;
            }

            crate::metrics::record_dbt(kind, "succeeded");
            if progress.active() {
                progress.end(format!("dbt {kind} succeeded for '{label}'"));
            } else {
                link.show_message(
                    message_type::INFO,
                    format!("dbt {kind} succeeded for '{label}'."),
                );
            }

            if let Some((refresh_tx, root)) = refresh {
                let _ = refresh_tx.send(root);
            }
            if kind == "compile" {
                open_compiled(&link, &project_root, &project_name, &model_path);
            }
        }
        .instrument(span),
    );
}

/// After a successful compile: locate target/compiled/<project>/<model path>
/// and open it, or explain where we looked.
fn open_compiled(link: &ClientLink, root: &Path, project: &str, model_path: &Path) {
    let compiled = cogitator_analysis::features::compiled_sql_path(root, project, model_path);
    if compiled.is_file() {
        link.show_document(cogitator_lsp::fs_path_to_uri(&compiled), None);
    } else {
        link.show_message(
            message_type::WARNING,
            cogitator_error::guidance::Guidance::new("Compiled SQL not found.")
                .at(compiled.display().to_string())
                .because(
                    "this dbt version or your target-path setting writes compiled \
                     output elsewhere.",
                )
                .fix(
                    "check `target-path` in dbt_project.yml and look under \
                     <target-path>/compiled/.",
                )
                .to_message(),
        );
    }
}

fn tail(text: &str, max: usize) -> String {
    let trimmed = text.trim();
    if trimmed.len() <= max {
        return trimmed.to_string();
    }
    let start = trimmed.len() - max;
    let start = (start..trimmed.len())
        .find(|&i| trimmed.is_char_boundary(i))
        .unwrap_or(start);
    format!("…{}", &trimmed[start..])
}

#[cfg(test)]
mod tests {
    use super::dbt_args;

    #[test]
    fn selector_and_target_build_the_run_line() {
        assert_eq!(
            dbt_args("run", Some("customers"), Some("prod"), None, None),
            ["run", "--select", "customers", "--target", "prod"]
        );
        assert_eq!(
            dbt_args("compile", Some("staging.stg_orders"), None, None, None),
            ["compile", "--select", "staging.stg_orders"]
        );
    }

    #[test]
    fn parse_runs_project_wide_without_select() {
        assert_eq!(dbt_args("parse", None, None, None, None), ["parse"]);
        assert_eq!(
            dbt_args("parse", None, Some("dev"), None, None),
            ["parse", "--target", "dev"]
        );
    }

    #[test]
    fn active_overrides_reach_the_subprocess() {
        assert_eq!(
            dbt_args(
                "run",
                Some("customers"),
                Some("prod"),
                Some("/etc/dbt"),
                Some("jaffle_shop")
            ),
            [
                "run",
                "--select",
                "customers",
                "--target",
                "prod",
                "--profiles-dir",
                "/etc/dbt",
                "--profile",
                "jaffle_shop"
            ]
        );
    }
}

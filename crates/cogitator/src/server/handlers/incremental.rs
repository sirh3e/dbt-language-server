//! `dbt.incrementalDiff {uri, show?}`: the model rendered with
//! `is_incremental()` false and true, and their unified diff. With
//! `show: true` (the code lens) the diff is written under
//! `target/.cogitator/incremental/` and opened through showDocument,
//! so every client shows it without wrapping the command.

use cogitator_analysis::features;
use cogitator_error::guidance::Guidance;
use cogitator_lsp::{error_codes, ShowDocumentParams};

use super::super::derived_cache::DerivedCache;
use super::super::reply::{Effect, Reply};
use super::super::snapshot::Snapshot;
use super::navigation::short_uri;

#[tracing::instrument(skip_all, fields(uri = %short_uri(uri), show))]
pub fn incremental_diff(snap: &Snapshot, derived: &DerivedCache, uri: &str, show: bool) -> Reply {
    let Some(model) = features::model_name_from_uri(uri) else {
        return invalid(
            Guidance::new("dbt.incrementalDiff needs a model's .sql file.")
                .at(uri)
                .fix("pass the uri of a dbt model."),
        );
    };
    let text = match snap.doc.as_deref() {
        Some(doc) => doc.text.to_string(),
        None => {
            let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
            match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(_) => {
                    return invalid(
                        Guidance::new(format!("Could not read model '{model}'."))
                            .at(path)
                            .fix("open the model in the editor, or check the uri."),
                    )
                }
            }
        }
    };
    let mut inputs = super::commands::preview_inputs(snap, derived);
    inputs.this = inputs.model_relations.get(&model).cloned();
    let diff = match features::incremental_diff(&model, &text, &mut inputs) {
        Ok(diff) => diff,
        Err(guidance) => return invalid(guidance),
    };
    if !show {
        return Reply::result(&diff);
    }
    match write_diff(snap, &model, &diff) {
        Some(file_uri) => Reply::result(&diff).with(Effect::ShowDocument(ShowDocumentParams {
            uri: file_uri,
            take_focus: true,
            selection: None,
        })),
        None => Reply::result(&diff),
    }
}

/// The text the lens opens: the diff, or a note that nothing differs.
fn shown_text(diff: &features::IncrementalDiff) -> String {
    if diff.identical {
        format!(
            "-- {}: is_incremental() changes nothing in the rendered SQL.\n\n{}",
            diff.model, diff.full_refresh
        )
    } else {
        diff.diff.clone()
    }
}

fn write_diff(snap: &Snapshot, model: &str, diff: &features::IncrementalDiff) -> Option<String> {
    let root = snap.ctx.project_root.as_deref()?;
    let dir = root.join("target/.cogitator/incremental");
    let path = dir.join(format!("{model}.diff"));
    if let Err(err) =
        std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, shown_text(diff)))
    {
        tracing::warn!(%err, "could not write the incremental diff; returning it only");
        return None;
    }
    Some(cogitator_lsp::fs_path_to_uri(&path))
}

/// `dbt.incrementalSql {uri, strategy?, show?}`: what an incremental
/// run of the model executes for its strategy (or `strategy`), and the
/// full refresh. With `show: true` (the code action) the text is written
/// under `target/.cogitator/incremental/` and opened.
#[tracing::instrument(skip_all, fields(uri = %short_uri(uri), show))]
pub fn incremental_sql(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
    strategy: Option<&str>,
    show: bool,
) -> Reply {
    let Some(model) = features::model_name_from_uri(uri) else {
        return invalid(
            Guidance::new("dbt.incrementalSql needs a model's .sql file.")
                .at(uri)
                .fix("pass the uri of a dbt model."),
        );
    };
    let text = match snap.doc.as_deref() {
        Some(doc) => doc.text.to_string(),
        None => {
            let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
            match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(_) => {
                    return invalid(
                        Guidance::new(format!("Could not read model '{model}'."))
                            .at(path)
                            .fix("open the model in the editor, or check the uri."),
                    )
                }
            }
        }
    };
    let columns: Vec<String> = snap
        .ctx
        .model_detail_map
        .get(&model)
        .map(|d| d.columns.iter().map(|c| c.name.clone()).collect())
        .unwrap_or_default();
    let mut inputs = super::commands::preview_inputs(snap, derived);
    let answer = match features::incremental_sql(
        &model,
        &text,
        snap.ctx.dialect.as_str(),
        &columns,
        &mut inputs,
        strategy,
    ) {
        Ok(answer) => answer,
        Err(guidance) => return invalid(guidance),
    };
    if !show {
        return Reply::result(&answer);
    }
    let written = snap.ctx.project_root.as_deref().and_then(|root| {
        let dir = root.join("target/.cogitator/incremental");
        let path = dir.join(format!("{model}.{}.sql", answer.strategy.replace('+', "_")));
        match std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, &answer.text)) {
            Ok(()) => Some(cogitator_lsp::fs_path_to_uri(&path)),
            Err(err) => {
                tracing::warn!(%err, "could not write the incremental SQL; returning it only");
                None
            }
        }
    });
    match written {
        Some(file_uri) => Reply::result(&answer).with(Effect::ShowDocument(ShowDocumentParams {
            uri: file_uri,
            take_focus: true,
            selection: None,
        })),
        None => Reply::result(&answer),
    }
}

fn invalid(guidance: Guidance) -> Reply {
    Reply::error(error_codes::INVALID_PARAMS, guidance.to_message())
}

/// "Compare incremental / full refresh" on an open incremental model.
pub fn lens(
    snap: &Snapshot,
    artifacts: &cogitator_analysis::artifacts::ArtifactSet,
    uri: &str,
) -> Option<cogitator_lsp::CodeLens> {
    let doc = snap.doc.as_deref()?;
    let model = features::model_name_from_uri(uri)?;
    let manifest = artifacts
        .manifest
        .as_deref()
        .and_then(|m| m.models.get(&model))
        .and_then(|node| node.config.materialized.as_deref());
    features::incremental_lens(&snap.ctx, uri, &doc.text, manifest)
}

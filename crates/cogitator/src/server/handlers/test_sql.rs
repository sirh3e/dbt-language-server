//! `dbt.testSql {uri, show?}`: the SQL every test of the model runs,
//! rendered without dbt. With `show: true` (the code action) the text is
//! written under `target/.cogitator/tests/` and opened through
//! showDocument, so every client shows it without wrapping the command.

use cogitator_analysis::features;
use cogitator_analysis::graph::NodeKind;
use cogitator_error::guidance::Guidance;
use cogitator_lsp::{error_codes, ShowDocumentParams};

use super::super::derived_cache::DerivedCache;
use super::super::reply::{Effect, Reply};
use super::super::snapshot::Snapshot;
use super::navigation::short_uri;

#[tracing::instrument(skip_all, fields(uri = %short_uri(uri), show))]
pub fn test_sql(snap: &Snapshot, derived: &DerivedCache, uri: &str, show: bool) -> Reply {
    let Some(model) = features::model_name_from_uri(uri) else {
        return invalid(
            Guidance::new("dbt.testSql needs a model's .sql file.")
                .at(uri)
                .fix("pass the uri of a dbt model."),
        );
    };
    let mut inputs = super::commands::preview_inputs(snap, derived);
    // dbt reads generic tests from the test paths' `generic/` too, not only
    // from the macro paths the renderer's sources come from
    inputs.macro_sources.extend(generic_test_sources(snap));
    let singular = singular_files(snap, &model);
    let Some(answer) = features::test_sql(&snap.ctx, uri, &inputs, &singular) else {
        return invalid(
            Guidance::new(format!("'{model}' is not a model of the project."))
                .at(uri)
                .fix("open a model's .sql file; seeds and snapshots have no model tests here."),
        );
    };
    if !show {
        return Reply::result(&answer);
    }
    match write_text(snap, &answer.model, &answer.text) {
        Some(file_uri) => Reply::result(&answer).with(Effect::ShowDocument(ShowDocumentParams {
            uri: file_uri,
            take_focus: true,
            selection: None,
        })),
        None => Reply::result(&answer),
    }
}

/// The `.sql` files under `<test path>/generic/` of the project: dbt's
/// other home of `{% test %}` blocks.
fn generic_test_sources(snap: &Snapshot) -> Vec<String> {
    let Some(root) = snap.ctx.project_root.as_deref() else {
        return Vec::new();
    };
    let mut files: Vec<std::path::PathBuf> = snap
        .ctx
        .project
        .test_paths
        .iter()
        .map(|dir| root.join(dir).join("generic"))
        .filter(|dir| dir.is_dir())
        .flat_map(|dir| cogitator_analysis::util::walk_filepath(&dir, ".sql"))
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect()
}

/// The singular tests that ref `model`: the graph's test nodes downstream
/// of it, read from disk.
fn singular_files(snap: &Snapshot, model: &str) -> Vec<features::SingularFile> {
    let mut files: Vec<features::SingularFile> = snap
        .ctx
        .graph
        .downstream_of(model)
        .filter(|node| node.kind == NodeKind::Test)
        .filter_map(|node| {
            Some(features::SingularFile {
                text: std::fs::read_to_string(&node.uri).ok()?,
                path: node.uri.clone(),
            })
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

fn write_text(snap: &Snapshot, model: &str, text: &str) -> Option<String> {
    let root = snap.ctx.project_root.as_deref()?;
    let dir = root.join("target/.cogitator/tests");
    let path = dir.join(format!("{model}.sql"));
    if let Err(err) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, text)) {
        tracing::warn!(%err, "could not write the test SQL; returning it only");
        return None;
    }
    Some(cogitator_lsp::fs_path_to_uri(&path))
}

fn invalid(guidance: Guidance) -> Reply {
    Reply::error(error_codes::INVALID_PARAMS, guidance.to_message())
}

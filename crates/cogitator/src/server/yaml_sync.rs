//! The shell of the schema-YAML sync (`features::yaml_sync`): gathers
//! each model's typed lineage, the project's inferred types and the
//! properties files' text (an open editor buffer wins over the disk),
//! and hands the pure planner closures over them. Three callers:
//! `dbt.syncYaml`, the "Sync schema YAML" code action, and the
//! `cogitator yaml-sync` CLI ([`offline_plan`]). Offloaded callers only.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cogitator_analysis::features::column_types;
use cogitator_analysis::features::yaml_sync::{
    plan_sync, NewEntries, SyncInputs, SyncOptions, SyncPlan,
};
use cogitator_analysis::lineage::ModelLineage;
use cogitator_error::guidance::Guidance;
use cogitator_lsp::message_type;
use serde::Serialize;

use super::derived_cache::DerivedCache;
use super::handlers::lineage::TypedLineage;
use super::reply::{Effect, Reply};
use super::snapshot::Snapshot;

/// Open documents' text, by file path.
pub type OpenTexts = HashMap<PathBuf, Arc<str>>;

/// The open documents of `texts` (uri → text) keyed by path.
pub fn open_texts(texts: Vec<(String, Arc<str>)>) -> OpenTexts {
    texts
        .into_iter()
        .filter_map(|(uri, text)| Some((PathBuf::from(cogitator_lsp::uri_to_fs_path(&uri)?), text)))
        .collect()
}

/// The plan over `snap`'s project. `doc_uri` names `snap.doc`, whose
/// live text is used for that model's lineage.
pub fn plan(
    snap: &Snapshot,
    derived: &DerivedCache,
    artifact_cap: u64,
    open: &OpenTexts,
    doc_uri: &str,
    options: &SyncOptions,
) -> SyncPlan {
    let ctx = &snap.ctx;
    let typed = TypedLineage::new(snap, derived, artifact_cap);
    let lineage = |model: &str| -> Option<ModelLineage> {
        let details = ctx.model_detail_map.get(model)?;
        let uri = cogitator_lsp::fs_path_to_uri(&details.uri);
        match snap.doc.as_deref() {
            Some(doc) if uri == doc_uri => (*typed.for_open_document(doc, &uri)).clone(),
            _ => typed.for_saved_file(&uri),
        }
    };
    let schemas = ctx
        .project_root
        .as_deref()
        .and_then(|root| derived.project_schemas(root));
    let types = |model: &str| {
        let snapshot = schemas.as_deref()?;
        column_types::column_types(ctx, snapshot, model)
    };
    let read = |path: &Path| -> Option<String> {
        match open.get(path) {
            Some(text) => Some(text.to_string()), // boundary: the planner owns its copy
            None => std::fs::read_to_string(path).ok(),
        }
    };
    plan_sync(
        &SyncInputs {
            ctx,
            lineage: &lineage,
            types: &types,
            read: &read,
        },
        options,
    )
}

/// The options of a `dbt.syncYaml` argument; `uri` narrows to its model
/// (a `.sql` file) or the models of its entries (a properties file)
/// when no `models` are named.
pub fn options_of(
    snap: &Snapshot,
    uri: &str,
    arg: &serde_json::Map<String, serde_json::Value>,
) -> SyncOptions {
    let flag = |key: &str| arg.get(key).and_then(|v| v.as_bool()) == Some(true);
    let mut models: Vec<String> = arg
        .get("models")
        .and_then(|m| m.as_array())
        .map(|list| {
            list.iter()
                .filter_map(|m| m.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if models.is_empty() && !uri.is_empty() {
        models = models_of_uri(snap, uri);
    }
    let new_entries = match arg.get("newEntries").and_then(|v| v.as_str()) {
        Some("folder") => NewEntries::PerFolder,
        _ => NewEntries::PerModel,
    };
    SyncOptions {
        models,
        prune: flag("prune"),
        inherit_types: flag("inheritTypes"),
        reorder: flag("reorder"),
        new_entries,
    }
}

fn models_of_uri(snap: &Snapshot, uri: &str) -> Vec<String> {
    if let Some(model) = cogitator_analysis::features::model_name_from_uri(uri) {
        return vec![model];
    }
    let Some(path) = cogitator_lsp::uri_to_fs_path(uri).map(PathBuf::from) else {
        return Vec::new();
    };
    let mut models: Vec<String> = snap
        .ctx
        .model_detail_map
        .iter()
        .filter(|(_, d)| d.schema_uri.as_deref() == Some(path.as_path()))
        .map(|(name, _)| name.clone())
        .collect();
    models.sort();
    models
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Answer<'a> {
    #[serde(flatten)]
    plan: &'a SyncPlan,
    applied: bool,
}

/// `dbt.syncYaml`: the plan, and with `apply` its workspace edit.
#[tracing::instrument(skip_all, fields(models = plan.models.len(), apply))]
pub fn sync_yaml(snap: &Snapshot, derived: &DerivedCache, plan: &SyncPlan, apply: bool) -> Reply {
    let applied = apply && !plan.files.is_empty();
    let mut reply = Reply::result(&Answer { plan, applied });
    let schemas_ready = snap
        .ctx
        .project_root
        .as_deref()
        .is_some_and(|root| derived.project_schemas(root).is_some());
    if !schemas_ready {
        reply = reply.with(Effect::ShowMessage {
            level: message_type::WARNING,
            text: Guidance::new("The project's schemas are not computed yet.")
                .because("columns behind a `*` are only known once they are.")
                .fix("run dbt.syncYaml again in a moment.")
                .to_message(),
        });
    }
    if applied {
        reply = reply.with(Effect::ApplyEdit(cogitator_lsp::ApplyWorkspaceEditParams {
            label: "Sync schema YAML".to_string(),
            edit: plan.workspace_edit(),
        }));
    }
    reply.with(Effect::ShowMessage {
        level: message_type::INFO,
        text: summary_line(plan, applied),
    })
}

/// One line on what the plan holds, for messages and the CLI.
pub fn summary_line(plan: &SyncPlan, applied: bool) -> String {
    let count = |f: fn(&cogitator_analysis::features::yaml_sync::ModelSummary) -> usize| {
        plan.models.iter().map(f).sum::<usize>()
    };
    if plan.changes == 0 {
        return format!(
            "Schema YAML is in sync ({} model(s); {} stale column(s), {} skipped).",
            plan.models.len(),
            count(|m| m.stale.len()),
            count(|m| m.skipped.len()),
        );
    }
    format!(
        "{} {} change(s) in {} file(s): {} entr(y/ies) created, {} column(s) added, {} description(s) inherited, {} type(s) filled, {} pruned; {} stale, {} skipped.",
        if applied { "Applied" } else { "Planned" },
        plan.changes,
        plan.files.len(),
        count(|m| usize::from(m.created_entry)),
        count(|m| m.added.len()),
        count(|m| m.inherited.len()),
        count(|m| m.typed.len()),
        count(|m| m.pruned.len()),
        count(|m| m.stale.len()),
        count(|m| m.skipped.len()),
    )
}

/// The single-model sync as an edit-bearing code action, on a model's
/// SQL file or its properties entry.
pub fn code_action(
    snap: &Snapshot,
    derived: &DerivedCache,
    artifact_cap: u64,
    doc_uri: &str,
    model: &str,
) -> Option<cogitator_lsp::CodeAction> {
    let mut open = OpenTexts::new();
    if let (Some(doc), Some(path)) = (snap.doc.as_deref(), cogitator_lsp::uri_to_fs_path(doc_uri)) {
        open.insert(PathBuf::from(path), Arc::from(doc.text.as_ref()));
    }
    let options = SyncOptions {
        models: vec![model.to_string()],
        ..SyncOptions::default()
    };
    let plan = plan(snap, derived, artifact_cap, &open, doc_uri, &options);
    (plan.changes > 0).then(|| cogitator_lsp::CodeAction {
        title: format!("Sync schema YAML of '{model}' ({} changes)", plan.changes),
        kind: Some("refactor.rewrite"),
        diagnostics: None,
        edit: Some(plan.workspace_edit()),
        command: None,
        is_preferred: None,
    })
}

/// The launch flags an offline plan honours (dialect resolution).
#[derive(Debug, Default)]
pub struct Offline {
    pub profiles_dir: Option<String>,
    pub target: Option<String>,
}

/// Why `cogitator yaml-sync` could not plan.
#[derive(Debug)]
pub struct OfflineError(pub String);

/// The plan for the project at `root`, without a server: the context is
/// built, the schema job run on this thread, and every file read from
/// disk.
pub fn offline_plan(
    root: &Path,
    profiles: &Offline,
    options: &SyncOptions,
) -> Result<SyncPlan, OfflineError> {
    const ARTIFACT_CAP: u64 = 64 * 1024 * 1024;
    let root_str = root.to_str().ok_or_else(|| {
        OfflineError(
            Guidance::new(format!("'{}' is not a UTF-8 path.", root.display()))
                .fix("run from a project directory whose path is UTF-8.")
                .to_message(),
        )
    })?;
    let mut builder = cogitator_analysis::ContextBuilder::default();
    if let Some(dir) = &profiles.profiles_dir {
        builder.set_profiles_dir(dir.as_str());
    }
    if let Some(target) = &profiles.target {
        builder.set_target_override(target.as_str());
    }
    let ctx = Arc::new(builder.build(root_str));
    if ctx.project_root.is_none() {
        return Err(OfflineError(
            Guidance::new(format!("No dbt project at '{}'.", root.display()))
                .because("no dbt_project.yml was found there or above it.")
                .fix("pass --project-dir with the directory holding dbt_project.yml.")
                .to_message(),
        ));
    }
    let derived = DerivedCache::default();
    derived.publish_schemas_now(&ctx, ARTIFACT_CAP);
    let snap = Snapshot {
        ctx,
        doc: None,
        root_path: root_str.to_string(),
        profiles: Default::default(),
        fusion_path: String::new(),
        fusion_enabled: false,
        compiled_cap_bytes: 2 * 1024 * 1024,
    };
    Ok(plan(
        &snap,
        &derived,
        ARTIFACT_CAP,
        &OpenTexts::new(),
        "",
        options,
    ))
}

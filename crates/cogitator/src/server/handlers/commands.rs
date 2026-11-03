//! Data-command handlers: cached modelInfo/modelTests, compiled SQL,
//! column lineage and the cache command — pure over the snapshot plus
//! the derived cache.

use std::sync::Arc;

use cogitator_analysis::features;
use cogitator_analysis::lineage::ModelLineage;
use cogitator_lsp::message_type;

use cogitator_analysis::{DbtContext, Document};

use super::super::derived_cache::DerivedCache;
use super::super::reply::{Effect, Reply};
use super::super::snapshot::Snapshot;
use super::navigation::short_uri;

/// `dbt.modelInfo`, cached per context snapshot + input stamps.
#[tracing::instrument(skip_all, fields(uri = %short_uri(uri)))]
pub fn model_info(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
    model: Option<&str>,
) -> Reply {
    let compute = || serde_json::to_string(&features::model_info(&snap.ctx, uri, model)).ok();
    let json = match super::super::schema_fingerprint(&snap.ctx, uri, model) {
        Some(fp) => derived.result(
            Arc::as_ptr(&snap.ctx) as usize,
            &format!("modelInfo:{uri}:{}", model.unwrap_or_default()),
            fp,
            compute,
        ),
        None => compute().map(|s| Arc::from(s.as_str())),
    };
    match json {
        Some(json) => Reply::raw(&json),
        None => Reply::null(),
    }
}

/// `dbt.modelTests`, cached like modelInfo.
#[tracing::instrument(skip_all, fields(uri = %short_uri(uri)))]
pub fn model_tests(snap: &Snapshot, derived: &DerivedCache, uri: &str) -> Reply {
    let compute = || serde_json::to_string(&features::model_tests_json(&snap.ctx, uri)).ok();
    let json = match super::super::schema_fingerprint(&snap.ctx, uri, None) {
        Some(fp) => derived.result(
            Arc::as_ptr(&snap.ctx) as usize,
            &format!("modelTests:{uri}"),
            fp,
            compute,
        ),
        None => compute().map(|s| Arc::from(s.as_str())),
    };
    match json {
        Some(json) => Reply::raw(&json),
        None => Reply::null(),
    }
}

/// `dbt.compiledSql`: bounded read, warning effect past the cap.
#[tracing::instrument(skip_all, fields(uri = %short_uri(uri)))]
pub fn compiled_sql(snap: &Snapshot, uri: &str, model: Option<&str>) -> Reply {
    let (result, warning) =
        super::super::compiled::compiled_sql(&snap.ctx, uri, model, snap.compiled_cap_bytes);
    if result.is_none() {
        // unknown model: say so instead of a silent null
        let requested = model.unwrap_or_default();
        let guidance = cogitator_error::guidance::Guidance::new(format!(
            "No model '{requested}' to fetch compiled SQL for."
        ))
        .suggest(cogitator_error::guidance::closest_matches(
            requested,
            snap.ctx.model_detail_map.keys().map(String::as_str),
        ))
        .fix("pass {\"model\": <a known model>} or a model file's uri.");
        return Reply::null().with(Effect::ShowMessage {
            level: message_type::WARNING,
            text: guidance.to_message(),
        });
    }
    let mut reply = Reply::result(&result);
    if let Some(text) = warning {
        reply = reply.with(Effect::ShowMessage {
            level: message_type::WARNING,
            text,
        });
    }
    reply
}

/// Column lineage for `uri`: cached per document generation for open
/// documents; disk files (unopened) are computed per call.
pub fn lineage_for(
    derived: &DerivedCache,
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
) -> Arc<Option<ModelLineage>> {
    match doc {
        Some(doc) => derived.lineage(uri, doc.generation, || {
            cogitator_analysis::lineage::column_lineage(&doc.parsed, &ctx.dialect)
        }),
        None => {
            let compute = || {
                let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
                let text = std::fs::read_to_string(path).ok()?;
                let parsed =
                    cogitator_parser::pipeline::parse_document(&text, &ctx.dialect).ok()?;
                cogitator_analysis::lineage::column_lineage(&parsed, &ctx.dialect)
            };
            Arc::new(compute())
        }
    }
}

/// `dbt.columnLineage`: the (cached) lineage payload.
#[tracing::instrument(skip_all)]
pub fn column_lineage(lineage: &Arc<Option<ModelLineage>>) -> Reply {
    Reply::result(&lineage.as_ref())
}

/// `dbt.cache {action}`: report, or clear + refresh.
#[tracing::instrument(skip_all, fields(action = %action))]
pub fn cache(derived: &DerivedCache, action: &str) -> Reply {
    if action == "clear" {
        derived.clear();
        Reply::result(&derived.report())
            .with(Effect::Refresh)
            .with(Effect::ShowMessage {
                level: message_type::INFO,
                text: "Derived caches cleared; the project context is refreshing.".to_string(),
            })
    } else {
        Reply::result(&derived.report())
    }
}

/// Everything dbt.expandJinja's renderer needs (05 §9): macro file
/// sources (size-capped), relation strings from the active target's
/// dbt defaults, project vars and the target name.
pub fn preview_inputs(
    snap: &Snapshot,
    derived: &DerivedCache,
) -> cogitator_analysis::features::PreviewInputs {
    const MACRO_SOURCE_CAP: usize = 1024 * 1024;
    let target = super::profiles::active_target_relation(snap, derived);
    let (database, schema) = target
        .map(|t| (t.database, t.schema))
        .unwrap_or((None, None));
    let relation = |identifier: &str, schema_override: Option<&str>| {
        let mut parts: Vec<&str> = Vec::new();
        if let Some(db) = database.as_deref() {
            parts.push(db);
        }
        if let Some(s) = schema_override.or(schema.as_deref()) {
            parts.push(s)
        }
        parts.push(identifier);
        parts.join(".")
    };
    let model_relations = snap
        .ctx
        .model_detail_map
        .keys()
        .map(|name| (name.clone(), relation(name, None)))
        .collect();
    let mut source_relations = std::collections::HashMap::new();
    for source in snap.ctx.source_detail_map.values() {
        for table in source.tables.values() {
            // dbt defaults: schema falls back to the source name, the
            // identifier to the declared table name (spec §1.6)
            let schema_override = source.schema.as_deref().unwrap_or(&source.name);
            let identifier = table.identifier.as_deref().unwrap_or(&table.name);
            source_relations.insert(
                (source.name.clone(), table.name.clone()),
                relation(identifier, Some(schema_override)),
            );
        }
    }
    let mut macro_sources = Vec::new();
    let mut macro_files: Vec<&std::path::PathBuf> = snap
        .ctx
        .macro_detail_map
        .values()
        .flat_map(|package| package.values().map(|m| &m.uri))
        .collect();
    macro_files.sort();
    macro_files.dedup();
    let mut total = 0usize;
    for path in macro_files {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        total += text.len();
        if total > MACRO_SOURCE_CAP {
            tracing::debug!("macro sources over the preview cap; remainder skipped");
            break;
        }
        macro_sources.push(text);
    }
    cogitator_analysis::features::PreviewInputs {
        macro_sources,
        model_relations,
        source_relations,
        vars: snap
            .ctx
            .variable_detail_map
            .iter()
            .map(|(name, details)| (name.clone(), details.value.clone()))
            .collect(),
        target_name: if snap.profiles.target.is_empty() {
            "default".to_string()
        } else {
            snap.profiles.target.clone()
        },
    }
}

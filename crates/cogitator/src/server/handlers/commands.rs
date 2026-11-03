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
    let (result, warning) = super::super::compiled::compiled_sql(&snap.ctx, uri, model);
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

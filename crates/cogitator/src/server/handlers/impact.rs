//! The shell of `dbt.impact` and the column reach: the downstream typed
//! lineages (capped like dbt.impact), the properties YAML read from
//! disk, and the BI manifest cogitator.yml `bi.manifest` names; the pure
//! `features::impact` / `features::column_reach` do the rest. Offloaded
//! callers only.

use std::collections::HashMap;
use std::path::Path;

use cogitator_analysis::artifacts::bi_manifest::{self, BiManifest};
use cogitator_analysis::features::{self, column_reach};
use cogitator_analysis::lineage::ModelLineage;

use super::super::derived_cache::DerivedCache;
use super::super::reply::Reply;
use super::super::snapshot::Snapshot;
use super::lineage::TypedLineage;

/// The largest BI manifest read (a dashboard list, not an artifact).
const BI_MANIFEST_CAP: u64 = 16 * 1024 * 1024;

/// The downstream models' typed lineages, each read and parsed from disk.
pub fn downstream_lineages(
    typed: &TypedLineage<'_>,
    snap: &Snapshot,
    model: &str,
) -> HashMap<String, ModelLineage> {
    let mut lineages = HashMap::new();
    for (name, path) in features::impact_targets(&snap.ctx, model) {
        let uri = cogitator_lsp::fs_path_to_uri(&path);
        if let Some(lineage) = typed.for_saved_file(&uri) {
            lineages.insert(name, lineage);
        }
    }
    lineages
}

/// The BI manifest at `rel` under the project root: `Ok(None)` when none
/// is configured, `Err(note)` when it cannot be read — said in the
/// report, never silently dropped.
pub fn bi_manifest(root: Option<&Path>, rel: &str) -> Result<Option<BiManifest>, String> {
    if rel.trim().is_empty() {
        return Ok(None);
    }
    let path = match root {
        Some(root) => root.join(rel),
        None => Path::new(rel).to_path_buf(),
    };
    let too_big = std::fs::metadata(&path).map(|m| m.len() > BI_MANIFEST_CAP);
    let text = match too_big {
        Ok(false) => std::fs::read_to_string(&path).map_err(|e| e.to_string()),
        Ok(true) => Err("it is over 16 MiB".to_string()),
        Err(e) => Err(e.to_string()),
    }
    .map_err(|why| {
        format!(
            "BI manifest {} not read: {why} (cogitator.yml bi.manifest)",
            path.display()
        )
    })?;
    bi_manifest::parse(&text, &path)
        .map(Some)
        .map_err(|g| g.to_message())
}

fn disk(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// `dbt.impact {uri, model?, column?}`: the closure, and with a column
/// its reach.
pub fn impact_reply(
    snap: &Snapshot,
    derived: &DerivedCache,
    artifact_cap: u64,
    bi_path: &str,
    model: &str,
    column: Option<&str>,
) -> Reply {
    // typed lineage per downstream model, capped: each file is read and
    // parsed from disk, stars expand against the project schemas,
    // control flow only from fresh SQL
    let typed = TypedLineage::new(snap, derived, artifact_cap);
    let lineages = downstream_lineages(&typed, snap, model);
    let report = features::impact(&snap.ctx, model, column, &lineages);
    if column.is_none() {
        return Reply::result(&report);
    }
    let (bi, note) = match bi_manifest(snap.ctx.project_root.as_deref(), bi_path) {
        Ok(bi) => (bi, None),
        Err(note) => (None, Some(note)),
    };
    let corpus = column_reach::Corpus::read(&snap.ctx, &disk);
    let mut reach = column_reach::reach(&snap.ctx, &report, &corpus, bi.as_ref());
    if let (Some(reach), Some(note)) = (reach.as_mut(), note) {
        reach.notes.push(note);
    }
    Reply::result(&report.with_reach(reach))
}

/// Every output column's reach of the model at `uri`, for the hover
/// cache; `None` when its columns are unknown.
pub fn model_reach(
    snap: &Snapshot,
    derived: &DerivedCache,
    artifact_cap: u64,
    bi_path: &str,
    uri: &str,
) -> Option<column_reach::ModelReach> {
    let model = features::model_name_from_uri(uri)?;
    if !snap.ctx.model_detail_map.contains_key(&model) {
        return None;
    }
    let typed = TypedLineage::new(snap, derived, artifact_cap);
    let own = typed.for_saved_file(uri)?;
    let columns: Vec<String> = own
        .columns
        .iter()
        .filter(|c| !c.name.contains('*'))
        .map(|c| c.name.clone())
        .collect();
    let lineages = downstream_lineages(&typed, snap, &model);
    let bi = bi_manifest(snap.ctx.project_root.as_deref(), bi_path)
        .ok()
        .flatten();
    let corpus = column_reach::Corpus::read(&snap.ctx, &disk);
    Some(column_reach::model_reach(
        &snap.ctx,
        &model,
        &columns,
        &lineages,
        &corpus,
        bi.as_ref(),
    ))
}

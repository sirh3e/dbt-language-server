//! What one model's cached lineage is valid for, from stats alone: the
//! model file's stamp, the dialect, and the inferred schemas of the
//! relations it reads (a `select *` expands against them) — and, apart,
//! the compiled file and the manifest, which only a template with control
//! flow reads. Nothing here reads a file's text, so keying every model of
//! a project costs a stat each.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

use cogitator_analysis::artifacts::ArtifactPaths;
use cogitator_analysis::features::compiled_target;
use cogitator_analysis::features::plan::schemas::SchemaSnapshot;
use cogitator_analysis::graph::NodeKind;
use cogitator_analysis::DbtContext;
use serde::{Deserialize, Serialize};

use super::super::derived_cache::stamp;

/// The two halves of a model's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelKey {
    /// The file, the dialect and the upstream schemas.
    pub base: u64,
    /// The compiled file and the manifest; consulted only for an entry
    /// computed over compiled SQL.
    pub compiled: u64,
}

/// A model, its file uri and its key.
#[derive(Debug, Clone)]
pub struct Keyed {
    pub model: String,
    pub uri: String,
    pub key: ModelKey,
}

/// Every model of `ctx` with a file, keyed.
pub fn keys(ctx: &DbtContext, project: Option<&SchemaSnapshot>) -> HashMap<String, Keyed> {
    let manifest = ctx
        .project_root
        .as_deref()
        .and_then(|root| stamp(&ArtifactPaths::under(root).manifest));
    ctx.model_detail_map
        .iter()
        .filter(|(_, details)| !details.uri.as_os_str().is_empty())
        .map(|(model, details)| {
            let uri = cogitator_lsp::fs_path_to_uri(&details.uri);
            let key = ModelKey {
                base: base_key(ctx, project, model, &details.uri),
                compiled: compiled_key(ctx, &uri, manifest),
            };
            let keyed = Keyed {
                model: model.clone(),
                uri,
                key,
            };
            (model.clone(), keyed)
        })
        .collect()
}

fn base_key(ctx: &DbtContext, project: Option<&SchemaSnapshot>, model: &str, path: &Path) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    stamp(path).hash(&mut hasher);
    ctx.dialect.0.hash(&mut hasher);
    upstream_fingerprints(ctx, project, model).hash(&mut hasher);
    hasher.finish()
}

/// The schema fingerprint of each relation `model` reads, sorted; `None`
/// without a schema snapshot (lineage then binds against nothing).
fn upstream_fingerprints(
    ctx: &DbtContext,
    project: Option<&SchemaSnapshot>,
    model: &str,
) -> Option<Vec<(String, Option<u64>)>> {
    let schemas = &project?.schemas;
    let mut prints: Vec<(String, Option<u64>)> = Vec::new();
    for parent in ctx.graph.upstream_of(model) {
        match parent.kind {
            NodeKind::Model => prints.push((
                parent.name.clone(),
                schemas.model(&parent.name).map(|s| s.fingerprint),
            )),
            NodeKind::Source => prints.extend(
                schemas
                    .sources
                    .iter()
                    .filter(|((source, _), _)| **source == *parent.name)
                    .map(|((source, table), s)| (format!("{source}.{table}"), Some(s.fingerprint))),
            ),
            _ => {}
        }
    }
    prints.sort_unstable();
    Some(prints)
}

fn compiled_key(
    ctx: &DbtContext,
    uri: &str,
    manifest: Option<super::super::derived_cache::FileStamp>,
) -> u64 {
    let compiled = compiled_target(ctx, uri, None).and_then(|(_, path)| stamp(&path));
    let mut hasher = DefaultHasher::new();
    (compiled, manifest).hash(&mut hasher);
    hasher.finish()
}

/// The compiled file of `model`, as a uri, when it is on disk.
pub fn compiled_uri(ctx: &DbtContext, uri: &str) -> Option<String> {
    let (_, path) = compiled_target(ctx, uri, None)?;
    path.is_file().then(|| cogitator_lsp::fs_path_to_uri(&path))
}

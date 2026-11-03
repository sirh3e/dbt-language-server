//! workspace/diagnostic (01 §6.4): project-wide dangling refs and
//! dangling exposure targets, grouped per file. Unopened SQL files are
//! re-read through the injected reader and parsed with the phase-1
//! pipeline — the same move rename makes for downstream edits.

use std::collections::BTreeMap;
use std::path::Path;

use cogitator_lsp::Diagnostic;
use cogitator_parser::{DocumentParser, JinjaSqlParser};

use super::{citizen_diagnostics, dangling_ref_diagnostics, file_uri, DbtContext, Document};

/// Every file with project-level findings, uri-sorted. `read` is the
/// shell's filesystem seam.
pub fn project_diagnostics(
    ctx: &DbtContext,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Vec<(String, Vec<Diagnostic>)> {
    let mut out: BTreeMap<String, Vec<Diagnostic>> = BTreeMap::new();

    // dangling refs: parse each referencing file once
    let mut files: Vec<&Path> = ctx
        .graph
        .dangling_refs()
        .filter(|(from, _)| !from.uri.as_os_str().is_empty())
        .map(|(from, _)| from.uri.as_path())
        .collect();
    files.sort();
    files.dedup();
    for path in files {
        let Some(text) = read(path) else { continue };
        let Ok(parsed) = JinjaSqlParser.parse(&text, &ctx.dialect) else {
            continue;
        };
        let doc = Document {
            text: text.into(),
            parsed,
            generation: 0,
        };
        let items = dangling_ref_diagnostics(ctx, &doc);
        if !items.is_empty() {
            out.entry(file_uri(path)).or_default().extend(items);
        }
    }

    // dangling exposure/metric/semantic-model targets, per YAML file
    let mut yaml_files: Vec<&Path> = ctx
        .exposure_detail_map
        .values()
        .chain(ctx.metric_detail_map.values())
        .chain(ctx.semantic_model_detail_map.values())
        .map(|c| c.uri.as_path())
        .collect();
    yaml_files.sort();
    yaml_files.dedup();
    for path in yaml_files {
        let uri = file_uri(path);
        let items = citizen_diagnostics(ctx, &uri);
        if !items.is_empty() {
            out.entry(uri).or_default().extend(items);
        }
    }

    out.into_iter().collect()
}

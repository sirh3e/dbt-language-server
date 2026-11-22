//! The shell of the properties-YAML features: schema-driven completion
//! (offloaded, as the model's output columns come from typed lineage
//! and the generic tests from the macro and test files), and the
//! per-model snapshots the YAML-side code actions and the
//! unproduced-column diagnostic compute lineage on.

use std::sync::Arc;

use cogitator_analysis::features;
use cogitator_analysis::lineage::ModelLineage;
use cogitator_analysis::yaml_properties::{self, PropertiesInputs, TestDefinition};
use cogitator_analysis::DbtContext;
use cogitator_lsp::{Diagnostic, Position, Range};

use super::super::derived_cache::DerivedCache;
use super::super::snapshot::Snapshot;
use super::Dispatcher;

/// A model whose entry lives in the YAML file at hand, with the
/// snapshot (its own document, when open) its lineage is computed on.
pub(super) struct EntryModel {
    pub name: String,
    pub uri: String,
    pub snap: Snapshot,
}

impl Dispatcher {
    /// The models whose properties entries live in `yaml_uri`.
    pub(super) fn yaml_entry_models(&self, yaml_uri: &str) -> Vec<EntryModel> {
        let ctx = self.ctx_for(yaml_uri);
        let path = cogitator_lsp::uri_to_fs_path(yaml_uri).unwrap_or_else(|| yaml_uri.to_string());
        let path = std::path::PathBuf::from(path);
        let mut out: Vec<EntryModel> = ctx
            .model_detail_map
            .iter()
            .filter(|(_, d)| d.schema_uri.as_deref() == Some(path.as_path()))
            .filter(|(_, d)| !d.uri.as_os_str().is_empty())
            .map(|(name, d)| {
                let uri = cogitator_lsp::fs_path_to_uri(&d.uri);
                EntryModel {
                    name: name.clone(),
                    snap: self.snapshot_for(&uri),
                    uri,
                }
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// textDocument/completion in a properties file.
    pub(super) fn properties_completion(&mut self, id: i64, uri: String, pos: Position) {
        let Some(text) = self.documents.get(&uri).map(|d| d.text.clone()) else {
            self.send(&cogitator_lsp::ResultResponse::new(id, Vec::<()>::new()));
            return;
        };
        let ctx = self.ctx_for(&uri);
        let entry = features::model_entry_at(&ctx, &uri, Range::zero_width(pos))
            .map(|(name, _)| name.clone());
        let model = entry.and_then(|name| {
            self.yaml_entry_models(&uri)
                .into_iter()
                .find(|m| m.name == name)
        });
        let derived = self.derived.clone();
        let cap = self
            .limits
            .artifact_cap_bytes
            .load(std::sync::atomic::Ordering::Relaxed);
        self.offload_blocking(id, "textDocument/completion", move || {
            let tests = known_tests(&ctx);
            let (name, output, declared) = match &model {
                Some(m) => {
                    let lineage = lineage_of(m, &derived, cap);
                    let output = output_columns(lineage.as_ref().as_ref());
                    let declared = ctx
                        .model_detail_map
                        .get(&m.name)
                        .map(|d| d.columns.iter().map(|c| c.name.clone()).collect())
                        .unwrap_or_default();
                    (Some(m.name.as_str()), output, declared)
                }
                None => (None, Vec::new(), Vec::new()),
            };
            let inputs = PropertiesInputs {
                model: name,
                output_columns: &output,
                declared_columns: &declared,
                tests: &tests,
            };
            let items = yaml_properties::properties_completion(&text, pos, &inputs);
            super::super::reply::Reply::result(&items)
        });
    }
}

/// The model's typed lineage, on its own document when open.
pub(super) fn lineage_of(
    model: &EntryModel,
    derived: &DerivedCache,
    cap: u64,
) -> Arc<Option<ModelLineage>> {
    crate::server::handlers::lineage::typed_lineage_for(&model.snap, derived, &model.uri, cap)
}

/// Output column names; empty when a `*` leaves them unknown.
fn output_columns(lineage: Option<&ModelLineage>) -> Vec<String> {
    lineage
        .map(|l| {
            l.columns
                .iter()
                .map(|c| c.name.clone())
                .filter(|n| !n.contains('*'))
                .collect()
        })
        .unwrap_or_default()
}

/// The unproduced-column warnings of every entry in a YAML file.
pub(super) fn unproduced_diagnostics(
    ctx: &DbtContext,
    yaml_text: &str,
    models: &[EntryModel],
    derived: &DerivedCache,
    cap: u64,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for model in models {
        let Some(details) = ctx.model_detail_map.get(&model.name) else {
            continue;
        };
        if !details.columns.iter().any(|c| !c.tests.is_empty()) {
            continue; // nothing tested: no lineage to compute
        }
        let lineage = lineage_of(model, derived, cap);
        if let Some(lineage) = lineage.as_ref() {
            out.extend(features::unproduced_column_diagnostics(
                yaml_text,
                &model.name,
                details,
                lineage,
            ));
        }
    }
    out
}

/// Built-ins, legacy `test_*` macros and `{% test %}` blocks.
fn known_tests(ctx: &DbtContext) -> Vec<TestDefinition> {
    let mut tests = yaml_properties::builtin_tests();
    tests.extend(yaml_properties::macro_tests(ctx));
    tests.extend(
        crate::server::node_info::generic_tests(ctx)
            .into_iter()
            .map(|t| TestDefinition {
                package: package_of(&t.path),
                name: t.name,
                params: t.params,
            }),
    );
    tests
}

/// `…/dbt_packages/<package>/…` names an installed package's file.
fn package_of(path: &std::path::Path) -> Option<String> {
    let mut parts = path.components().map(|c| c.as_os_str().to_string_lossy());
    parts.find(|p| p == "dbt_packages")?;
    parts.next().map(|p| p.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_files_name_their_package() {
        let p = std::path::Path::new("/p/dbt_packages/dbt_utils/macros/generic_tests/x.sql");
        assert_eq!(package_of(p).as_deref(), Some("dbt_utils"));
        assert_eq!(package_of(std::path::Path::new("/p/macros/x.sql")), None);
    }
}

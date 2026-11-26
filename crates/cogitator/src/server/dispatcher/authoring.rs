//! The shell of the authoring actions: scaffold a staging model, inline
//! a model as a CTE, inherit a column's description. Gathers what the
//! pure builders read — typed lineage per model (the open buffer wins
//! for its own model), the inferred column types, `catalog.json` and
//! file text — and calls them. Offloaded callers only.

use std::path::Path;

use cogitator_analysis::features::{self, ScaffoldInputs};
use cogitator_analysis::lineage::ModelLineage;
use cogitator_analysis::Document;
use cogitator_lsp::{CodeAction, Range};

use crate::server::derived_cache::DerivedCache;
use crate::server::handlers::lineage::TypedLineage;
use crate::server::snapshot::Snapshot;

/// What every authoring action reads, built once per request.
pub(super) struct Authoring<'a> {
    snap: &'a Snapshot,
    derived: &'a DerivedCache,
    typed: TypedLineage<'a>,
    artifact_cap: u64,
}

impl<'a> Authoring<'a> {
    pub(super) fn new(snap: &'a Snapshot, derived: &'a DerivedCache, artifact_cap: u64) -> Self {
        Authoring {
            snap,
            derived,
            typed: TypedLineage::new(snap, derived, artifact_cap),
            artifact_cap,
        }
    }

    /// A model's typed lineage; the snapshot's document for its own uri.
    fn lineage(&self, uri: &str, model: &str) -> Option<ModelLineage> {
        let details = self.snap.ctx.model_detail_map.get(model)?;
        let model_uri = cogitator_lsp::fs_path_to_uri(&details.uri);
        match self.snap.doc.as_deref() {
            Some(doc) if model_uri == uri => (*self.typed.for_open_document(doc, uri)).clone(),
            _ => self.typed.for_saved_file(&model_uri),
        }
    }

    /// The staging scaffold at the cursor (a SQL call or a source entry).
    pub(super) fn scaffold(
        &self,
        doc: &Document,
        uri: &str,
        range: Range,
        read: &dyn Fn(&Path) -> Option<String>,
    ) -> Option<CodeAction> {
        let ctx = &self.snap.ctx;
        let root = ctx.project_root.as_deref();
        let artifacts = root.map(|r| self.derived.artifacts(r, self.artifact_cap));
        let catalog = artifacts.as_ref().and_then(|a| a.catalog.as_deref());
        let schemas = super::yaml_types::snapshot(ctx, self.derived);
        let types =
            |model: &str| features::column_types::column_types(ctx, schemas.as_deref()?, model);
        let inputs = ScaffoldInputs {
            catalog,
            types: &types,
            read,
        };
        features::staging_scaffold_action(ctx, doc, uri, range, &inputs)
    }

    /// The SQL-side actions: inline a ref'd model, inherit a description.
    pub(super) fn sql_actions(
        &self,
        doc: &Document,
        uri: &str,
        range: Range,
        read: &dyn Fn(&Path) -> Option<String>,
    ) -> Vec<CodeAction> {
        let ctx = &self.snap.ctx;
        let lineage = |model: &str| self.lineage(uri, model);
        let mut out = Vec::new();
        out.extend(self.scaffold(doc, uri, range, read));
        out.extend(features::inline_model_action(ctx, doc, uri, range, read));
        out.extend(features::inherit_description_sql_action(
            ctx, doc, uri, range, &lineage, read,
        ));
        out
    }

    /// The YAML entry's inherit actions, over lineage multi-hop.
    pub(super) fn inherit_in_yaml(
        &self,
        text: &str,
        uri: &str,
        model: &str,
        details: &cogitator_analysis::context::ModelDetails,
        read: &dyn Fn(&Path) -> Option<String>,
    ) -> Vec<CodeAction> {
        let lineage = |name: &str| self.lineage("", name);
        features::inherit_description_actions(
            &self.snap.ctx,
            text,
            uri,
            model,
            details,
            &lineage,
            read,
        )
    }
}

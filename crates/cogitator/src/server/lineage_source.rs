//! One request's column lineage, from the one cache of it.
//!
//! Every feature that reads another model's lineage asks here: the open
//! document answers for itself (unsaved edits and all), every other model
//! from the project's lineage store (`lineage_graph::Store`, keyed by the
//! file, the dialect and the upstream schemas, persisted beside the scan
//! cache), computed on a miss and kept, so the next request, of any
//! feature, computes nothing that did not change. Only per-model lineage is
//! kept; the cross-model graph (`lineage::ColumnGraph`) is built from it in
//! memory. Offloaded callers only: a miss reads and parses a file.

use std::collections::HashMap;
use std::sync::Arc;

use cogitator_analysis::lineage::ModelLineage;

use super::derived_cache::DerivedCache;
use super::handlers::lineage::TypedLineage;
use super::lineage_graph::stamps::{self, Keyed};
use super::lineage_graph::Store;
use super::snapshot::Snapshot;

pub struct LineageSource<'a> {
    snap: &'a Snapshot,
    typed: TypedLineage<'a>,
    store: Option<Arc<Store>>,
    keys: HashMap<String, Keyed>,
}

impl<'a> LineageSource<'a> {
    pub fn new(snap: &'a Snapshot, derived: &'a DerivedCache, artifact_cap: u64) -> Self {
        let typed = TypedLineage::new(snap, derived, artifact_cap);
        let (store, keys) = match snap.ctx.project_root.as_deref() {
            Some(root) => {
                let project = derived.project_schemas(root);
                let keys = stamps::keys(&snap.ctx, project.as_deref());
                let store = derived.lineage_graph.store(root);
                store.set_live(keys.keys().cloned().collect());
                (Some(store), keys)
            }
            None => (None, HashMap::new()),
        };
        LineageSource {
            snap,
            typed,
            store,
            keys,
        }
    }

    /// The lineage of `model`: the open document's when it is that model's,
    /// else the store's, computed now on a miss. `None` inside when the
    /// lineage is unknown (a parse hole, control flow without fresh SQL).
    pub fn lineage(&self, model: &str) -> Arc<Option<ModelLineage>> {
        let Some(details) = self.snap.ctx.model_detail_map.get(model) else {
            return Arc::new(None);
        };
        let uri = cogitator_lsp::fs_path_to_uri(&details.uri);
        if let Some(doc) = self.snap.doc.as_deref().filter(|d| *d.uri == *uri) {
            return self.typed.for_open_document(doc, &uri);
        }
        match (&self.store, self.keys.get(model)) {
            (Some(store), Some(keyed)) => {
                store.get_or_compute(keyed, |uri| self.typed.for_saved_file_routed(uri))
            }
            _ => Arc::new(self.typed.for_saved_file(&uri)),
        }
    }

    /// The known lineages of `models`, by name; a model whose lineage is
    /// unknown is left out.
    pub fn lineages<'m>(
        &self,
        models: impl IntoIterator<Item = &'m str>,
    ) -> HashMap<String, ModelLineage> {
        models
            .into_iter()
            .filter_map(|model| {
                let lineage = self.lineage(model);
                (*lineage).clone().map(|l| (model.to_string(), l))
            })
            .collect()
    }
}

/// What a request computed is written to the store's file when it is done
/// with its lineages (nothing, when it computed nothing).
impl Drop for LineageSource<'_> {
    fn drop(&mut self) {
        if let Some(store) = &self.store {
            store.flush();
        }
    }
}

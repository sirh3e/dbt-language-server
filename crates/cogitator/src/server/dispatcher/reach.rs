//! The column reach's dispatcher side: on open/save, every output
//! column's reach of the model (a SQL file) or of the models a
//! properties file documents, computed off this task and stored for
//! the hover, which only reads it.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use cogitator_analysis::features;
use tracing::Instrument;

use super::Dispatcher;

/// The most models one properties file recomputes per save.
const YAML_MODELS_CAP: usize = 20;

impl Dispatcher {
    pub(super) fn spawn_reach(&mut self, uri: &str) {
        let snap = self.snapshot_for(uri);
        let models: Vec<(String, String)> = if uri.ends_with(".sql") {
            features::model_name_from_uri(uri)
                .filter(|m| snap.ctx.model_detail_map.contains_key(m))
                .map(|m| vec![(m, uri.to_string())])
                .unwrap_or_default()
        } else if crate::server::diagnostics::is_yaml(uri) {
            let path = cogitator_lsp::uri_to_fs_path(uri).map(std::path::PathBuf::from);
            let mut found: Vec<(String, String)> = snap
                .ctx
                .model_detail_map
                .iter()
                .filter(|(_, d)| path.is_some() && d.schema_uri.as_deref() == path.as_deref())
                .map(|(name, d)| (name.clone(), cogitator_lsp::fs_path_to_uri(&d.uri)))
                .collect();
            found.sort();
            found.truncate(YAML_MODELS_CAP);
            found
        } else {
            Vec::new()
        };
        if models.is_empty() {
            return;
        }
        let derived = self.derived.clone();
        let cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
        let bi = self.bi_manifest_path();
        let tickets: Vec<_> = models
            .into_iter()
            .map(|(model, uri)| {
                let ticket = derived.reach_begin(&model);
                (model, uri, ticket)
            })
            .collect();
        let span = tracing::info_span!("column_reach", %uri);
        tokio::spawn(
            async move {
                let job = move || {
                    for (model, uri, ticket) in tickets {
                        let reach = crate::server::handlers::impact::model_reach(
                            &snap, &derived, cap, &bi, &uri,
                        );
                        derived.reach_finish(&model, ticket, reach.map(Arc::new));
                    }
                };
                if let Err(err) = tokio::task::spawn_blocking(job).await {
                    tracing::warn!(%err, "column-reach job failed");
                }
            }
            .instrument(span),
        );
    }
}

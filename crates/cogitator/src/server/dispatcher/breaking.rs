//! The breaking-change preview's dispatcher side: the background job
//! that computes a model's report on open/save (never per keystroke)
//! and republishes its push diagnostics when the report's findings
//! changed, and the `dbt.breakingChanges` command.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use cogitator_analysis::features;
use cogitator_lsp::{error_codes, Diagnostic};
use tracing::Instrument;

use super::Dispatcher;
use crate::server::breaking::{report, Source};

impl Dispatcher {
    /// Recompute the saved model's report off this task. Fusion owns
    /// the publishes when enabled (07 §3), so the job then only stores.
    pub(super) fn spawn_breaking(&mut self, uri: &str) {
        if !uri.ends_with(".sql") || features::model_name_from_uri(uri).is_none() {
            return;
        }
        let snap = self.snapshot_for(uri);
        if snap.doc.is_none() {
            return;
        }
        let derived = self.derived.clone();
        let cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
        let lints =
            features::lint::LintOptions::from_flags(self.toggles.lints.load(Ordering::Relaxed));
        let perf = self.perf_settings();
        let fusion = self.fusion_enabled.clone();
        let link = self.link.clone();
        let uri = uri.to_string();
        let ticket = derived.breaking_begin(&uri);
        let span = tracing::info_span!("breaking_changes", %uri);
        tokio::spawn(
            async move {
                let job = move || {
                    let fresh = report(&snap, &derived, &uri, cap, Source::Disk).map(Arc::new);
                    let new = fresh.as_deref().map(features::breaking::diagnostics);
                    let Some(previous) = derived.breaking_finish(&uri, ticket, fresh) else {
                        return; // a newer save took over
                    };
                    let old = previous.as_deref().map(features::breaking::diagnostics);
                    if same(old.as_deref(), new.as_deref()) || fusion.load(Ordering::Relaxed) {
                        return;
                    }
                    let items = crate::server::diagnostics::push_diagnostics(
                        &snap.ctx,
                        snap.doc.as_deref(),
                        &uri,
                        lints,
                        &derived,
                        &perf,
                    );
                    link.publish_diagnostics(uri, items);
                };
                if let Err(err) = tokio::task::spawn_blocking(job).await {
                    tracing::warn!(%err, "breaking-change job failed");
                }
            }
            .instrument(span),
        );
    }

    /// `dbt.breakingChanges {uri}`: the open buffer's report, computed now.
    pub(super) fn breaking_changes(&mut self, id: i64, uri: String) {
        let snap = self.snapshot_for(&uri);
        let derived = self.derived.clone();
        let cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
        self.offload_blocking(id, "dbt.breakingChanges", move || {
            match report(&snap, &derived, &uri, cap, Source::Buffer) {
                Some(report) => crate::server::reply::Reply::result(&report),
                None => crate::server::reply::Reply::error(
                    error_codes::INVALID_PARAMS,
                    cogitator_error::guidance::Guidance::new(
                        "dbt.breakingChanges needs a model file.",
                    )
                    .because("the uri is not a model of the project.")
                    .fix("call it from a model's .sql buffer: {\"uri\": \"file://…/model.sql\"}.")
                    .to_message(),
                ),
            }
        });
    }
}

/// Same findings: no republish needed.
fn same(a: Option<&[Diagnostic]>, b: Option<&[Diagnostic]>) -> bool {
    let key = |d: Option<&[Diagnostic]>| -> Vec<(i32, String)> {
        d.unwrap_or_default()
            .iter()
            .map(|d| (d.severity, d.message.clone()))
            .collect()
    };
    key(a) == key(b)
}

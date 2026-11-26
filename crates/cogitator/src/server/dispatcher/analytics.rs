//! dbt.costReport, dbt.diffAt and dbt.profileColumn: snapshot here on
//! the dispatcher task, the work off it.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use super::Dispatcher;

impl Dispatcher {
    pub(super) fn analytics_settings(&self) -> Arc<crate::server::config_file::AnalyticsSettings> {
        self.toggles
            .analytics
            .read()
            .expect("analytics settings lock")
            .clone()
    }

    /// `dbt.costReport {}`: project-wide.
    pub(super) fn cost_report(&mut self, id: i64) {
        let snap = self.snapshot_for("");
        let derived = self.derived.clone();
        let cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
        let settings = self.analytics_settings();
        self.offload_blocking(id, "dbt.costReport", move || {
            crate::server::handlers::cost::cost_report(&snap, &derived, cap, &settings)
        });
    }
}

impl Dispatcher {
    /// `dbt.diffAt {uri, rev?, what?, show?}`.
    pub(super) fn diff_at(
        &mut self,
        id: i64,
        uri: String,
        arg: &serde_json::Map<String, serde_json::Value>,
    ) {
        let text = |key: &str| arg.get(key).and_then(|v| v.as_str()).map(str::to_string);
        let ask = crate::server::handlers::diff_at::DiffAsk {
            rev: text("rev").filter(|r| !r.trim().is_empty()),
            what: text("what"),
            show: arg.get("show").and_then(|s| s.as_bool()) == Some(true),
            uri: uri.clone(),
        };
        let snap = self.snapshot_for(&uri);
        let derived = self.derived.clone();
        let cap = self.limits.artifact_cap_bytes.load(Ordering::Relaxed);
        self.offload_blocking(id, "dbt.diffAt", move || {
            crate::server::handlers::diff_at::diff_at(&snap, &derived, &ask, cap)
        });
    }
}

impl Dispatcher {
    /// `dbt.profileColumn {uri, position?, column?}`: a background job.
    pub(super) fn profile_column(
        &mut self,
        id: i64,
        uri: String,
        arg: &serde_json::Map<String, serde_json::Value>,
    ) {
        let binary = if self.fusion_path.is_empty() {
            "dbt".to_string()
        } else {
            self.fusion_path.clone()
        };
        let job = crate::server::profile::ProfileJob {
            id,
            position: super::arg_position(arg),
            column: arg
                .get("column")
                .and_then(|c| c.as_str())
                .map(str::to_string),
            binary,
            snap: self.snapshot_for(&uri),
            derived: self.derived.clone(),
            settings: self.analytics_settings(),
            artifact_cap: self.limits.artifact_cap_bytes.load(Ordering::Relaxed),
            uri,
        };
        crate::server::profile::spawn(self.link.clone(), self.jobs.clone(), job);
    }
}

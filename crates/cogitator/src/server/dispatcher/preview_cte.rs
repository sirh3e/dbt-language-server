//! dbt.previewCte's entry: read the arguments and the limits, then hand
//! the work to `server::preview_cte`, which answers the request itself
//! once dbt is done.

use std::sync::atomic::Ordering;

use super::Dispatcher;

impl Dispatcher {
    pub(super) fn preview_cte(
        &mut self,
        id: i64,
        uri: String,
        arg: &serde_json::Map<String, serde_json::Value>,
    ) {
        let default = self.limits.preview_rows.load(Ordering::Relaxed);
        let max = self.limits.preview_max_rows.load(Ordering::Relaxed).max(1);
        let limit = arg
            .get("limit")
            .and_then(|l| l.as_u64())
            .filter(|&l| l > 0)
            .unwrap_or(default)
            .clamp(1, max) as usize;
        let ask = crate::server::handlers::preview_cte::PreviewAsk {
            cte: arg.get("cte").and_then(|c| c.as_str()).map(str::to_string),
            position: super::arg_position(arg),
            uri: uri.clone(),
            limit,
        };
        let binary = if self.fusion_path.is_empty() {
            "dbt".to_string()
        } else {
            self.fusion_path.clone()
        };
        let job = crate::server::preview_cte::PreviewJob {
            id,
            ask,
            binary,
            snap: self.snapshot_for(&uri),
            derived: self.derived.clone(),
        };
        crate::server::preview_cte::spawn(self.link.clone(), self.jobs.clone(), job);
    }
}

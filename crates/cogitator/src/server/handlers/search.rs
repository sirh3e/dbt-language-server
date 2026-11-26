//! The shell of `dbt.search {query, uri?}`: the structural query over
//! the models' HIR (`features::hir_search`), each file read from disk —
//! the open buffer's text for the model the call came from. Offloaded.

use cogitator_analysis::features::{self, hir_search};
use cogitator_lsp::error_codes;
use serde_json::{Map, Value};

use super::super::reply::Reply;
use super::super::snapshot::Snapshot;

pub fn search_reply(snap: &Snapshot, arg: &Map<String, Value>, uri: &str) -> Reply {
    let Some(query) = arg
        .get("query")
        .and_then(Value::as_str)
        .filter(|q| !q.trim().is_empty())
    else {
        return Reply::error(
            error_codes::INVALID_PARAMS,
            cogitator_error::guidance::Guidance::new("dbt.search needs a query.")
                .fix("pass {\"query\": \"joins stg_orders on customer_id\"} (docs/search.md).")
                .to_message(),
        );
    };
    // a model's uri narrows the search to it; "" (or another file) is the project
    let only =
        features::model_name_from_uri(uri).filter(|m| snap.ctx.model_detail_map.contains_key(m));
    let open = snap.doc.as_deref().and_then(|doc| {
        let path = cogitator_lsp::uri_to_fs_path(uri)?;
        Some((std::path::PathBuf::from(path), doc.text.to_string()))
    });
    let read = |path: &std::path::Path| match &open {
        Some((p, text)) if p == path => Some(text.clone()),
        _ => std::fs::read_to_string(path).ok(),
    };
    match hir_search::search(&snap.ctx, query, only.as_deref(), &read) {
        Ok(report) => Reply::result(&report),
        Err(guidance) => Reply::error(error_codes::INVALID_PARAMS, guidance.to_message()),
    }
}

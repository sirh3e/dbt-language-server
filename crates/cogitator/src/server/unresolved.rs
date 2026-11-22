//! `dbt.unresolved`: every unresolved ref, source, source table, doc,
//! macro and var of the project (`features::project_unresolved`), reading
//! open documents' unsaved text first and the files on disk otherwise.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cogitator_analysis::features::{self, Unresolved};
use cogitator_analysis::DbtContext;
use serde_json::{json, Map, Value};

use crate::server::reply::Reply;

/// Cap on the items one answer carries; `truncated` says when it hit.
pub const MAX_ITEMS: usize = 5_000;

/// A `dbt.unresolved` request: `{kinds?: ["ref", "macro", …]}`.
#[derive(Debug, Clone, Default)]
pub struct Request {
    pub kinds: Option<Vec<String>>,
}

impl Request {
    pub fn of(arg: &Map<String, Value>) -> Self {
        let kinds = arg.get("kinds").and_then(|k| k.as_array()).map(|ks| {
            ks.iter()
                .filter_map(|k| k.as_str().map(str::to_string))
                .collect()
        });
        Self { kinds }
    }
}

fn kind_name(u: &Unresolved) -> String {
    serde_json::to_value(u.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// `dbt.unresolved`.
pub fn reply(ctx: &DbtContext, open: Vec<(String, Arc<str>)>, request: &Request) -> Reply {
    let open: HashMap<PathBuf, Arc<str>> = open
        .into_iter()
        .filter_map(|(uri, text)| Some((PathBuf::from(cogitator_lsp::uri_to_fs_path(&uri)?), text)))
        .collect();
    let text = |path: &Path| {
        open.get(path)
            .cloned()
            .or_else(|| std::fs::read_to_string(path).ok().map(Arc::from))
    };
    let mut items = features::project_unresolved(ctx, &text);
    if let Some(kinds) = &request.kinds {
        items.retain(|u| kinds.contains(&kind_name(u)));
    }
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for u in &items {
        *by_kind.entry(kind_name(u)).or_default() += 1;
    }
    let count = items.len();
    let truncated = count > MAX_ITEMS;
    items.truncate(MAX_ITEMS);
    Reply::result(&json!({
        "items": items,
        "count": count,
        "byKind": by_kind,
        "truncated": truncated,
    }))
}

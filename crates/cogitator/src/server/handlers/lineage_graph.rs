//! `dbt.columnLineageGraph`: the column lineage around a model column,
//! across the project, in one answer. The lineages come from the
//! project-wide cache (`server/lineage_graph`); what it lacks is queued
//! for its workers, waited for up to a budget, and named `stale` in the
//! answer when the budget ran out. Then the rest of the project is queued
//! behind, so the next walk anywhere finds it computed. Offloaded.
//!
//! With `show: true` (the code action on a column, for clients that
//! cannot wrap a command, as Zed) the walk is also written as text under
//! `target/.cogitator/lineage/` and opened through showDocument.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cogitator_analysis::features::lineage_graph::{
    text, walk, Direction, Known, LineageEdge, LineageNode, WalkInput,
};
use cogitator_analysis::features::{model_name_from_uri, project_graph_json};
use cogitator_analysis::graph::NodeKind;
use cogitator_analysis::DbtContext;
use cogitator_error::guidance::Guidance;
use cogitator_lsp::{error_codes, ShowDocumentParams};
use serde::Serialize;

use super::super::derived_cache::DerivedCache;
use super::super::lineage_graph::stamps::{self, Keyed};
use super::super::lineage_graph::Compute;
use super::super::reply::{Effect, Reply};
use super::super::snapshot::Snapshot;
use super::lineage::TypedLineage;

/// How long a request waits for lineages still being computed.
pub const DEFAULT_BUDGET: Duration = Duration::from_secs(10);

/// The command's argument, decoded.
pub struct Request {
    pub column: Option<String>,
    pub direction: Direction,
    pub depth: usize,
    pub budget: Duration,
    /// Write the walk as text and open it (the code action).
    pub show: bool,
}

impl Request {
    pub fn from_args(arg: &serde_json::Map<String, serde_json::Value>) -> Result<Self, String> {
        let direction = match arg.get("direction").and_then(|d| d.as_str()) {
            None => Direction::Both,
            Some(word) => Direction::parse(word).ok_or_else(|| {
                Guidance::new(format!("'{word}' is no lineage direction."))
                    .fix("send direction \"up\", \"down\" or \"both\".")
                    .to_message()
            })?,
        };
        Ok(Request {
            column: arg
                .get("column")
                .and_then(|c| c.as_str())
                .filter(|c| !c.is_empty())
                .map(str::to_string),
            direction,
            depth: arg
                .get("depth")
                .and_then(|d| d.as_u64())
                .map_or(3, |d| d.clamp(1, 64) as usize),
            budget: arg
                .get("budgetMs")
                .and_then(|b| b.as_u64())
                .map_or(DEFAULT_BUDGET, Duration::from_millis),
            show: arg.get("show").and_then(|s| s.as_bool()) == Some(true),
        })
    }
}

#[derive(Serialize)]
struct Answer {
    nodes: Vec<LineageNode>,
    edges: Vec<LineageEdge>,
    start: Option<String>,
    /// Every lineage the walk read was already computed.
    cached: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    stale: Vec<String>,
    missing: Vec<String>,
    truncated: bool,
}

/// The answer for `uri`'s model.
pub fn command(
    snap: Snapshot,
    derived: Arc<DerivedCache>,
    artifact_cap: u64,
    uri: &str,
    request: Request,
) -> Reply {
    let ctx = snap.ctx.clone();
    let (Some(root), Some(model)) = (ctx.project_root.clone(), model_name_from_uri(uri)) else {
        return Reply::null();
    };
    if !ctx.model_detail_map.contains_key(&model) {
        return Reply::error(
            error_codes::INVALID_PARAMS,
            Guidance::new(format!("'{model}' is no model of this project."))
                .fix("run the command from a model's SQL file.")
                .to_message(),
        );
    }
    let project = derived.project_schemas(&root);
    let keys = stamps::keys(&ctx, project.as_deref());
    let store = derived.lineage_graph.store(&root);
    store.set_live(keys.keys().cloned().collect());
    let compute = computer(&snap, &derived, artifact_cap);
    let deadline = Instant::now() + request.budget;

    let reach = reachable(&ctx, &model, request.direction, request.depth);
    let wanted = reach.iter().filter_map(|m| keys.get(m).cloned());
    let waiting = store.ensure(wanted, &compute, true);
    let mut cached = waiting.is_empty();
    store.wait(&waiting, deadline);

    // the open document answers for itself, unsaved edits and all
    let open = snap
        .doc
        .as_deref()
        .map(|doc| TypedLineage::new(&snap, &derived, artifact_cap).for_open_document(doc, uri));
    let pg = project_graph_json(&ctx);
    let compiled = |m: &str| keys.get(m).and_then(|k| stamps::compiled_uri(&ctx, &k.uri));
    let input = WalkInput {
        project: &pg,
        model: &model,
        column: request.column.as_deref(),
        direction: request.direction,
        depth: request.depth,
        compiled_uri: &compiled,
    };
    let lookup = |m: &str| -> Known {
        if m == model {
            if let Some(open) = &open {
                return Known::Lineage(open.clone());
            }
        }
        match keys.get(m) {
            Some(k) => store
                .valid(m, &k.key)
                .map_or(Known::Pending, Known::Lineage),
            None => Known::Lineage(Arc::new(None)),
        }
    };
    let mut result = walk(&input, &lookup);
    // a lineage can name a model the graph did not reach: one more round
    while !result.stale.is_empty() && Instant::now() < deadline {
        cached = false;
        let more: Vec<Keyed> = result
            .stale
            .iter()
            .filter_map(|m| keys.get(m).cloned())
            .collect();
        let waiting = store.ensure(more, &compute, true);
        if !store.wait(&waiting, deadline).is_empty() {
            break;
        }
        result = walk(&input, &lookup);
    }
    // the rest of the project, behind any request's models
    let rest = keys
        .values()
        .filter(|k| k.uri.ends_with(".sql"))
        .cloned()
        .collect::<Vec<_>>();
    store.ensure(rest, &compute, false);

    let shown = request
        .show
        .then(|| {
            let heading = text::Heading {
                model: &model,
                column: request.column.as_deref(),
                direction: request.direction,
                depth: request.depth,
            };
            write_text(&root, &heading, &result)
        })
        .flatten();
    let reply = Reply::result(&Answer {
        nodes: result.nodes,
        edges: result.edges,
        start: result.start,
        cached,
        stale: result.stale,
        missing: result.missing,
        truncated: false,
    });
    match shown {
        Some(uri) => reply.with(Effect::ShowDocument(ShowDocumentParams {
            uri,
            take_focus: true,
            selection: None,
        })),
        None => reply,
    }
}

/// The walk as text under `target/.cogitator/lineage/`; its uri, or
/// `None` (logged) when it cannot be written — the answer still goes out.
fn write_text(
    root: &std::path::Path,
    heading: &text::Heading<'_>,
    result: &cogitator_analysis::features::lineage_graph::WalkResult,
) -> Option<String> {
    let dir = root.join("target/.cogitator/lineage");
    let path = dir.join(text::file_name(heading.model, heading.column));
    let body = text::render(heading, result, Some(root));
    if let Err(err) = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, body)) {
        tracing::warn!(%err, "could not write the column lineage text; returning it only");
        return None;
    }
    Some(cogitator_lsp::fs_path_to_uri(&path))
}

/// What a worker computes a model with: this request's context, read
/// from the saved file (the open document is answered apart).
fn computer(snap: &Snapshot, derived: &Arc<DerivedCache>, artifact_cap: u64) -> Compute {
    let snap = Snapshot {
        ctx: snap.ctx.clone(),
        doc: None,
        root_path: snap.root_path.clone(),
        profiles: snap.profiles.clone(),
        fusion_path: snap.fusion_path.clone(),
        fusion_enabled: snap.fusion_enabled,
        compiled_cap_bytes: snap.compiled_cap_bytes,
    };
    let derived = derived.clone();
    Arc::new(move |uri: &str| {
        TypedLineage::new(&snap, &derived, artifact_cap).for_saved_file_routed(uri)
    })
}

/// The models within `depth` model-graph hops of `model` in `direction`:
/// a superset of what the column walk can visit.
fn reachable(ctx: &DbtContext, model: &str, direction: Direction, depth: usize) -> Vec<String> {
    let mut found = vec![model.to_string()];
    let mut seen: HashSet<String> = found.iter().cloned().collect();
    let ups = direction != Direction::Down;
    let downs = direction != Direction::Up;
    for (go, up) in [(ups, true), (downs, false)] {
        if !go {
            continue;
        }
        let mut queue = VecDeque::from([(model.to_string(), 0usize)]);
        let mut level: HashMap<String, usize> = HashMap::new();
        while let Some((name, hops)) = queue.pop_front() {
            if hops >= depth {
                continue;
            }
            let next: Vec<String> = if up {
                ctx.graph
                    .upstream_of(&name)
                    .filter(|n| n.kind == NodeKind::Model)
                    .map(|n| n.name.clone())
                    .collect()
            } else {
                ctx.graph
                    .downstream_of(&name)
                    .map(|n| n.name.clone())
                    .collect()
            };
            for n in next {
                if level.insert(n.clone(), hops + 1).is_none() {
                    if seen.insert(n.clone()) {
                        found.push(n.clone());
                    }
                    queue.push_back((n, hops + 1));
                }
            }
        }
    }
    found
}

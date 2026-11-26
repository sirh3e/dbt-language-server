//! A column-lineage walk as plain text, for the file the
//! `dbt.columnLineageGraph {show: true}` code action opens: the columns
//! layer by layer as the DAG lays them out (sources first), the start
//! marked, each with its file relative to the project; then the edges,
//! and what the walk could not read. Pure: paths are cut against `root`,
//! nothing is read.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::path::Path;

use super::{Direction, WalkResult};

/// What the walk was asked, for the heading.
pub struct Heading<'a> {
    pub model: &'a str,
    pub column: Option<&'a str>,
    pub direction: Direction,
    pub depth: usize,
}

/// The file name the text is written under (no directory).
pub fn file_name(model: &str, column: Option<&str>) -> String {
    let safe = |s: &str| -> String {
        s.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
    };
    match column {
        Some(column) => format!("{}.{}.lineage.txt", safe(model), safe(column)),
        None => format!("{}.lineage.txt", safe(model)),
    }
}

/// The walk as text; `root` is the project root that file paths are
/// shown relative to.
pub fn render(heading: &Heading<'_>, result: &WalkResult, root: Option<&Path>) -> String {
    let mut out = String::new();
    let subject = match heading.column {
        Some(column) => format!("{}.{column}", heading.model),
        None => format!("every column of {}", heading.model),
    };
    let way = match heading.direction {
        Direction::Up => "upstream",
        Direction::Down => "downstream",
        Direction::Both => "up- and downstream",
    };
    let hops = if heading.depth == 1 { "hop" } else { "hops" };
    let _ = writeln!(out, "Column lineage of {subject}");
    let _ = writeln!(
        out,
        "{way}, {} {hops} · {} columns, {} edges",
        heading.depth,
        result.nodes.len(),
        result.edges.len()
    );
    if result.nodes.is_empty() {
        let _ = writeln!(out, "\nNo lineage found.");
    }
    let mut layers: BTreeMap<usize, Vec<&super::LineageNode>> = BTreeMap::new();
    for node in &result.nodes {
        layers.entry(node.layer).or_default().push(node);
    }
    let width = result.nodes.iter().map(|n| n.name.len()).max().unwrap_or(0);
    for (layer, nodes) in &layers {
        let _ = writeln!(out, "\nlayer {layer}");
        for node in nodes {
            let mark = if result.start.as_deref() == Some(node.id.as_str()) {
                "▶"
            } else {
                " "
            };
            let place = node
                .uri
                .as_deref()
                .map(|uri| relative(uri, root))
                .unwrap_or_default();
            let line = format!("  {mark} {:<6} {:<width$}  {place}", node.kind, node.name);
            let _ = writeln!(out, "{}", line.trim_end());
        }
    }
    let names: HashMap<&str, &str> = result
        .nodes
        .iter()
        .map(|n| (n.id.as_str(), n.name.as_str()))
        .collect();
    if !result.edges.is_empty() {
        let _ = writeln!(out, "\nedges");
        for edge in &result.edges {
            let name = |id: &str| names.get(id).copied().unwrap_or(id).to_string();
            let _ = writeln!(
                out,
                "  {} → {}  ({})",
                name(&edge.from),
                name(&edge.to),
                edge.kind
            );
        }
    }
    if !result.stale.is_empty() {
        let _ = writeln!(
            out,
            "\nstill being computed (run the action again): {}",
            result.stale.join(", ")
        );
    }
    if !result.missing.is_empty() {
        let _ = writeln!(out, "\nno lineage read for: {}", result.missing.join(", "));
    }
    out
}

/// A file uri as a path under `root`, else the path as it is.
fn relative(uri: &str, root: Option<&Path>) -> String {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let path = Path::new(&path);
    root.and_then(|r| path.strip_prefix(r).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::super::{LineageEdge, LineageNode};
    use super::*;

    fn node(
        id: &str,
        name: &str,
        kind: &'static str,
        layer: usize,
        uri: Option<&str>,
    ) -> LineageNode {
        LineageNode {
            id: id.into(),
            name: name.into(),
            kind,
            uri: uri.map(str::to_string),
            tags: Vec::new(),
            columns: Vec::new(),
            model: None,
            column: String::new(),
            layer,
            compiled_uri: None,
        }
    }

    #[test]
    fn layers_edges_and_the_start() {
        let result = WalkResult {
            nodes: vec![
                node(
                    "model:orders::id",
                    "orders.id",
                    "model",
                    1,
                    Some("file:///p/models/orders.sql"),
                ),
                node("source:s.t::id", "s.t.id", "source", 0, None),
            ],
            edges: vec![LineageEdge {
                from: "source:s.t::id".into(),
                to: "model:orders::id".into(),
                kind: "direct",
            }],
            start: Some("model:orders::id".into()),
            stale: vec!["later".into()],
            ..Default::default()
        };
        let heading = Heading {
            model: "orders",
            column: Some("id"),
            direction: Direction::Both,
            depth: 3,
        };
        let text = render(&heading, &result, Some(Path::new("/p")));
        let expected = "\
Column lineage of orders.id
up- and downstream, 3 hops · 2 columns, 1 edges

layer 0
    source s.t.id

layer 1
  ▶ model  orders.id  models/orders.sql

edges
  s.t.id → orders.id  (direct)

still being computed (run the action again): later
";
        assert_eq!(text, expected);
    }

    #[test]
    fn nothing_found_and_file_names() {
        let heading = Heading {
            model: "m",
            column: None,
            direction: Direction::Up,
            depth: 1,
        };
        let text = render(&heading, &WalkResult::default(), None);
        assert!(
            text.starts_with("Column lineage of every column of m\nupstream, 1 hop"),
            "{text}"
        );
        assert!(text.contains("No lineage found."));
        assert_eq!(file_name("m", Some("a b")), "m.a_b.lineage.txt");
        assert_eq!(file_name("m", None), "m.lineage.txt");
    }
}

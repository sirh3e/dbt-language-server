//! Where every node's box goes: `layout.lua` in Rust. Layers become
//! columns left to right; within a column nodes order by the mean centre
//! of their already-placed parents, names breaking ties (the first
//! column keeps the graph's order). Coordinates are 1-based grid cells.

use std::collections::HashMap;

use super::canvas::GUTTER;
use super::{Dag, DagNode};

/// Columns an expanded box lists before "… +N more".
pub const MAX_COLUMNS: usize = 8;

/// A placed box.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub id: String,
    pub lines: Vec<String>,
    /// Outer width in cells.
    pub w: i64,
    /// Outer height in rows.
    pub h: i64,
    /// Left edge (1-based column).
    pub x: i64,
    /// Top edge (1-based row).
    pub y: i64,
    /// Vertical centre, where edges attach.
    pub cy: i64,
    /// 1-based layer.
    pub layer: usize,
}

fn width_of(text: &str) -> i64 {
    text.chars().count() as i64
}

/// A node's box content: the title (with a `[kind]` badge but for
/// models), a `#tags` line, and — expanded — a rule and its columns.
pub fn node_lines(node: &DagNode, expanded: bool) -> Vec<String> {
    let badge = if node.kind == "model" {
        String::new()
    } else {
        format!("  [{}]", node.kind.replace('_', " "))
    };
    let title = format!("{}{badge}", node.name);
    let mut lines = vec![title.clone()];
    if !node.tags.is_empty() {
        let tags: Vec<String> = node.tags.iter().map(|t| format!("#{t}")).collect();
        lines.push(tags.join(" "));
    }
    if expanded && !node.columns.is_empty() {
        lines.push("╌".repeat(title.chars().count()));
        for (i, column) in node.columns.iter().enumerate() {
            if i >= MAX_COLUMNS {
                lines.push(format!("… +{} more", node.columns.len() - MAX_COLUMNS));
                break;
            }
            lines.push(format!("• {column}"));
        }
    }
    lines
}

/// Every node placed; also the grid's width and height, as `layout.layout`
/// returns them (`draw` takes two more columns and one more row).
pub fn layout(dag: &Dag, expanded: &dyn Fn(&str) -> bool) -> (Vec<Placed>, i64, i64) {
    let deepest = dag.nodes.iter().map(|n| n.layer).max();
    let mut layers: Vec<Vec<&DagNode>> = vec![Vec::new(); deepest.map_or(0, |d| d + 1)];
    for node in &dag.nodes {
        layers[node.layer].push(node);
    }
    let mut parents: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &dag.edges {
        parents
            .entry(edge.to.as_str())
            .or_default()
            .push(edge.from.as_str());
    }

    let mut boxes = Vec::new();
    let mut centers: HashMap<&str, i64> = HashMap::new();
    let mut x = 2;
    let mut max_bottom = 0;
    for (li, column) in layers.iter_mut().enumerate() {
        if li > 0 {
            let score = |node: &DagNode| -> f64 {
                let placed: Vec<i64> = parents
                    .get(node.id.as_str())
                    .into_iter()
                    .flatten()
                    .filter_map(|p| centers.get(p).copied())
                    .collect();
                if placed.is_empty() {
                    f64::INFINITY
                } else {
                    placed.iter().sum::<i64>() as f64 / placed.len() as f64
                }
            };
            column.sort_by(|a, b| {
                score(a)
                    .total_cmp(&score(b))
                    .then_with(|| a.name.cmp(&b.name))
            });
        }
        let mut width = 0;
        let mut y = 2;
        for node in column.iter() {
            let lines = node_lines(node, expanded(&node.id));
            let inner = lines.iter().map(|l| width_of(l)).max().unwrap_or(0);
            let (w, h) = (inner + 4, lines.len() as i64 + 2);
            let cy = y + h / 2;
            boxes.push(Placed {
                id: node.id.clone(),
                lines,
                w,
                h,
                x,
                y,
                cy,
                layer: li + 1,
            });
            centers.insert(node.id.as_str(), cy);
            width = width.max(w);
            y += h + 1;
            max_bottom = max_bottom.max(y);
        }
        x += width + GUTTER;
    }
    (boxes, x, max_bottom)
}

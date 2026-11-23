//! One node of the site: its details and its columns, with each traced
//! column's inputs and how it is made.

use std::collections::BTreeMap;
use std::path::Path;

use cogitator_schemas::ColumnEntry;

use super::{docs, Hop, Traced, Transform};
use super::{node_id, site_kind, LineageState, SiteColumn, SiteInput, SiteKind, SiteNode};
use crate::features::column_types::{column_types, ModelColumnTypes, Provenance};
use crate::graph::NodeKind;
use crate::lineage::{LineageOrigin, ModelLineage, SourceRef};
use crate::selectors::ResourceType;
use crate::DbtContext;

pub(super) fn node_of(input: &SiteInput<'_>, kind: NodeKind, name: &str, uri: &Path) -> SiteNode {
    let ctx = input.ctx;
    let site = site_kind(ctx, kind, name);
    let mut node = SiteNode {
        id: node_id(kind, name),
        name: name.to_string(), // boundary: the site owns its names
        kind: site,
        layer: 0,
        uri: file_of(uri).map(cogitator_lsp::fs_path_to_uri),
        path: file_of(uri).and_then(|p| relative(ctx, p)),
        package: None,
        materialized: None,
        description: String::new(),
        tags: Vec::new(),
        tests: Vec::new(),
        columns: Vec::new(),
        lineage: LineageState::NotApplicable,
        note: None,
    };
    match kind {
        NodeKind::Model | NodeKind::Macro => model_node(input, &mut node),
        NodeKind::Source => source_node(ctx, &mut node),
        NodeKind::Exposure | NodeKind::Metric | NodeKind::SemanticModel => {
            let details = match kind {
                NodeKind::Exposure => ctx.exposure_detail_map.get(name),
                NodeKind::Metric => ctx.metric_detail_map.get(name),
                _ => ctx.semantic_model_detail_map.get(name),
            };
            if let Some(d) = details {
                node.description = docs::resolve_docs(ctx, d.description.as_deref().unwrap_or(""));
                if node.uri.is_none() {
                    node.uri = file_of(&d.uri).map(cogitator_lsp::fs_path_to_uri);
                    node.path = file_of(&d.uri).and_then(|p| relative(ctx, p));
                }
            }
        }
        NodeKind::Test | NodeKind::Analysis => {}
    }
    node
}

fn file_of(path: &Path) -> Option<&Path> {
    (!path.as_os_str().is_empty()).then_some(path)
}

fn relative(ctx: &DbtContext, path: &Path) -> Option<String> {
    let root = ctx.project_root.as_deref()?;
    let rel = path.strip_prefix(root).unwrap_or(path);
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

fn model_node(input: &SiteInput<'_>, node: &mut SiteNode) {
    let ctx = input.ctx;
    let resource = match node.kind {
        SiteKind::Seed => ResourceType::Seed,
        SiteKind::Snapshot => ResourceType::Snapshot,
        _ => ResourceType::Model,
    };
    if let Some(found) = input.universe.find(resource, &node.name) {
        let u = &input.universe.nodes[found];
        node.package = Some(u.package.clone()); // boundary: as above
        node.materialized = u
            .config
            .get("materialized")
            .and_then(|m| m.as_str())
            .map(str::to_string);
    }
    let Some(details) = ctx.model_detail_map.get(&node.name) else {
        node.note = Some("No file in the project defines this model.".to_string());
        return;
    };
    if node.uri.is_none() {
        node.uri = file_of(&details.uri).map(cogitator_lsp::fs_path_to_uri);
        node.path = file_of(&details.uri).and_then(|p| relative(ctx, p));
    }
    if node.package.is_none() && !details.project_name.is_empty() {
        node.package = Some(details.project_name.clone());
    }
    node.description = docs::resolve_docs(ctx, &details.description);
    node.tags = details.tags.clone();
    match node.kind {
        SiteKind::Model => {
            let types = input.schemas.and_then(|s| column_types(ctx, s, &node.name));
            match (input.traced)(&node.name) {
                Traced::Lineage(lineage) => traced_columns(ctx, node, &lineage, types.as_ref()),
                declined => {
                    node.lineage = LineageState::Declined;
                    node.note = Some(decline_note(&declined).to_string());
                }
            }
        }
        SiteKind::Snapshot => {
            node.lineage = LineageState::Declined;
            node.note = Some("Snapshots are not traced: the snapshot block is Jinja.".to_string());
        }
        _ => {}
    }
    append_documented(ctx, node, &details.columns, "");
}

fn decline_note(traced: &Traced) -> &'static str {
    match traced {
        Traced::ControlFlow => {
            "The template has Jinja control flow and no compiled SQL known fresh. \
             Run `dbt compile`, then generate the site again."
        }
        Traced::Incomplete => {
            "The SQL did not parse completely, so no column is claimed: \
             a partial answer would read as a complete one."
        }
        Traced::Unreadable => "The model file could not be read.",
        Traced::Lineage(_) => "",
    }
}

fn traced_columns(
    ctx: &DbtContext,
    node: &mut SiteNode,
    lineage: &ModelLineage,
    types: Option<&ModelColumnTypes>,
) {
    node.lineage = match lineage.origin {
        Some(LineageOrigin::Compiled) => LineageState::Compiled,
        None => LineageState::Traced,
    };
    let documented = ctx.model_detail_map.get(&node.name).map(|d| &d.columns);
    for column in &lineage.columns {
        let yaml = documented.and_then(|cols| find_entry(cols, &column.name));
        let typed = types.and_then(|t| t.column(&column.name));
        let (transform, detail) = match typed {
            Some(t) => transform_of(&t.provenance, &column.name),
            None => (None, None),
        };
        node.columns.push(SiteColumn {
            name: column.name.clone(), // boundary: as above
            description: yaml
                .map(|y| docs::resolve_docs(ctx, &y.description))
                .unwrap_or_default(),
            data_type: yaml
                .and_then(|y| y.data_type.clone())
                .or_else(|| typed.and_then(|t| t.data_type.clone())),
            documented: yaml.is_some(),
            produced: true,
            tests: yaml.map(column_tests).unwrap_or_default(),
            inputs: column.sources.iter().map(hop_of).collect(),
            star: column.name == "*" || column.name.ends_with(".*"),
            transform,
            detail,
        });
    }
}

fn transform_of(provenance: &Provenance, name: &str) -> (Option<Transform>, Option<String>) {
    let derived = |detail: String| (Some(Transform::Derived), Some(detail));
    match provenance {
        Provenance::Upstream { from } if from.column.eq_ignore_ascii_case(name) => {
            (Some(Transform::Passthrough), None)
        }
        Provenance::Upstream { from } => (Some(Transform::Renamed), Some(from.column.clone())),
        Provenance::Literal => (Some(Transform::Constant), None),
        Provenance::Cast => derived("cast".to_string()),
        Provenance::Function { name } => derived(format!("{name}()")),
        Provenance::Aggregate { name } => derived(format!("{name}() aggregate")),
        Provenance::Window { name } => derived(format!("{name}() window")),
        Provenance::SetOp => derived("set operation".to_string()),
        Provenance::Expression => derived("expression".to_string()),
        Provenance::Unknown => (Some(Transform::Unknown), None),
    }
}

fn hop_of(source: &SourceRef) -> Hop {
    match source {
        SourceRef::Model { name, column } => Hop {
            node: Some(node_id(NodeKind::Model, name)),
            column: column.clone(), // boundary: as above
            relation: None,
        },
        SourceRef::Source {
            source,
            table,
            column,
        } => Hop {
            node: Some(node_id(NodeKind::Source, source)),
            column: format!("{table}.{column}"),
            relation: None,
        },
        SourceRef::Unknown { name, column } => Hop {
            node: None,
            column: column.clone(),
            relation: Some(name.clone()),
        },
    }
}

fn source_node(ctx: &DbtContext, node: &mut SiteNode) {
    let Some(source) = ctx.source_detail_map.get(&node.name) else {
        return;
    };
    node.uri = file_of(&source.uri).map(cogitator_lsp::fs_path_to_uri);
    node.path = file_of(&source.uri).and_then(|p| relative(ctx, p));
    node.description = docs::resolve_docs(ctx, &source.description);
    let tables: BTreeMap<&String, _> = source.tables.iter().collect();
    for (name, table) in tables {
        append_documented(ctx, node, &table.columns, &format!("{name}."));
    }
}

/// YAML columns the node does not list yet, `prefix`ed (a source table).
fn append_documented(ctx: &DbtContext, node: &mut SiteNode, entries: &[ColumnEntry], prefix: &str) {
    for entry in entries {
        let name = format!("{prefix}{}", entry.name);
        if node
            .columns
            .iter()
            .any(|c| c.name.eq_ignore_ascii_case(&name))
        {
            continue;
        }
        node.columns.push(SiteColumn {
            name,
            description: docs::resolve_docs(ctx, &entry.description),
            data_type: entry.data_type.clone(),
            documented: true,
            produced: false,
            tests: column_tests(entry),
            inputs: Vec::new(),
            star: false,
            transform: None,
            detail: None,
        });
    }
}

fn find_entry<'a>(entries: &'a [ColumnEntry], name: &str) -> Option<&'a ColumnEntry> {
    entries.iter().find(|e| e.name.eq_ignore_ascii_case(name))
}

fn column_tests(entry: &ColumnEntry) -> Vec<String> {
    entry.tests.iter().map(|t| t.name.clone()).collect()
}

/// True when a column's inputs are known (or it is a literal).
pub(super) fn is_traced(column: &SiteColumn) -> bool {
    !column.star && (!column.inputs.is_empty() || column.transform == Some(Transform::Constant))
}

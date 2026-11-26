use pretty_assertions::assert_eq;
use rstest::rstest;

use super::*;
use crate::lineage::ColumnLineage;

/// `name <- model.column, …`; `=` for a direct column: `"id = a.id"`.
fn column(spec: &str) -> ColumnLineage {
    let (name, sources, direct) = match spec.split_once(" = ") {
        Some((name, sources)) => (name, sources, true),
        None => {
            let (name, sources) = spec.split_once(" <- ").unwrap_or((spec, ""));
            (name, sources, false)
        }
    };
    let sources = sources
        .split(", ")
        .filter(|s| !s.is_empty())
        .map(|s| {
            let (relation, column) = s.rsplit_once('.').expect("relation.column");
            match relation.split_once(':') {
                Some(("source", rest)) => {
                    let (source, table) = rest.split_once('/').expect("source/table");
                    SourceRef::Source {
                        source: source.into(),
                        table: table.into(),
                        column: column.into(),
                    }
                }
                Some(("raw", name)) => SourceRef::Unknown {
                    name: name.into(),
                    column: column.into(),
                },
                _ => SourceRef::Model {
                    name: relation.into(),
                    column: column.into(),
                },
            }
        })
        .collect();
    ColumnLineage {
        name: name.into(),
        sources,
        direct,
        derived: !direct,
        end: None,
    }
}

fn lineage(columns: &[&str]) -> ModelLineage {
    ModelLineage {
        columns: columns.iter().map(|c| column(c)).collect(),
        origin: None,
    }
}

/// A small project: a staging model over a source, a star model over it,
/// a mart that renames and computes, and a model reading both.
fn project() -> Vec<(&'static str, ModelLineage)> {
    vec![
        (
            "stg",
            lineage(&[
                "id = source:shop/customers.id",
                "name = source:shop/customers.name",
            ]),
        ),
        ("wide", lineage(&["* = stg.*"])),
        (
            "mart",
            lineage(&[
                "customer_key <- wide.id",
                "label <- wide.name, raw:lookup.label",
                "Name = wide.NAME",
            ]),
        ),
        (
            "report",
            lineage(&["key = mart.customer_key", "id = stg.id"]),
        ),
    ]
}

fn graph() -> ColumnGraph {
    let models = project();
    ColumnGraph::build(models.iter().map(|(m, l)| (*m, l)))
}

fn label(graph: &ColumnGraph, node: NodeIndex) -> String {
    let c = graph.column(node);
    let relation = match &c.relation {
        Relation::Model(m) => m.clone(),
        Relation::Source { source, table } => format!("{source}/{table}"),
        Relation::Unknown(n) => format!("raw:{n}"),
    };
    format!("{relation}.{}", c.column)
}

/// In the walks' order: depth, then relation (models, then sources, then
/// unknown relations, each by name), then column ignoring case.
fn labels(graph: &ColumnGraph, reached: &[Reached]) -> Vec<String> {
    reached
        .iter()
        .map(|r| format!("{}@{}", label(graph, r.node), r.depth))
        .collect()
}

#[rstest]
// a star passes every column of its input on, under the input's names
#[case("wide", &["*", "id", "name"])]
// the select list's order, case as written
#[case("mart", &["customer_key", "label", "Name"])]
#[case("unknown_model", &[])]
fn a_models_outputs(#[case] model: &str, #[case] want: &[&str]) {
    let g = graph();
    let got: Vec<&str> = g
        .outputs(model)
        .iter()
        .map(|&n| g.column(n).column.as_str())
        .collect();
    assert_eq!(got, want);
}

#[rstest]
#[case("stg", "id", None, &["report.id@1", "wide.id@1", "mart.customer_key@2", "report.key@3"])]
#[case("stg", "id", Some(1), &["report.id@1", "wide.id@1"])]
// case-insensitive: NAME, name and Name are one column
#[case("stg", "NAME", None, &["wide.name@1", "mart.label@2", "mart.Name@2"])]
#[case("report", "key", None, &[])]
fn downstream_of_a_column(
    #[case] model: &str,
    #[case] column: &str,
    #[case] depth: Option<usize>,
    #[case] want: &[&str],
) {
    let g = graph();
    let start = g.find_model_column(model, column).expect("the column");
    assert_eq!(labels(&g, &g.downstream(&[start], depth)), want);
}

#[test]
fn upstream_reaches_sources_and_unknown_relations() {
    let g = graph();
    let start = g.find_model_column("mart", "label").unwrap();
    assert_eq!(
        labels(&g, &g.upstream(&[start], None)),
        [
            "wide.name@1",
            "raw:lookup.label@1",
            "stg.name@2",
            "shop/customers.name@3"
        ]
    );
}

#[test]
fn a_closure_starts_from_several_columns_at_once() {
    let g = graph();
    let id = g.find_model_column("stg", "id").unwrap();
    let name = g.find_model_column("stg", "name").unwrap();
    let both = g.downstream(&[id, name], Some(1));
    assert_eq!(
        labels(&g, &both),
        ["report.id@1", "wide.id@1", "wide.name@1"]
    );
}

#[rstest]
// through the star and the direct hop to the source, under one name
#[case("mart", "Name", &["mart.Name", "wide.name", "stg.name", "shop/customers.name"])]
// a rename ends the chain where it starts
#[case("mart", "customer_key", &["mart.customer_key"])]
// two upstream columns: no single origin
#[case("mart", "label", &["mart.label"])]
fn the_origin_of_a_column(#[case] model: &str, #[case] column: &str, #[case] want: &[&str]) {
    let g = graph();
    let start = g.find_model_column(model, column).unwrap();
    let chain: Vec<String> = g
        .origin_chain(start, 64)
        .iter()
        .map(|&n| label(&g, n))
        .collect();
    assert_eq!(chain, want);
}

#[test]
fn flows_say_how_a_column_is_passed_on() {
    let g = graph();
    let at = |m, c| g.find_model_column(m, c).unwrap();
    assert_eq!(g.reads(at("wide", "id")), [(at("stg", "id"), Flow::Star)]);
    assert_eq!(
        g.reads(at("mart", "Name")),
        [(at("wide", "name"), Flow::Direct)]
    );
    assert_eq!(
        g.reads(at("mart", "customer_key")),
        [(at("wide", "id"), Flow::Derived)]
    );
}

/// dbt.impact walked models in (depth, name) order, so a model one step
/// downstream that reads a sibling sorting after it missed the sibling's
/// columns. A graph walk has no order to get wrong.
#[test]
fn a_sibling_read_through_a_sibling_is_downstream() {
    let models = [
        ("base", lineage(&["x = source:s/t.x"])),
        ("a_reader", lineage(&["x = z_sibling.x"])),
        ("z_sibling", lineage(&["x = base.x"])),
    ];
    let g = ColumnGraph::build(models.iter().map(|(m, l)| (*m, l)));
    let start = g.find_model_column("base", "x").unwrap();
    assert_eq!(
        labels(&g, &g.downstream(&[start], None)),
        ["z_sibling.x@1", "a_reader.x@2"]
    );
}

#[test]
fn cycles_and_internal_placeholders_end_nowhere() {
    let models = [
        ("a", lineage(&["x = b.x", "rel <- b.__dbt_rel_0"])),
        ("b", lineage(&["x = a.x"])),
        ("c", lineage(&["* = d.*"])),
        ("d", lineage(&["* = c.*", "y = source:s/t.y"])),
    ];
    let g = ColumnGraph::build(models.iter().map(|(m, l)| (*m, l)));
    let ax = g.find_model_column("a", "x").unwrap();
    assert_eq!(labels(&g, &g.downstream(&[ax], None)), ["b.x@1"]);
    assert_eq!(
        g.origin_chain(ax, 64).len(),
        2,
        "the cycle stops where it repeats"
    );
    assert!(g.find_model_column("b", "__dbt_rel_0").is_none());
    // a star cycle resolves what is known and stops
    let cy = g
        .find_model_column("c", "y")
        .expect("c gains d's y through its star");
    assert_eq!(
        g.reads(cy),
        [(g.find_model_column("d", "y").unwrap(), Flow::Star)]
    );
}

#[test]
fn the_answer_does_not_depend_on_the_order_models_are_added() {
    let models = project();
    let forward = ColumnGraph::build(models.iter().map(|(m, l)| (*m, l)));
    let backward = ColumnGraph::build(models.iter().rev().map(|(m, l)| (*m, l)));
    let walk = |g: &ColumnGraph| {
        let start = g.find_model_column("stg", "id").unwrap();
        labels(g, &g.downstream(&[start], None))
    };
    assert_eq!(walk(&forward), walk(&backward));
    assert_eq!(forward.len(), backward.len());
}

/// An untyped lineage writes an unexpanded `select *` as one `*` row of
/// named columns: each is passed on under its own name, unless the model
/// lists that name itself.
#[test]
fn a_star_row_of_named_columns_passes_each_on() {
    let models = [
        (
            "stg",
            lineage(&["id = source:s/t.id", "name = source:s/t.name"]),
        ),
        (
            "mart",
            lineage(&["* <- stg.id, stg.name", "name <- stg.name"]),
        ),
    ];
    let g = ColumnGraph::build(models.iter().map(|(m, l)| (*m, l)));
    let at = |m, c| g.find_model_column(m, c).unwrap();
    let outputs: Vec<&str> = g
        .outputs("mart")
        .iter()
        .map(|&n| g.column(n).column.as_str())
        .collect();
    assert_eq!(
        outputs,
        ["name", "id"],
        "no `*` column: every source is named"
    );
    assert_eq!(g.reads(at("mart", "id")), [(at("stg", "id"), Flow::Star)]);
    // listed by the model itself: its own lineage, not the star's
    assert_eq!(
        g.reads(at("mart", "name")),
        [(at("stg", "name"), Flow::Derived)]
    );
}

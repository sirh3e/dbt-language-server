use super::*;
use pretty_assertions::assert_eq;
use rstest::rstest;

fn node(name: &str, layer: usize) -> DagNode {
    DagNode {
        id: format!("model:{name}"),
        name: name.to_string(),
        kind: "model".into(),
        layer,
        tags: Vec::new(),
        columns: Vec::new(),
    }
}

fn edge(from: &str, to: &str) -> DagEdge {
    DagEdge {
        from: format!("model:{from}"),
        to: format!("model:{to}"),
    }
}

/// A graph whose layers `relayer` assigns, as a focus would.
fn graph(names: &[&str], edges: &[(&str, &str)]) -> Dag {
    let edges: Vec<DagEdge> = edges.iter().map(|(f, t)| edge(f, t)).collect();
    let nodes = names.iter().map(|n| node(n, 0)).collect();
    Dag {
        nodes: relayer(nodes, &edges),
        edges,
    }
}

fn drawn(dag: &Dag) -> String {
    let mut lines = picture(dag, &Expanded::None);
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

#[rstest]
#[case::chain(&["a", "b", "c"], &[("a", "b"), ("b", "c")], &[0, 1, 2])]
#[case::diamond(&["a", "b", "c", "d"], &[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")], &[0, 1, 1, 2])]
#[case::longest_path_wins(&["a", "b", "c"], &[("a", "b"), ("b", "c"), ("a", "c")], &[0, 1, 2])]
#[case::cycle_one_past_the_deepest(&["r", "a", "x", "y"], &[("r", "a"), ("x", "y"), ("y", "x")], &[0, 1, 2, 2])]
#[case::self_loop_ignored(&["a"], &[("a", "a")], &[0])]
fn layers_follow_relayer(
    #[case] names: &[&str],
    #[case] edges: &[(&str, &str)],
    #[case] layers: &[usize],
) {
    let got: Vec<usize> = graph(names, edges).nodes.iter().map(|n| n.layer).collect();
    assert_eq!(got, layers);
}

#[rstest]
#[case::model(node("orders", 0), false, vec!["orders"])]
#[case::source(
    DagNode { kind: "semantic_model".into(), ..node("orders_sm", 0) },
    false,
    vec!["orders_sm  [semantic model]"]
)]
#[case::tags(
    DagNode { tags: vec!["daily".into(), "pii".into()], ..node("orders", 0) },
    false,
    vec!["orders", "#daily #pii"]
)]
#[case::columns_only_expanded(
    DagNode { columns: vec!["id".into()], ..node("orders", 0) },
    false,
    vec!["orders"]
)]
#[case::columns(
    DagNode { columns: vec!["id".into(), "amount".into()], ..node("orders", 0) },
    true,
    vec!["orders", "╌╌╌╌╌╌", "• id", "• amount"]
)]
fn a_box_holds_its_title_tags_and_columns(
    #[case] n: DagNode,
    #[case] expanded: bool,
    #[case] lines: Vec<&str>,
) {
    assert_eq!(node_lines(&n, expanded), lines);
}

#[test]
fn an_expanded_box_lists_eight_columns_then_says_how_many_more() {
    let n = DagNode {
        columns: (1..=10).map(|i| format!("c{i}")).collect(),
        ..node("t", 0)
    };
    let lines = node_lines(&n, true);
    assert_eq!(lines.len(), 1 + 1 + MAX_COLUMNS + 1);
    assert_eq!(lines.last().map(String::as_str), Some("… +2 more"));
}

#[test]
fn a_column_orders_by_its_parents_then_by_name() {
    // b and c sit under a, z under y: z's parent is below, so z comes last
    let dag = graph(
        &["a", "y", "z", "c", "b"],
        &[("a", "c"), ("a", "b"), ("y", "z")],
    );
    let (boxes, _, _) = layout(&dag, &|_| false);
    let second: Vec<&str> = boxes
        .iter()
        .filter(|b| b.layer == 2)
        .map(|b| b.id.as_str())
        .collect();
    assert_eq!(second, vec!["model:b", "model:c", "model:z"]);
}

#[test]
fn focus_keeps_the_hops_up_and_down_but_not_sideways() {
    let dag = graph(
        &["s", "a", "b", "c", "d", "sib"],
        &[("s", "a"), ("a", "b"), ("b", "c"), ("c", "d"), ("a", "sib")],
    );
    let near = focus(&dag, "model:b", 1);
    let ids: Vec<&str> = near.nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(ids, vec!["a", "b", "c"]);
    let layers: Vec<usize> = near.nodes.iter().map(|n| n.layer).collect();
    assert_eq!(layers, vec![0, 1, 2]);
    assert_eq!(near.edges.len(), 2);
    let wider = focus(&dag, "model:b", 2);
    assert_eq!(wider.nodes.len(), 5, "s and d, never the sibling");
    assert_eq!(focus(&dag, "model:nope", 2), dag);
}

#[test]
fn a_large_graph_keeps_its_first_layers_and_says_so() {
    let names: Vec<String> = (0..10).map(|i| format!("m{i}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let chain: Vec<(&str, &str)> = refs.windows(2).map(|w| (w[0], w[1])).collect();
    let dag = graph(&refs, &chain);
    let answer = render(&dag, &Subject::Project, &Expanded::None, 4);
    assert!(answer.truncated);
    assert_eq!(answer.nodes, 4);
    assert!(answer.text.contains("Showing the first 4 of 10 nodes"));
    assert!(answer.text.contains("m3") && !answer.text.contains("m4"));
}

#[test]
fn chain() {
    insta::assert_snapshot!(drawn(&graph(&["a", "b", "c"], &[("a", "b"), ("b", "c")])));
}

#[test]
fn diamond() {
    insta::assert_snapshot!(drawn(&graph(
        &["src", "left", "right", "mart"],
        &[
            ("src", "left"),
            ("src", "right"),
            ("left", "mart"),
            ("right", "mart")
        ],
    )));
}

#[test]
fn cycle() {
    insta::assert_snapshot!(drawn(&graph(
        &["root", "a", "b"],
        &[("root", "a"), ("a", "b"), ("b", "a")],
    )));
}

#[test]
fn wide_layer() {
    insta::assert_snapshot!(drawn(&graph(
        &["hub", "one", "two", "three", "four", "sink"],
        &[
            ("hub", "one"),
            ("hub", "two"),
            ("hub", "three"),
            ("hub", "four"),
            ("one", "sink"),
            ("four", "sink"),
        ],
    )));
}

#[test]
fn tags_kinds_and_columns() {
    let mut dag = graph(
        &["raw_orders", "orders", "revenue"],
        &[("raw_orders", "orders"), ("orders", "revenue")],
    );
    dag.nodes[0].kind = "source".into();
    dag.nodes[0].id = "source:raw_orders".into();
    dag.edges[0].from = "source:raw_orders".into();
    dag.nodes[1].tags = vec!["daily".into(), "finance".into()];
    dag.nodes[1].columns = vec!["order_id".into(), "amount".into()];
    dag.nodes[2].kind = "exposure".into();
    let expanded = Expanded::Some(["model:orders".to_string()].into());
    insta::assert_snapshot!(
        render(
            &dag,
            &Subject::Around {
                model: "orders",
                depth: 1
            },
            &expanded,
            MAX_NODES
        )
        .text
    );
}

#[test]
fn file_names_are_safe() {
    assert_eq!(file_name(None), "project.dag.txt");
    assert_eq!(file_name(Some("a/b c")), "a_b_c.dag.txt");
}

use super::*;
use crate::selectors::syntax;
use crate::selectors::universe::{Node, ResourceType, TestInfo};

/// a → b → c, d → b; t tests b (generic), u tests c and d (cautious
/// relevant); src feeds a.
fn universe() -> Universe {
    let mut u = Universe {
        root_package: "p".into(),
        ..Universe::default()
    };
    let mut add = |name: &str, rt: ResourceType, tags: &[&str]| {
        let mut n = Node::new(rt, name, "p");
        n.tags = tags.iter().map(|t| t.to_string()).collect();
        if rt == ResourceType::Test {
            n.test = Some(TestInfo {
                test_type: "generic",
                test_name: Some("unique".into()),
            });
        }
        if rt == ResourceType::Source {
            n.source = Some(("raw".into(), name.into()));
        }
        u.push(n)
    };
    let a = add("a", ResourceType::Model, &["x"]);
    let b = add("b", ResourceType::Model, &["x", "y"]);
    let c = add("c", ResourceType::Model, &[]);
    let d = add("d", ResourceType::Model, &["y"]);
    let t = add("t", ResourceType::Test, &[]);
    let w = add("w", ResourceType::Test, &[]);
    let src = add("orders", ResourceType::Source, &[]);
    u.link(b, a);
    u.link(c, b);
    u.link(b, d);
    u.link(a, src);
    u.link(t, b);
    u.link(w, c);
    u.link(w, d);
    u
}

fn names(u: &Universe, e: &Evaluation) -> Vec<String> {
    let mut v: Vec<String> = e.nodes.iter().map(|&i| u.nodes[i].display_name()).collect();
    v.sort();
    v
}

fn run(text: &str, indirect: Indirect) -> Vec<String> {
    let u = universe();
    let s = syntax::parse(text).unwrap();
    names(&u, &select(&u, &[], &s, indirect))
}

#[test]
fn graph_operators_and_sets() {
    let cases: &[(&str, &[&str])] = &[
        ("b", &["b"]),
        ("+b", &["a", "b", "d", "raw.orders"]),
        ("1+b", &["a", "b", "d"]),
        ("b+", &["b", "c", "t", "w"]),
        ("a+1", &["a", "b"]),
        ("a+", &["a", "b", "c", "t", "w"]),
        ("@a", &["a", "b", "c", "d", "raw.orders", "t", "w"]),
        ("tag:x", &["a", "b"]),
        ("tag:x,tag:y", &["b"]),
        ("tag:x tag:y", &["a", "b", "d"]),
        ("source:raw+1", &["a", "raw.orders"]),
        ("resource_type:test", &["t", "w"]),
        ("test_name:unique,resource_type:test", &["t", "w"]),
        ("wildcard:*", &["a", "b", "c", "d", "raw.orders", "t", "w"]),
        ("tag:nope", &[]),
        // tests are graph nodes: a test's parents are what it tests
        ("+t", &["a", "b", "d", "raw.orders", "t"]),
    ];
    for (text, expected) in cases {
        assert_eq!(run(text, Indirect::Empty), *expected, "{text}");
    }
}

#[test]
fn indirect_selection_modes() {
    let cases: &[(&str, Indirect, &[&str])] = &[
        ("b", Indirect::Eager, &["b", "t"]),
        ("c", Indirect::Eager, &["c", "w"]),
        ("c", Indirect::Cautious, &["c"]),
        ("c d", Indirect::Cautious, &["c", "d", "w"]),
        ("c", Indirect::Buildable, &["c", "w"]),
        ("b", Indirect::Empty, &["b"]),
        // an intersection keeps what both sides bring along
        ("tag:x,tag:y", Indirect::Eager, &["b", "t"]),
    ];
    for (text, mode, expected) in cases {
        assert_eq!(run(text, *mode), *expected, "{text} {mode:?}");
    }
}

fn defs(text: &str) -> Vec<Definition> {
    crate::selectors::yaml::parse(text).definitions
}

#[test]
fn definitions_reference_each_other() {
    let u = universe();
    let d = defs(
        "selectors:\n  - name: base\n    definition: 'tag:x'\n  - name: wide\n    definition:\n      union:\n        - method: selector\n          value: base\n        - c\n        - exclude:\n            - a\n  - name: loop1\n    definition: 'selector:loop2'\n  - name: loop2\n    definition: 'selector:loop1'\n  - name: lost\n    definition: 'selector:nowhere'\n  - name: old\n    definition: 'state:modified'\n  - name: odd\n    definition: 'tagz:x'\n",
    );
    let wide = select_definition(&u, &d, "wide");
    assert_eq!(names(&u, &wide), vec!["b", "c", "t", "w"]);
    assert!(wide.issues.is_empty());
    let looped = select_definition(&u, &d, "loop1");
    assert!(looped.nodes.is_empty());
    assert!(matches!(
        &looped.issues[..],
        [Issue::Cycle { chain, .. }] if chain == &["loop1", "loop2", "loop1"]
    ));
    let lost = select_definition(&u, &d, "lost");
    assert!(matches!(&lost.issues[..], [Issue::UnknownSelector { name, .. }] if name == "nowhere"));
    let old = select_definition(&u, &d, "old");
    assert!(matches!(&old.issues[..], [Issue::NeedsState { method, .. }] if method == "state"));
    let odd = select_definition(&u, &d, "odd");
    assert!(matches!(&odd.issues[..], [Issue::UnknownMethod { name, .. }] if name == "tagz"));
}

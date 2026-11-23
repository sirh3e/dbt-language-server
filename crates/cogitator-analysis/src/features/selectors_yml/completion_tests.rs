use super::*;

fn universe() -> Universe {
    let mut u = Universe {
        root_package: "p".into(),
        ..Universe::default()
    };
    let mut m = Node::new(ResourceType::Model, "orders", "p");
    m.tags = vec!["nightly".into(), "mart".into()];
    m.path = "models/marts/orders.sql".into();
    m.config.insert("materialized".into(), "table".into());
    u.push(m);
    u
}

fn labels(uri: &str, text: &str, line: u32, character: u32) -> Option<Vec<String>> {
    let defs = crate::selectors::yaml::parse(text).definitions;
    selector_completion(uri, text, Position { line, character }, &universe(), &defs)
        .map(|items| items.into_iter().map(|i| i.label).collect())
}

#[test]
fn completion_cases() {
    let sel = "file:///p/selectors.yml";
    let text = "selectors:\n  - name: a\n    definition:\n      method: ta\n  - name: b\n    definition: 'tag:ni'\n  - name: c\n    definition: 'models/m'\n  - name: d\n    definition:\n      method: config.materialized\n      value: \n  - name: e\n    definition: 'selector:'\n";
    let cases: &[(u32, u32, &[&str])] = &[
        (3, 16, &["tag"]),
        (5, 23, &["nightly"]),
        (11, 13, &["table"]),
        (13, 26, &["a", "b", "c", "d", "e"]),
    ];
    for (line, character, expected) in cases {
        assert_eq!(
            labels(sel, text, *line, *character).unwrap(),
            *expected,
            "{line}:{character}"
        );
    }
    // a bare word offers methods and node names
    let word = labels(sel, "selectors:\n  - name: x\n    definition: 'ord", 2, 20).unwrap();
    assert_eq!(word, vec!["orders"]);
    let word = labels(sel, "selectors:\n  - name: x\n    definition: 'con", 2, 20).unwrap();
    assert_eq!(word, vec!["config.materialized"]);
}

#[test]
fn tags_complete_in_project_and_properties_files() {
    let props = "models:\n  - name: m\n    tags: [ni";
    assert_eq!(
        labels("file:///p/models/schema.yml", props, 2, 14).unwrap(),
        vec!["nightly"]
    );
    let project = "models:\n  p:\n    +tags:\n      - ";
    assert_eq!(
        labels("file:///p/dbt_project.yml", project, 3, 8).unwrap(),
        vec!["mart", "nightly"]
    );
    assert!(labels("file:///p/models/schema.yml", props, 1, 5).is_none());
}

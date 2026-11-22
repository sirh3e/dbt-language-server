use super::*;
use rstest::rstest;

fn key(k: &str) -> PathStep {
    PathStep::Key(k.to_string())
}

/// `|` marks the cursor.
fn at(marked: &str) -> YamlCursor {
    let offset = marked.find('|').expect("cursor mark");
    let text = marked.replace('|', "");
    let before = &marked[..offset];
    let line = before.matches('\n').count() as u32;
    let character = before.rsplit('\n').next().unwrap_or("").chars().count() as u32;
    cursor_at(&text, Position { line, character }).expect("cursor in text")
}

const MODEL: &str = "version: 2\nmodels:\n  - name: customers\n    config:\n      materialized: table\n    columns:\n      - name: id\n        data_tests:\n          - not_null\n";

#[rstest]
#[case::root("ver|", vec![], Slot::Key { partial: "ver".into() })]
#[case::model_key(
    "models:\n  - name: a\n    desc|",
    vec![key("models"), PathStep::Item],
    Slot::Key { partial: "desc".into() }
)]
#[case::config_key(
    "models:\n  - name: a\n    config:\n      mat|",
    vec![key("models"), PathStep::Item, key("config")],
    Slot::Key { partial: "mat".into() }
)]
#[case::config_value(
    "models:\n  - name: a\n    config:\n      materialized: ta|",
    vec![key("models"), PathStep::Item, key("config")],
    Slot::Value { key: "materialized".into(), partial: "ta".into() }
)]
#[case::column_name(
    "models:\n  - name: a\n    columns:\n      - name: |",
    vec![key("models"), PathStep::Item, key("columns"), PathStep::Item],
    Slot::Value { key: "name".into(), partial: String::new() }
)]
#[case::test_item(
    "models:\n  - name: a\n    columns:\n      - name: id\n        data_tests:\n          - no|",
    vec![key("models"), PathStep::Item, key("columns"), PathStep::Item, key("data_tests")],
    Slot::Item { partial: "no".into() }
)]
#[case::test_argument(
    "models:\n  - name: a\n    columns:\n      - name: id\n        data_tests:\n          - accepted_values:\n              va|",
    vec![key("models"), PathStep::Item, key("columns"), PathStep::Item, key("data_tests"), PathStep::Item, key("accepted_values")],
    Slot::Key { partial: "va".into() }
)]
#[case::sibling_after_nested(
    "models:\n  - name: a\n    config:\n      materialized: table\n    co|",
    vec![key("models"), PathStep::Item],
    Slot::Key { partial: "co".into() }
)]
#[case::second_item(
    "models:\n  - name: a\n    columns:\n      - name: id\n      - name: b\n        de|",
    vec![key("models"), PathStep::Item, key("columns"), PathStep::Item],
    Slot::Key { partial: "de".into() }
)]
#[case::unindented_list(
    "models:\n- name: a\n  data_tests:\n  - uni|",
    vec![key("models"), PathStep::Item, key("data_tests")],
    Slot::Item { partial: "uni".into() }
)]
#[case::blank_under_list_key(
    "models:\n  - name: a\n    columns:\n      |",
    vec![key("models"), PathStep::Item, key("columns")],
    Slot::Key { partial: String::new() }
)]
#[case::comments_and_blanks(
    "models:\n  # a comment\n  - name: a\n\n    tags: [x]\n    acc|",
    vec![key("models"), PathStep::Item],
    Slot::Key { partial: "acc".into() }
)]
fn the_path_follows_indentation(
    #[case] marked: &str,
    #[case] path: Vec<PathStep>,
    #[case] slot: Slot,
) {
    assert_eq!(at(marked), YamlCursor { path, slot });
}

#[test]
fn a_cursor_inside_a_real_file() {
    let text = MODEL;
    let got = cursor_at(
        text,
        Position {
            line: 8,
            character: 20,
        },
    )
    .unwrap();
    assert_eq!(
        got.slot,
        Slot::Item {
            partial: "not_null".into()
        }
    );
    assert_eq!(got.path.len(), 5);
    assert!(cursor_at(
        text,
        Position {
            line: 99,
            character: 0
        }
    )
    .is_none());
}

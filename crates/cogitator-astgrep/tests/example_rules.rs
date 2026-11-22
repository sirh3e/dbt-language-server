//! The example rules shipped for the ast-grep CLI (editors/ast-grep) load
//! here, and each fires on the example project where it should — so the
//! server and the CLI read one set of rules alike.

use std::path::{Path, PathBuf};

use cogitator_astgrep::Rules;

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../editors/ast-grep")
}

fn rules() -> Rules {
    let rules = Rules::load(&example());
    assert!(rules.errors.is_empty(), "{:?}", rules.errors);
    rules
}

#[test]
fn the_example_rules_load() {
    assert_eq!(
        rules().ids(),
        [
            "ref-of-a-variable",
            "select-star-in-compiled-sql",
            "this-outside-incremental-guard"
        ]
    );
}

#[test]
fn each_example_rule_fires_where_it_should() {
    let rules = rules();
    let model = example().join("tests/models/example.sql");
    let text = std::fs::read_to_string(&model).unwrap();
    let found: Vec<(String, u32)> = rules
        .check(Path::new("models/example.sql"), &text)
        .into_iter()
        .map(|f| (f.rule, f.at.start.line))
        .collect();
    // the {{ this }} inside the guard (line 4) is fine
    assert_eq!(
        found,
        vec![
            ("ref-of-a-variable".to_string(), 1),
            ("this-outside-incremental-guard".to_string(), 2),
        ]
    );

    let compiled = example().join("tests/target/compiled/p/models/example.sql");
    let text = std::fs::read_to_string(&compiled).unwrap();
    let found: Vec<String> = rules
        .check(Path::new("target/compiled/p/models/example.sql"), &text)
        .into_iter()
        .map(|f| f.rule)
        .collect();
    assert_eq!(found, vec!["select-star-in-compiled-sql".to_string()]);
}

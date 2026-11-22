//! The "Run this test" selectors against a real dbt: every generic test
//! name dbt-core gives the fixtures (testdata/test_names/golden.json,
//! written by testdata/test_names/golden.sh) is exactly what the lenses
//! of their properties YAML select, no more and no less.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use cogitator_analysis::features::quality::yaml_tests;
use pretty_assertions::assert_eq;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn golden(key: &str) -> Vec<String> {
    let text = std::fs::read_to_string(testdata().join("test_names/golden.json")).unwrap();
    let golden: serde_json::Value = serde_json::from_str(&text).unwrap();
    serde_json::from_value(golden[key].clone()).unwrap()
}

/// The names every properties YAML under `dir` selects, each lens once.
fn selected(dir: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for entry in walkdir::WalkDir::new(dir).sort_by_file_name() {
        let entry = entry.unwrap();
        let path = entry.path();
        let yaml = path.extension().is_some_and(|e| e == "yml" || e == "yaml");
        let skip = path.components().any(|c| {
            matches!(
                c.as_os_str().to_str(),
                Some("pkgs" | "dbt_packages" | "target" | ".dbt")
            )
        });
        if !yaml || skip || path.file_name().is_some_and(|f| f == "dbt_project.yml") {
            continue;
        }
        let text = std::fs::read_to_string(path).unwrap();
        for test in yaml_tests(&text).into_iter().filter(|t| !t.unit) {
            assert_eq!(test.selector, test.names.join(" "), "{}", path.display());
            names.extend(test.names);
        }
    }
    names.sort();
    names
}

#[test]
fn lenses_select_dbts_own_names() {
    let names = selected(&testdata().join("test_names"));
    let unique: BTreeSet<_> = names.iter().collect();
    assert_eq!(
        unique.len(),
        names.len(),
        "a name selected twice: {names:?}"
    );
    assert_eq!(names, golden("names"));
}

#[test]
fn jaffle_lenses_select_dbts_own_names() {
    let names = selected(&testdata().join("jaffle_shop_duckdb/models"));
    assert_eq!(names, golden("jaffle_shop_duckdb"));
}

#[test]
fn a_base_entry_test_runs_on_every_version_it_applies_to() {
    let text = std::fs::read_to_string(testdata().join("test_names/models/schema.yml")).unwrap();
    let selectors: Vec<String> = yaml_tests(&text).into_iter().map(|t| t.selector).collect();
    for expected in [
        "tiny_utils_expression_is_true_vm_v1_id_0 tiny_utils_expression_is_true_vm_v2_id_0",
        "not_null_vm_v1_id not_null_vm_v2_id not_null_vm_v3_id",
        "unique_vm_v1_extra",
        "unique_vm_v3_id",
    ] {
        assert!(
            selectors.iter().any(|s| s == expected),
            "{expected}: {selectors:?}"
        );
    }
}

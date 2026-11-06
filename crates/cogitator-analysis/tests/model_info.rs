//! dbt.modelInfo: one-round-trip model detail for the :DbtBrowse browser —
//! neighbors, versions, tests and the compiled-SQL location.

use std::path::{Path, PathBuf};

use cogitator_analysis::features::{compiled_sql_path, model_info};
use cogitator_analysis::{build_context, DbtContext};
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn ctx() -> DbtContext {
    build_context(fixture_root().to_str().unwrap())
}

fn names(list: &serde_json::Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect()
}

#[test]
fn versioned_model_reports_neighbors_versions_and_tests() {
    let info = serde_json::to_value(model_info(&ctx(), "", Some("customer_orders")).expect("info"))
        .unwrap();

    assert_eq!(info["model"], "customer_orders");
    assert!(info["uri"]
        .as_str()
        .unwrap()
        .ends_with("customer_orders.sql"));
    assert!(names(&info["upstream"]).contains(&"orders"));

    let versions = &info["versions"];
    assert_eq!(versions["latestVersion"].as_i64(), Some(2));
    let list = versions["versions"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert!(list[0]["uri"]
        .as_str()
        .unwrap()
        .ends_with("customer_orders_v1.sql"));
    // the latest version falls back to the unsuffixed file
    assert!(list[1]["uri"]
        .as_str()
        .unwrap()
        .ends_with("customer_orders.sql"));

    let tests: Vec<(&str, Option<&str>, Option<i64>)> = info["tests"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            (
                t["name"].as_str().unwrap(),
                t["column"].as_str(),
                t["version"].as_i64(),
            )
        })
        .collect();
    assert!(
        tests.contains(&("unique", Some("customer_id"), None)),
        "{tests:?}"
    );
    assert!(
        tests.contains(&("not_null", Some("order_count"), Some(1))),
        "{tests:?}"
    );

    // no target/ in the fixture
    assert!(info["compiledUri"].is_null());
}

#[test]
fn versioned_file_keeps_its_node_but_reads_the_base_schema() {
    let info =
        serde_json::to_value(model_info(&ctx(), "", Some("customer_orders_v1")).expect("info"))
            .unwrap();
    assert_eq!(info["model"], "customer_orders_v1");
    assert!(names(&info["upstream"]).contains(&"stg_orders"));
    // versions and tests come from customer_orders.yml via the base model
    assert_eq!(info["versions"]["latestVersion"].as_i64(), Some(2));
    assert!(!info["tests"].as_array().unwrap().is_empty());
}

#[test]
fn unversioned_model_and_unknown_names() {
    let ctx = ctx();
    let info =
        serde_json::to_value(model_info(&ctx, "", Some("customers")).expect("info")).unwrap();
    assert!(info["versions"].is_null());
    assert!(!info["tests"].as_array().unwrap().is_empty());
    assert!(names(&info["upstream"]).contains(&"stg_customers"));
    // no model depends on customers; the fixture analysis file does
    assert_eq!(names(&info["downstream"]), vec!["order_counts"]);

    assert!(model_info(&ctx, "", Some("does_not_exist")).is_none());
    assert!(model_info(&ctx, "", None).is_none());
}

#[test]
fn compiled_uri_appears_when_dbt_wrote_output() {
    let root = std::env::temp_dir().join(format!("cogitator-modelinfo-{}", std::process::id()));
    let models = root.join("models");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(
        root.join("dbt_project.yml"),
        "name: t\nprofile: t\nmodel-paths: [\"models\"]\n",
    )
    .unwrap();
    std::fs::write(models.join("m.sql"), "select 1 as id\n").unwrap();
    let compiled_dir = root.join("target/compiled/t/models");
    std::fs::create_dir_all(&compiled_dir).unwrap();
    std::fs::write(compiled_dir.join("m.sql"), "select 1 as id\n").unwrap();

    let ctx = build_context(root.to_str().unwrap());
    let info = serde_json::to_value(model_info(&ctx, "", Some("m")).expect("info")).unwrap();
    let compiled = info["compiledUri"].as_str().expect("compiled uri");
    assert!(
        compiled.ends_with("target/compiled/t/models/m.sql"),
        "{compiled}"
    );

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn compiled_path_math_is_pure() {
    let root = Path::new("/work/proj");
    assert_eq!(
        compiled_sql_path(root, "jaffle", Path::new("/work/proj/models/marts/m.sql")),
        Path::new("/work/proj/target/compiled/jaffle/models/marts/m.sql")
    );
    // a path outside the root is appended as-is (open_compiled's behavior)
    assert_eq!(
        compiled_sql_path(root, "jaffle", Path::new("elsewhere/m.sql")),
        Path::new("/work/proj/target/compiled/jaffle/elsewhere/m.sql")
    );
}

#[test]
fn info_carries_description_and_tags() {
    let ctx = ctx();
    let info =
        serde_json::to_value(model_info(&ctx, "", Some("customers")).expect("info")).unwrap();
    assert!(
        info["tags"]
            .as_array()
            .is_some_and(|t| t.iter().any(|tag| tag == "nightly")),
        "{info}"
    );
    assert!(info["description"].is_string(), "{info}");
}

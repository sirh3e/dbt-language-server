//! Model governance over a temp project: private-ref warnings across
//! groups, contract column drift, and unit_tests surfacing through
//! dbt.modelTests.

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, lineage, Documents};

/// A minimal project: a private `core_users` in group `core`, a
/// consumer outside the group, and a contracted model with drift.
fn temp_project() -> PathBuf {
    let root = std::env::temp_dir().join(format!("cogitator-governance-{}", std::process::id()));
    let models = root.join("models");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(
        root.join("dbt_project.yml"),
        "name: gov\nprofile: gov\nmodel-paths: [\"models\"]\n",
    )
    .unwrap();
    std::fs::write(models.join("core_users.sql"), "select 1 as id").unwrap();
    std::fs::write(
        models.join("outsider.sql"),
        "select * from {{ ref('core_users') }}",
    )
    .unwrap();
    std::fs::write(
        models.join("insider.sql"),
        "select * from {{ ref('core_users') }}",
    )
    .unwrap();
    std::fs::write(
        models.join("contracted.sql"),
        "select 1 as id, 2 as amount from x",
    )
    .unwrap();
    std::fs::write(
        models.join("schema.yml"),
        r#"version: 2
models:
  - name: core_users
    access: private
    group: core
  - name: insider
    group: core
  - name: contracted
    config:
      contract:
        enforced: true
    columns:
      - name: id
      - name: ghost_column
unit_tests:
  - name: contracted_handles_empty_input
    model: contracted
    given: []
    expect:
      rows: []
"#,
    )
    .unwrap();
    root
}

#[test]
fn governance_surface_end_to_end() {
    let root = temp_project();
    let ctx = build_context(root.to_str().unwrap());

    // parsing carried the governance keys through
    let core = &ctx.model_detail_map["core_users"];
    assert_eq!(core.access.as_deref(), Some("private"));
    assert_eq!(core.group.as_deref(), Some("core"));
    assert!(ctx.model_detail_map["contracted"].contract_enforced);

    let mut docs = Documents::default();
    let open = |docs: &mut Documents, name: &str| {
        let path = root.join("models").join(name);
        let uri = cogitator_lsp::fs_path_to_uri(&path);
        docs.open(
            uri.clone(),
            &std::fs::read_to_string(&path).unwrap(),
            &ctx.dialect,
        );
        uri
    };

    // a ref from outside the group warns at the ref token…
    let outsider = open(&mut docs, "outsider.sql");
    let diags = features::private_ref_diagnostics(&ctx, docs.get(&outsider).unwrap(), &outsider);
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].code, "private-ref");
    assert!(diags[0].message.contains("private to group 'core'"));

    // …an intra-group ref stays silent
    let insider = open(&mut docs, "insider.sql");
    let diags = features::private_ref_diagnostics(&ctx, docs.get(&insider).unwrap(), &insider);
    assert!(diags.is_empty(), "{diags:?}");

    // contract drift reports both directions
    let contracted = open(&mut docs, "contracted.sql");
    let doc = docs.get(&contracted).unwrap();
    let lin = lineage::column_lineage(&doc.parsed, &ctx.dialect).expect("lineage");
    let diags = features::contract_diagnostics(&ctx, &contracted, &lin);
    let messages: Vec<&str> = diags.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(diags.len(), 2, "{messages:?}");
    assert!(messages.iter().any(|m| m.contains("ghost_column")));
    assert!(messages.iter().any(|m| m.contains("amount")));
    assert!(diags.iter().all(|d| d.code == "contract-drift"));

    // the unit test rides along in dbt.modelTests, flagged as unit
    let (model, tests) = features::model_tests(&ctx, &contracted).expect("tests");
    assert_eq!(model, "contracted");
    assert!(
        tests
            .iter()
            .any(|t| t.unit && t.name == "contracted_handles_empty_input"),
        "{tests:?}"
    );

    std::fs::remove_dir_all(&root).ok();
}

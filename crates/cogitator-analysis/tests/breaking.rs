//! The breaking-change preview over a temp project: a staging model
//! drops and renames columns against its built schema, and the report
//! names the downstream columns, tests and contracts it breaks.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cogitator_analysis::artifacts::{catalog, manifest, ArtifactSet};
use cogitator_analysis::features::breaking::{self, BaselineSource, ChangeKind};
use cogitator_analysis::{build_context, lineage, Documents};

fn temp_project() -> PathBuf {
    let root = std::env::temp_dir().join(format!("cogitator-breaking-{}", std::process::id()));
    let models = root.join("models");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(
        root.join("dbt_project.yml"),
        "name: brk\nprofile: brk\nmodel-paths: [\"models\"]\n",
    )
    .unwrap();
    // id kept, first_name renamed, nickname dropped
    std::fs::write(
        models.join("stg_people.sql"),
        "select id, given_name from raw_people",
    )
    .unwrap();
    std::fs::write(
        models.join("people.sql"),
        "select p.id, p.first_name, p.nickname from {{ ref('stg_people') }} as p",
    )
    .unwrap();
    std::fs::write(
        models.join("schema.yml"),
        r#"version: 2
models:
  - name: stg_people
    access: public
    columns:
      - name: id
        tests: [unique]
  - name: people
    config:
      contract:
        enforced: true
    columns:
      - name: id
      - name: first_name
        tests: [not_null]
      - name: nickname
"#,
    )
    .unwrap();
    root
}

fn artifacts() -> ArtifactSet {
    let catalog = catalog::parse(
        r#"{"metadata": {"generated_at": "2026-11-20T10:00:00Z"},
            "nodes": {"model.brk.stg_people": {"metadata": {"name": "stg_people"},
              "columns": {
                "id": {"name": "id", "type": "INTEGER", "index": 1},
                "first_name": {"name": "first_name", "type": "VARCHAR", "index": 2},
                "nickname": {"name": "nickname", "type": "VARCHAR", "index": 3}}}},
            "sources": {}}"#,
        Path::new("catalog.json"),
    )
    .unwrap();
    ArtifactSet {
        catalog: Some(Arc::new(catalog)),
        ..ArtifactSet::default()
    }
}

#[test]
fn breaking_changes_end_to_end() {
    let root = temp_project();
    let ctx = build_context(root.to_str().unwrap());
    let mut docs = Documents::default();
    let mut lineage_of = |name: &str| {
        let path = root.join("models").join(format!("{name}.sql"));
        let uri = cogitator_lsp::fs_path_to_uri(&path);
        docs.open(
            uri.clone(),
            &std::fs::read_to_string(&path).unwrap(),
            &ctx.dialect,
        );
        lineage::column_lineage(&docs.get(&uri).unwrap().parsed, &ctx.dialect).expect("lineage")
    };
    let current = breaking::current_columns(&lineage_of("stg_people"), &|_| None).unwrap();
    let lineages = HashMap::from([("people".to_string(), lineage_of("people"))]);

    let set = artifacts();
    let base = breaking::baseline("stg_people", &set).expect("catalog baseline");
    assert_eq!(base.source, BaselineSource::Catalog);
    let report = breaking::report(&ctx, "stg_people", &base, &current, &lineages);

    assert_eq!(report.breaking(), 2, "{report:#?}");
    assert!(report.public);
    let renamed: Vec<_> = report
        .diff
        .removed
        .iter()
        .map(|r| (r.name.as_str(), r.possibly_renamed_to.as_deref()))
        .collect();
    assert_eq!(
        renamed,
        [("first_name", Some("given_name")), ("nickname", None)]
    );
    let first = &report.impacts[0];
    assert_eq!(first.change, ChangeKind::Removed);
    assert_eq!(first.downstream.len(), 1, "{first:?}");
    assert!(first.downstream[0].contracted);
    assert_eq!(first.downstream[0].tests, ["not_null"]);

    let diags = breaking::diagnostics(&report);
    assert_eq!(diags.len(), 2);
    assert!(
        diags.iter().all(|d| d.severity == 1),
        "contracted downstream → error"
    );
    assert!(diags[0]
        .message
        .contains("possibly renamed to 'given_name'"));
    assert!(
        diags[0].message.contains("breaks 1 model, 1 test"),
        "{}",
        diags[0].message
    );
    let related = diags[0].related_information.as_ref().expect("related");
    assert!(
        related
            .iter()
            .any(|r| r.message.starts_with("read by people.first_name")),
        "{related:?}"
    );
    let lens = breaking::lens(&report, "file:///x.sql").expect("lens");
    assert_eq!(lens.command.title, "2 breaking changes");

    // the manifest's documented columns are the fallback baseline
    let manifest = manifest::parse(
        r#"{"nodes": {"model.brk.stg_people": {"resource_type": "model", "name": "stg_people",
             "columns": {"id": {"name": "id", "data_type": "int"}}}}}"#,
        Path::new("manifest.json"),
    )
    .unwrap();
    let set = ArtifactSet {
        manifest: Some(Arc::new(manifest)),
        ..ArtifactSet::default()
    };
    let base = breaking::baseline("stg_people", &set).expect("manifest baseline");
    assert_eq!(base.source, BaselineSource::Manifest);
    let report = breaking::report(&ctx, "stg_people", &base, &current, &lineages);
    assert_eq!(report.breaking(), 0);
    assert!(breaking::lens(&report, "file:///x.sql").is_none());

    // no artifacts at all: no baseline, silence
    assert!(breaking::baseline("stg_people", &ArtifactSet::default()).is_none());
    std::fs::remove_dir_all(&root).ok();
}

//! The column reach over a made-up temp project: a staging column feeds
//! a downstream model, a semantic model's measure, the metrics over it,
//! an exposure (node level) and two dashboards of a BI manifest.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cogitator_analysis::artifacts::bi_manifest;
use cogitator_analysis::features::{self, column_reach};
use cogitator_analysis::{build_context, lineage, Documents};

fn temp_project() -> PathBuf {
    let root = std::env::temp_dir().join(format!("cogitator-reach-{}", std::process::id()));
    let models = root.join("models");
    std::fs::create_dir_all(&models).unwrap();
    std::fs::write(
        root.join("dbt_project.yml"),
        "name: reach\nprofile: reach\nmodel-paths: [\"models\"]\n",
    )
    .unwrap();
    std::fs::write(
        models.join("stg_sales.sql"),
        "select id, amount, region from raw_sales",
    )
    .unwrap();
    std::fs::write(
        models.join("sales.sql"),
        "select s.id, s.amount as gross, s.region from {{ ref('stg_sales') }} as s",
    )
    .unwrap();
    std::fs::write(
        models.join("schema.yml"),
        r#"version: 2
models:
  - name: stg_sales
    columns:
      - name: amount
        tests: [not_null]
  - name: sales
    columns:
      - name: gross
        tests: [not_null, positive]
semantic_models:
  - name: sales_sm
    model: ref('sales')
    entities:
      - name: id
        type: primary
    measures:
      - name: gross_total
        expr: gross
        agg: sum
      - name: sale_count
        expr: "1"
        agg: sum
    dimensions:
      - name: region
        type: categorical
metrics:
  - name: revenue
    type: simple
    type_params:
      measure: gross_total
  - name: revenue_share
    type: ratio
    type_params:
      numerator: revenue
      denominator: sales_count_metric
  - name: sales_count_metric
    type: simple
    type_params:
      measure: sale_count
exposures:
  - name: weekly_report
    type: dashboard
    owner: {name: someone}
    depends_on: [ref('sales')]
"#,
    )
    .unwrap();
    root
}

#[test]
fn a_column_reaches_semantic_models_metrics_and_dashboards() {
    let root = temp_project();
    let ctx = build_context(root.to_str().unwrap());
    let mut docs = Documents::default();
    let path = root.join("models").join("sales.sql");
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    docs.open(
        uri.clone(),
        &std::fs::read_to_string(&path).unwrap(),
        &ctx.dialect,
    );
    let sales = lineage::column_lineage(&docs.get(&uri).unwrap().parsed, &ctx.dialect).unwrap();
    let lineages = HashMap::from([("sales".to_string(), sales)]);
    let bi = bi_manifest::parse(
        r#"{"dashboards":[
          {"name":"Revenue board","url":"https://bi.example/1","reads":[{"model":"sales","column":"gross"}]},
          {"name":"Regions","reads":[{"model":"sales","column":"region"}]},
          {"name":"Everything","reads":[{"model":"stg_sales"}]}
        ]}"#,
        Path::new("bi.json"),
    )
    .unwrap();
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let corpus = column_reach::Corpus::read(&ctx, &read);

    let report = features::impact(&ctx, "stg_sales", Some("amount"), &lineages);
    let reach = column_reach::reach(&ctx, &report, &corpus, Some(&bi)).expect("a column");
    let tests: Vec<_> = reach
        .tests
        .iter()
        .map(|t| format!("{}.{}:{}", t.model, t.column, t.test))
        .collect();
    assert_eq!(
        tests,
        [
            "stg_sales.amount:not_null",
            "sales.gross:not_null",
            "sales.gross:positive"
        ]
    );
    let semantic: Vec<_> = reach
        .semantic_items
        .iter()
        .map(|s| (s.semantic_model.as_str(), s.name.as_deref()))
        .collect();
    assert_eq!(semantic, [("sales_sm", Some("gross_total"))]);
    let metrics: Vec<_> = reach
        .metrics
        .iter()
        .map(|m| (m.name.as_str(), m.via.as_str()))
        .collect();
    assert_eq!(
        metrics,
        [
            ("revenue", "measure:gross_total"),
            ("revenue_share", "metric:revenue")
        ]
    );
    assert_eq!(reach.exposures.len(), 1);
    let dashboards: Vec<_> = reach
        .dashboards
        .iter()
        .map(|d| (d.name.as_str(), d.level))
        .collect();
    assert_eq!(
        dashboards,
        [
            ("Revenue board", column_reach::Level::Column),
            ("Everything", column_reach::Level::Node)
        ]
    );
    assert_eq!(
        reach.summary,
        "read by 1 model, 3 tests, 1 semantic model, 2 dashboards, 2 metrics, 1 exposure (node level)"
    );
    let report = report.with_reach(Some(reach));
    assert!(report
        .summary
        .starts_with("stg_sales.amount feeds 1 model(s)"));

    // every column at once, for the hover cache
    let all = column_reach::model_reach(
        &ctx,
        "stg_sales",
        &["id".into(), "amount".into(), "region".into()],
        &lineages,
        &corpus,
        Some(&bi),
    );
    assert_eq!(all.column("AMOUNT").unwrap().semantic_models, 1);
    let region = all.column("region").unwrap();
    assert_eq!(region.semantic_items[0].name.as_deref(), Some("region"));
    assert_eq!(region.dashboards.len(), 2, "{region:?}");
    let _ = std::fs::remove_dir_all(&root);
}

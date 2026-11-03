//! Pure-feature coverage for the editor surfaces the feature batch
//! added: document links, linked editing, inlay hints, the convert
//! catalog, artifact lenses and the impact summary — all over the
//! shared fixture context, no server process.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use cogitator_analysis::artifacts::{run_results, ArtifactSet};
use cogitator_analysis::features::{self, RelationCatalog, TargetRelation};
use cogitator_analysis::{build_context, Documents};
use cogitator_lsp::{Position, Range};

fn ctx() -> cogitator_analysis::DbtContext {
    build_context(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/jaffle_shop_duckdb")
            .to_str()
            .unwrap(),
    )
}

fn target() -> TargetRelation {
    TargetRelation {
        database: Some("jaffle_shop".into()),
        schema: Some("main".into()),
    }
}

#[test]
fn document_links_target_definitions() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = "file:///p/models/links.sql";
    docs.open(
        uri.into(),
        "select * from {{ ref('customers') }} join {{ source('stripe', 'payments') }} using (id)",
        &ctx.dialect,
    );
    let links = features::document_links(&ctx, docs.get(uri).unwrap());
    let targets: Vec<&str> = links.iter().map(|l| l.target.as_str()).collect();
    assert!(
        targets.iter().any(|t| t.ends_with("customers.sql")),
        "{targets:?}"
    );
    // source name AND table both link into the declaring YAML
    assert_eq!(
        targets.iter().filter(|t| t.ends_with("schema.yml")).count(),
        2,
        "{targets:?}"
    );
}

#[test]
fn linked_editing_is_cte_only() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = "file:///p/models/le.sql";
    docs.open(
        uri.into(),
        "with totals as (select 1 as v) select * from totals",
        &ctx.dialect,
    );
    let doc = docs.get(uri).unwrap();
    let on_cte = features::linked_editing(
        doc,
        Position {
            line: 0,
            character: 6,
        },
    );
    assert_eq!(on_cte.map(|l| l.ranges.len()), Some(2));
    // a plain column name never links
    let on_column = features::linked_editing(
        doc,
        Position {
            line: 0,
            character: 28,
        },
    );
    assert!(on_column.is_none());
}

#[test]
fn inlay_hints_show_relations_and_macro_params() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = "file:///p/models/hints.sql";
    docs.open(
        uri.into(),
        "select {{ full_name('a', last_name='b') }} from {{ ref('customers') }}",
        &ctx.dialect,
    );
    let target = target();
    let hints = features::inlay_hints(
        &ctx,
        docs.get(uri).unwrap(),
        uri,
        Range::default(),
        Some(&target),
    );
    let labels: Vec<&str> = hints.iter().map(|h| h.label.as_str()).collect();
    assert!(
        labels.contains(&"→ jaffle_shop.main.customers"),
        "{labels:?}"
    );
    assert!(labels.contains(&"first_name:"), "{labels:?}");
    // the named argument gets NO positional hint
    assert!(!labels.contains(&"last_name:"), "{labels:?}");
}

#[test]
fn convert_catalog_resolves_both_spellings() {
    let ctx = ctx();
    let target = target();
    let catalog = RelationCatalog::build(&ctx, Some(&target));
    let mut docs = Documents::default();
    let uri = "file:///p/models/conv.sql";
    docs.open(
        uri.into(),
        "select * from jaffle_shop.main.customers join raw.jaffle_shop.orders using (id)",
        &ctx.dialect,
    );
    let actions = features::convert_literal_relations(docs.get(uri).unwrap(), uri, &catalog);
    let titles: Vec<&str> = actions.iter().map(|a| a.title.as_str()).collect();
    assert!(
        titles.contains(&"Convert to ref('customers')"),
        "{titles:?}"
    );
    // the fixture's jaffle_shop source declares database raw
    assert!(
        titles.contains(&"Convert to source('jaffle_shop', 'orders')"),
        "{titles:?}"
    );
}

#[test]
fn artifact_lenses_render_status_and_freshness() {
    let ctx = ctx();
    let set = ArtifactSet {
        run_results: Some(Arc::new(
            run_results::parse(
                r#"{"results": [{"unique_id": "model.jaffle_shop.customers",
                                "status": "success", "execution_time": 1.25}]}"#,
                std::path::Path::new("x"),
            )
            .unwrap(),
        )),
        manifest: None,
        freshness: Some(Arc::new(
            cogitator_analysis::artifacts::freshness::parse(
                r#"{"results": [{"unique_id": "source.jaffle_shop.stripe.payments",
                                "status": "warn"}]}"#,
                std::path::Path::new("y"),
            )
            .unwrap(),
        )),
    };
    let model_uri = cogitator_lsp::fs_path_to_uri(&ctx.model_detail_map["customers"].uri);
    let lens = features::run_lens(&ctx, &model_uri, &set).expect("run lens");
    assert_eq!(lens.command.title, "last run: OK in 1.2 s");
    assert!(lens.command.command.is_empty(), "informational");

    let schema_uri =
        cogitator_lsp::fs_path_to_uri(&ctx.source_detail_map["stripe"].tables["payments"].uri);
    let lenses = features::freshness_lenses(&ctx, &schema_uri, &set);
    assert_eq!(lenses.len(), 1, "{lenses:?}");
    assert_eq!(lenses[0].command.title, "freshness: warn");
}

#[test]
fn impact_summary_counts_models_and_exposures() {
    let ctx = ctx();
    let report = features::impact(&ctx, "stg_customers", None, &HashMap::new());
    assert!(
        report.models.iter().any(|m| m.name == "customers"),
        "{report:?}"
    );
    assert!(
        report.summary.starts_with("stg_customers feeds"),
        "{}",
        report.summary
    );
    assert!(!report.truncated);
    // an unknown model yields an empty, honest report
    let empty = features::impact(&ctx, "nope", None, &HashMap::new());
    assert!(empty.models.is_empty());
}

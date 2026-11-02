//! Editor-experience features: column hover/definition, model rename,
//! code lenses, structured macro signature help.

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, Documents};
use cogitator_lsp::Position;
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn ctx() -> cogitator_analysis::DbtContext {
    build_context(fixture_root().to_str().unwrap())
}

#[test]
fn column_hover_shows_description_and_lineage() {
    let ctx = ctx();
    let text = "select customer_id from {{ ref('customers') }}";
    let mut docs = Documents::default();
    docs.open("file:///h.sql".into(), text, &ctx.dialect);
    let doc = docs.get("file:///h.sql").unwrap();
    let col = text.find("customer_id").unwrap() as u32;

    let hover = features::hover(
        &ctx,
        doc,
        Position {
            line: 0,
            character: col + 1,
        },
    );
    assert!(hover.starts_with("Column of customers"), "{hover}");
    assert!(hover.contains("unique identifier"), "{hover}");
    assert!(hover.contains("Lineage: customers.customer_id"), "{hover}");
}

#[test]
fn column_definition_jumps_to_yaml_entry() {
    let ctx = ctx();
    let text = "select customer_id from {{ ref('customers') }}";
    let mut docs = Documents::default();
    docs.open("file:///d.sql".into(), text, &ctx.dialect);
    let doc = docs.get("file:///d.sql").unwrap();
    let col = text.find("customer_id").unwrap() as u32;

    let location = features::definition(
        &ctx,
        doc,
        "file:///d.sql",
        Position {
            line: 0,
            character: col + 1,
        },
    );
    assert!(location.uri.ends_with("schema.yml"), "{location:?}");
    assert!(location.range.start.line > 0, "a real yaml position");
}

#[test]
fn rename_model_produces_full_workspace_edit() {
    let ctx = ctx();
    // rename stg_orders from inside a dependent, cursor on the ref
    let text = "select * from {{ ref('stg_orders') }}";
    let mut docs = Documents::default();
    docs.open("file:///r.sql".into(), text, &ctx.dialect);
    let doc = docs.get("file:///r.sql");
    let col = text.find("stg_orders").unwrap() as u32;

    let edit = features::rename_model(
        &ctx,
        doc,
        "file:///r.sql",
        Position {
            line: 0,
            character: col + 1,
        },
        "stg_orders_v2",
    )
    .expect("workspace edit");
    let changes = edit["documentChanges"].as_array().expect("documentChanges");

    // dependents rewritten (customers.sql refs stg_orders in the fixture)
    let text_edits: Vec<&serde_json::Value> = changes
        .iter()
        .filter(|c| c.get("edits").is_some())
        .collect();
    assert!(
        text_edits.iter().any(|c| c["textDocument"]["uri"]
            .as_str()
            .unwrap()
            .ends_with("customers.sql")),
        "dependent rewritten: {changes:?}"
    );
    // yaml entry updated
    assert!(text_edits
        .iter()
        .any(|c| c["textDocument"]["uri"].as_str().unwrap().ends_with(".yml")));
    // file rename comes last
    let last = changes.last().unwrap();
    assert_eq!(last["kind"], "rename");
    assert!(last["newUri"]
        .as_str()
        .unwrap()
        .ends_with("stg_orders_v2.sql"));
    // rejects non-identifier names
    assert!(features::rename_model(
        &ctx,
        doc,
        "file:///r.sql",
        Position {
            line: 0,
            character: col
        },
        "bad name"
    )
    .is_none());
}

#[test]
fn code_lens_batch_for_models_and_macros() {
    let ctx = ctx();
    let stg = fixture_root().join("models/staging/stg_orders.sql");
    let lenses = features::code_lenses(&ctx, &cogitator_lsp::fs_path_to_uri(&stg));
    let titles: Vec<&str> = lenses.iter().map(|l| l.command.title.as_str()).collect();
    assert!(
        titles.iter().any(|t| t.contains("downstream")),
        "{titles:?}"
    );
    assert!(titles.iter().any(|t| t.contains("upstream")), "{titles:?}");
    for expected in [
        "Go to schema",
        "Column lineage",
        "dbt run",
        "dbt test",
        "dbt compile",
    ] {
        assert!(titles.contains(&expected), "{expected} in {titles:?}");
    }
    let run = lenses
        .iter()
        .find(|l| l.command.title == "dbt run")
        .unwrap();
    assert_eq!(run.command.command, "dbt.run");
    assert!(run.command.arguments[0]["uri"]
        .as_str()
        .unwrap()
        .ends_with("stg_orders.sql"));

    // macro files: one call-site lens per macro, anchored at its signature
    let macro_file = ctx
        .macro_detail_map
        .get("jaffle_shop")
        .and_then(|m| m.values().next())
        .map(|m| m.uri.clone())
        .expect("a macro file");
    let macro_lenses = features::code_lenses(&ctx, &cogitator_lsp::fs_path_to_uri(&macro_file));
    assert!(!macro_lenses.is_empty());
    assert!(
        macro_lenses
            .iter()
            .all(|l| l.command.title.contains("call site")),
        "{macro_lenses:?}"
    );

    // non-model files get none
    assert!(features::code_lenses(&ctx, "file:///tmp/nope.sql").is_empty());
}

#[test]
fn graph_upstream_downstream_and_macro_call_sites() {
    let ctx = ctx();
    let downstream: Vec<&str> = ctx
        .graph
        .downstream_of("stg_orders")
        .map(|n| n.name.as_str())
        .collect();
    assert!(downstream.contains(&"customers"), "{downstream:?}");
    let upstream: Vec<&str> = ctx
        .graph
        .upstream_of("customers")
        .map(|n| n.name.as_str())
        .collect();
    assert!(upstream.contains(&"stg_orders"), "{upstream:?}");
    assert!(ctx.graph.macro_call_sites("full_name").count() >= 1);
}

#[test]
fn signature_help_uses_structured_params_with_defaults() {
    let ctx = ctx();
    let macros = ctx.macro_detail_map.get("jaffle_shop").expect("own macros");
    // the fixture's cents_to_dollars-style macro has a defaulted parameter
    let with_default = macros
        .values()
        .find(|m| m.params.iter().any(|p| p.default.is_some()));
    if let Some(details) = with_default {
        let text = format!("select {{{{ {}(col, ", details.name);
        let mut docs = Documents::default();
        docs.open("file:///s.sql".into(), &text, &ctx.dialect);
        let doc = docs.get("file:///s.sql").unwrap();
        let help = features::signature_help(
            &ctx,
            doc,
            Position {
                line: 0,
                character: text.len() as u32,
            },
        )
        .expect("signature help");
        let labels: Vec<&str> = help.signatures[0]
            .parameters
            .iter()
            .map(|p| p.label.as_str())
            .collect();
        assert!(
            labels.iter().any(|l| l.contains('=')),
            "default shown: {labels:?}"
        );
        assert_eq!(help.active_parameter, 1);
    } else {
        // every fixture macro is default-free: params must still be structured
        assert!(macros.values().all(|m| !m.params.is_empty()));
    }
}

#[test]
fn model_graph_direct_neighbors() {
    let ctx = build_context(fixture_root().to_str().unwrap());

    let graph = features::model_graph(&ctx, "", Some("customers")).expect("customers graph");
    assert_eq!(graph["model"], "customers");
    assert!(graph["uri"].as_str().unwrap().ends_with("customers.sql"));
    let upstream: Vec<&str> = graph["upstream"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(upstream.contains(&"stg_customers"), "{upstream:?}");

    // re-center by name (no uri): the neighbor's downstream points back
    let stg = features::model_graph(&ctx, "", Some("stg_customers")).expect("stg graph");
    let down: Vec<&str> = stg["downstream"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(down.contains(&"customers"), "{down:?}");
    // model entries carry navigable uris and the model kind
    assert!(stg["downstream"]
        .as_array()
        .unwrap()
        .iter()
        .all(|e| e["kind"] == "model" && e["uri"].is_string()));

    // unknown model → no payload (the dispatcher answers null)
    assert!(features::model_graph(&ctx, "", Some("nope")).is_none());
}

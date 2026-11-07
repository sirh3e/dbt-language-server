//! Context building + features over the shared testdata fixture
//! (ports the intent of the Go repo's state_test.go / state_schema_test.go).

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, Documents};
use cogitator_dialects::Dialect;
use cogitator_lsp::Position;
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn ctx() -> cogitator_analysis::DbtContext {
    build_context(fixture_root().to_str().unwrap())
}

#[test]
fn project_file_and_dialect() {
    let ctx = ctx();
    assert_eq!(ctx.project.name, "jaffle_shop");
    assert_eq!(ctx.project.profile, "jaffle_shop");
    assert_eq!(ctx.dialect, Dialect::new("duckdb"));
    assert!(ctx.project_root.is_some());
}

#[test]
fn models_seeds_and_packages_discovered() {
    let ctx = ctx();
    let models = &ctx.model_detail_map;
    assert!(
        models.contains_key("customers"),
        "customers model: {:?}",
        models.keys()
    );
    assert!(models.contains_key("stg_orders"));
    // seeds get the fixed description
    let seed = models
        .values()
        .find(|m| m.description == "Seed File")
        .expect("at least one seed");
    assert_eq!(seed.uri.extension().unwrap(), "csv");
    // installed package models carry their own project name
    assert!(
        models.values().any(|m| m.project_name == "jaffle_package"),
        "package models present"
    );
}

#[test]
fn macros_and_variables_collected() {
    let ctx = ctx();
    let own = ctx.macro_detail_map.get("jaffle_shop").expect("own macros");
    assert!(
        own.values().any(|m| m.description.contains('(')),
        "macro signatures captured: {:?}",
        own.keys()
    );
    assert!(!ctx.variable_detail_map.is_empty(), "project vars parsed");
}

#[test]
fn graph_has_ref_edges() {
    let ctx = ctx();
    let edges = ctx.graph.sorted_edges();
    assert!(
        edges.iter().any(|(from, kind, to)| from == "customers"
            && *kind == "ref"
            && to.starts_with("stg_")),
        "customers refs staging models: {edges:?}"
    );
}

#[test]
fn hover_and_definition_on_a_real_model() {
    let ctx = ctx();
    let model_path = fixture_root().join("models/customers.sql");
    let text = std::fs::read_to_string(&model_path).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&model_path);

    let mut docs = Documents::default();
    docs.open(uri.clone(), &text, &ctx.dialect);
    let doc = docs.get(&uri).unwrap();

    // find a ref('...') target in the real file
    let ref_target = ctx
        .graph
        .sorted_edges()
        .into_iter()
        .find(|(from, kind, _)| from == "customers" && *kind == "ref")
        .map(|(_, _, to)| to)
        .expect("customers has refs");
    let offset = text.find(&ref_target).unwrap();
    let line = text[..offset].matches('\n').count() as u32;
    let col = (offset - text[..offset].rfind('\n').map(|p| p + 1).unwrap_or(0)) as u32;

    let location = features::definition(
        &ctx,
        doc,
        &uri,
        Position {
            line,
            character: col + 1,
        },
    );
    assert!(
        location.uri.ends_with(&format!("{ref_target}.sql")),
        "definition jumps to the model file: {location:?}"
    );

    // go-to-schema for the current file works without a cursor hit
    let schema = features::go_to_schema(&ctx, Some(doc), &uri, Position::default());
    if let Some(schema) = schema {
        assert!(schema.uri.ends_with(".yml"));
    }
}

#[test]
fn completion_inside_ref_mirrors_suffix() {
    let ctx = ctx();
    let uri = "file:///tmp/inline.sql".to_string();
    let text = "select * from {{ ref('";
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    let doc = docs.get(&uri).unwrap();

    let items = features::completion(
        &ctx,
        doc,
        Position {
            line: 0,
            character: text.len() as u32,
        },
    );
    assert!(!items.is_empty());
    let customers = items
        .iter()
        .find(|i| i.label == "customers")
        .expect("customers item");
    assert_eq!(customers.insert_text, "customers') }}");
    assert_eq!(customers.detail, "Project: jaffle_shop");
}

#[test]
fn incremental_change_reparses_and_malformed_keeps_last_good() {
    let ctx = ctx();
    let uri = "file:///tmp/change.sql".to_string();
    let mut docs = Documents::default();
    docs.open(
        uri.clone(),
        "select * from {{ ref('stg_orders') }}",
        &ctx.dialect,
    );

    // break the template mid-edit: last good parse must survive
    docs.change(
        &uri,
        &[cogitator_lsp::ContentChange {
            range: cogitator_lsp::Range {
                start: Position {
                    line: 0,
                    character: 14,
                },
                end: Position {
                    line: 0,
                    character: 16,
                },
            },
            range_length: 0,
            text: "{{".into(),
        }],
        &ctx.dialect,
    );
    let doc = docs.get(&uri).unwrap();
    assert!(doc
        .parsed
        .tokens
        .iter()
        .any(|t| t.kind == cogitator_parser::TokenKind::Ref));
}

#[test]
fn source_relation_coordinates_resolved() {
    let ctx = ctx();
    let jaffle = ctx
        .source_detail_map
        .get("jaffle_shop")
        .expect("jaffle_shop source");
    assert_eq!(jaffle.database.as_deref(), Some("raw"));
    assert_eq!(jaffle.schema.as_deref(), Some("jaffle_shop"));
    assert_eq!(jaffle.loader.as_deref(), Some("raw_ingest"));

    // stripe declares none of the coordinates: all stay None
    let stripe = ctx.source_detail_map.get("stripe").expect("stripe source");
    assert_eq!(stripe.database, None);
    assert_eq!(stripe.schema, None);
    assert_eq!(stripe.loader, None);
    let payments = stripe.tables.get("payments").expect("payments table");
    assert_eq!(payments.identifier, None);
}

#[test]
fn doc_blocks_land_on_the_context_with_positions() {
    let ctx = ctx();
    let block = ctx
        .doc_detail_map
        .get("orders_status")
        .expect("docs block from models/docs.md");
    assert!(block.uri.ends_with("models/docs.md"), "{:?}", block.uri);
    assert!(block.content.contains("placed"), "{}", block.content);
    assert_eq!(block.project_name, "jaffle_shop");
    // `{% docs orders_status %}` opens the file: the name sits at col 8
    assert_eq!(
        (block.range.start.line, block.range.start.character),
        (0, 8)
    );
}

#[test]
fn hover_and_definition_on_a_doc_reference() {
    let ctx = ctx();
    let schema_path = fixture_root().join("models/schema.yml");
    let text = std::fs::read_to_string(&schema_path).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&schema_path);

    let mut docs = Documents::default();
    docs.open(uri.clone(), &text, &ctx.dialect);
    let doc = docs.get(&uri).unwrap();

    let offset = text.find("orders_status").expect("doc() ref in schema.yml");
    let line = text[..offset].matches('\n').count() as u32;
    let col = (offset - text[..offset].rfind('\n').map(|p| p + 1).unwrap_or(0)) as u32;
    let pos = Position {
        line,
        character: col + 2,
    };

    let hover = features::hover(&ctx, doc, pos);
    assert!(hover.contains("placed"), "{hover}");

    let location = features::definition(&ctx, doc, &uri, pos);
    assert!(location.uri.ends_with("docs.md"), "{}", location.uri);
    assert_eq!(
        (location.range.start.line, location.range.start.character),
        (0, 8)
    );
}

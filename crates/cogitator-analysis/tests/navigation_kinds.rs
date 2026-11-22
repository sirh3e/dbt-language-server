//! References, rename, call hierarchy and document links for sources,
//! macros, docs blocks and columns, over the jaffle fixture.

use std::path::PathBuf;

use cogitator_analysis::{build_context, features, Documents};
use cogitator_lsp::{Location, Position};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn uri(rel: &str) -> String {
    cogitator_lsp::fs_path_to_uri(&root().join(rel))
}

/// Open a fixture file as the editor would, and the position of the
/// `nth` occurrence of `needle` (plus `offset` characters).
fn open(docs: &mut Documents, rel: &str, needle: &str, offset: u32) -> (String, Position) {
    let text = std::fs::read_to_string(root().join(rel)).unwrap();
    let ctx_dialect = cogitator_dialects::Dialect::default();
    let u = uri(rel);
    docs.open(u.clone(), &text, &ctx_dialect);
    let at = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} in {rel}"));
    let line = text[..at].matches('\n').count() as u32;
    let col = (at - text[..at].rfind('\n').map_or(0, |i| i + 1)) as u32;
    (
        u,
        Position {
            line,
            character: col + offset,
        },
    )
}

fn files(locations: &[Location]) -> Vec<String> {
    let mut out: Vec<String> = locations
        .iter()
        .map(|l| l.uri.rsplit('/').next().unwrap().to_string())
        .collect();
    out.dedup();
    out
}

#[test]
fn source_references_with_and_without_the_declaration() {
    let ctx = build_context(root().to_str().unwrap());
    let mut docs = Documents::default();
    let (u, pos) = open(
        &mut docs,
        "models/staging/stg_stripe_payments.sql",
        "payments'",
        1,
    );
    let doc = docs.get(&u);
    let uses = features::references_at(&ctx, doc, &u, pos, false);
    assert_eq!(files(&uses), vec!["stg_stripe_payments.sql"], "{uses:?}");
    assert!(uses
        .iter()
        .all(|l| l.range.start.character < l.range.end.character));

    let with_decl = features::references_at(&ctx, doc, &u, pos, true);
    assert_eq!(with_decl.len(), uses.len() + 1);
    assert!(
        files(&with_decl).contains(&"schema.yml".to_string()),
        "{with_decl:?}"
    );

    // the whole source, from its YAML entry
    let (yu, ypos) = open(&mut docs, "models/schema.yml", "name: stripe", 7);
    let from_yaml = features::references_at(&ctx, docs.get(&yu), &yu, ypos, false);
    assert_eq!(
        files(&from_yaml),
        vec!["stg_stripe_payments.sql"],
        "{from_yaml:?}"
    );
}

#[test]
fn macro_references_resolve_qualified_calls() {
    let ctx = build_context(root().to_str().unwrap());
    let mut docs = Documents::default();
    let (u, pos) = open(&mut docs, "models/customers.sql", "add_values", 2);
    let refs = features::references_at(&ctx, docs.get(&u), &u, pos, true);
    let names = files(&refs);
    assert!(names.contains(&"customers.sql".to_string()), "{refs:?}");
    assert!(
        names.contains(&"jaffle_package_macros.sql".to_string()),
        "{refs:?}"
    );

    // from the definition header of a root macro
    let (mu, mpos) = open(&mut docs, "macros/jaffle_macros.sql", "full_name(", 1);
    let refs = features::references_at(&ctx, docs.get(&mu), &mu, mpos, false);
    assert_eq!(files(&refs), vec!["customers.sql"], "{refs:?}");
    let decl = features::references_at(&ctx, docs.get(&mu), &mu, mpos, true);
    assert_eq!(decl.len(), refs.len() + 1);
}

#[test]
fn doc_and_column_references() {
    let ctx = build_context(root().to_str().unwrap());
    let mut docs = Documents::default();
    let (u, pos) = open(&mut docs, "models/docs.md", "orders_status", 2);
    let refs = features::references_at(&ctx, docs.get(&u), &u, pos, false);
    assert_eq!(files(&refs), vec!["schema.yml"], "{refs:?}");
    let decl = features::references_at(&ctx, docs.get(&u), &u, pos, true);
    assert!(files(&decl).contains(&"docs.md".to_string()), "{decl:?}");

    // orders.customer_id: its YAML entry, orders.sql and the dependent
    // customer_orders.sql, whose lineage reads it
    let text = std::fs::read_to_string(root().join("models/schema.yml")).unwrap();
    let orders_at = text.find("- name: orders").unwrap();
    let column_at = orders_at + text[orders_at..].find("- name: customer_id").unwrap() + 8;
    let line = text[..column_at].matches('\n').count() as u32;
    let character = (column_at - text[..column_at].rfind('\n').unwrap() - 1) as u32 + 2;
    let yu = uri("models/schema.yml");
    docs.open(yu.clone(), &text, &ctx.dialect);
    let refs =
        features::references_at(&ctx, docs.get(&yu), &yu, Position { line, character }, true);
    let names = files(&refs);
    for expected in ["customer_orders.sql", "orders.sql", "schema.yml"] {
        assert!(
            names.contains(&expected.to_string()),
            "{expected}: {refs:?}"
        );
    }
}

#[test]
fn macro_rename_edits_definition_and_calls_and_refuses_packages() {
    let ctx = build_context(root().to_str().unwrap());
    let mut docs = Documents::default();
    let (u, pos) = open(&mut docs, "models/customers.sql", "full_name(", 1);
    let doc = docs.get(&u).unwrap();

    let prep = features::prepare_rename_at(&ctx, doc, &u, pos)
        .unwrap()
        .unwrap();
    assert_eq!(prep.placeholder, "full_name");

    let edit = features::rename_at(&ctx, Some(doc), &u, pos, "person_name")
        .unwrap()
        .expect("edit");
    let json = serde_json::to_value(&edit).unwrap();
    let changes = json["documentChanges"].as_array().unwrap();
    let uris: Vec<&str> = changes
        .iter()
        .map(|c| c["textDocument"]["uri"].as_str().unwrap())
        .collect();
    assert!(
        uris.iter().any(|u| u.ends_with("jaffle_macros.sql")),
        "{uris:?}"
    );
    assert!(
        uris.iter().any(|u| u.ends_with("customers.sql")),
        "{uris:?}"
    );
    assert!(
        changes.iter().all(|c| c.get("kind").is_none()),
        "no file rename"
    );

    assert!(features::rename_at(&ctx, Some(doc), &u, pos, "2bad").is_err());
    assert!(features::rename_at(&ctx, Some(doc), &u, pos, "times_five").is_err());

    let (_, pkg) = open(&mut docs, "models/customers.sql", "add_values", 2);
    let doc = docs.get(&u).unwrap();
    let refused = features::prepare_rename_at(&ctx, doc, &u, pkg).unwrap_err();
    assert!(refused.contains("jaffle_package"), "{refused}");
    assert!(features::rename_at(&ctx, Some(doc), &u, pkg, "x").is_err());

    // refs still prepare through the model rename
    let (ru, rpos) = open(&mut docs, "models/customers.sql", "stg_orders", 1);
    let prep = features::prepare_rename_at(&ctx, docs.get(&ru).unwrap(), &ru, rpos)
        .unwrap()
        .unwrap();
    assert_eq!(prep.placeholder, "stg_orders");
}

#[test]
fn model_rename_reaches_plain_yaml_refs() {
    let ctx = build_context(root().to_str().unwrap());
    let mut docs = Documents::default();
    let (u, pos) = open(&mut docs, "analysis/order_counts.sql", "customers", 1);
    let edit = features::rename_model(&ctx, docs.get(&u), &u, pos, "clients").expect("edit");
    let json = serde_json::to_value(&edit).unwrap();
    let schema = json["documentChanges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| {
            c["textDocument"]["uri"]
                .as_str()
                .is_some_and(|u| u.ends_with("models/schema.yml"))
        })
        .expect("schema.yml edited");
    // the `- name: customers` entry and `to: ref('customers')`
    assert_eq!(schema["edits"].as_array().unwrap().len(), 2, "{schema}");
}

#[test]
fn outgoing_calls_and_links() {
    let ctx = build_context(root().to_str().unwrap());
    let mut docs = Documents::default();
    let text = "{% macro outer() %}{{ full_name('a', 'b') }} {{ jaffle_package.add_values(1, 2) }}{% endmacro %}\n{% macro other() %}{{ times_five(1) }}{% endmacro %}";
    let path = root().join("macros/probe_outgoing.sql");
    let item = cogitator_lsp::CallHierarchyItem {
        name: "outer".into(),
        kind: cogitator_lsp::symbol_kind::FUNCTION,
        uri: cogitator_lsp::fs_path_to_uri(&path),
        range: Default::default(),
        selection_range: Default::default(),
    };
    let calls = features::outgoing_in_text(&ctx, &item, &path, text);
    let names: Vec<&str> = calls.iter().map(|c| c.to.name.as_str()).collect();
    assert_eq!(names, vec!["add_values", "full_name"]);

    // prepare from a definition header
    let (mu, mpos) = open(&mut docs, "macros/jaffle_macros.sql", "times_five(", 2);
    let item = features::prepare_call_hierarchy(&ctx, docs.get(&mu).unwrap(), &mu, mpos)
        .expect("item from the header");
    assert_eq!(item.name, "times_five");
    let incoming = features::macro_incoming_calls(&ctx, &item);
    assert!(
        incoming.iter().any(|c| c.from.name == "customers"),
        "{incoming:?}"
    );

    // document links: macros and doc() in SQL/YAML, plain YAML refs
    let (cu, _) = open(&mut docs, "models/customers.sql", "full_name", 0);
    let links = features::document_links_at(&ctx, docs.get(&cu).unwrap(), &cu);
    let tips: Vec<String> = links.iter().filter_map(|l| l.tooltip.clone()).collect();
    assert!(
        tips.contains(&"macro jaffle_shop.full_name".to_string()),
        "{tips:?}"
    );
    assert!(
        tips.contains(&"macro jaffle_package.add_values".to_string()),
        "{tips:?}"
    );
    let (yu, _) = open(&mut docs, "models/schema.yml", "orders_status", 0);
    let links = features::document_links_at(&ctx, docs.get(&yu).unwrap(), &yu);
    let tips: Vec<String> = links.iter().filter_map(|l| l.tooltip.clone()).collect();
    assert!(
        tips.contains(&"docs block orders_status".to_string()),
        "{tips:?}"
    );
    assert!(tips.contains(&"model customers".to_string()), "{tips:?}");
}

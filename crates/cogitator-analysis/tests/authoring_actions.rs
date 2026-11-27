//! The authoring actions over the made-up `testdata/authoring` project:
//! scaffold a staging model, carry YAML along with an extracted CTE,
//! inline a model as a CTE, and inherit descriptions (YAML and SQL
//! side). Each produced edit is applied to the files it names and the
//! results are snapshotted, so a snapshot reads as the files after.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cogitator_analysis::lineage::{column_lineage, ModelLineage};
use cogitator_analysis::{build_context, features, DbtContext, Documents};
use cogitator_lsp::{CodeAction, Position, Range};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/authoring")
}

fn uri(rel: &str) -> String {
    cogitator_lsp::fs_path_to_uri(&root().join(rel))
}

fn ctx() -> DbtContext {
    build_context(root().to_str().unwrap())
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// The open document `rel` and the cursor at the first `needle` in it.
fn open(ctx: &DbtContext, rel: &str, needle: &str) -> (Documents, String, Range) {
    let text = read(&root().join(rel)).unwrap();
    let uri = uri(rel);
    let mut docs = Documents::default();
    docs.open(uri.clone(), &text, &ctx.dialect);
    let (line, column) = text
        .lines()
        .enumerate()
        .find_map(|(i, l)| l.find(needle).map(|c| (i, c)))
        .unwrap_or_else(|| panic!("{needle} in {rel}"));
    let range = Range::zero_width(Position {
        line: line as u32,
        character: column as u32,
    });
    (docs, uri, range)
}

/// Lineage read off each model's file on disk.
fn lineage_of(ctx: &DbtContext) -> impl Fn(&str) -> Option<ModelLineage> + '_ {
    move |model: &str| {
        let details = ctx.model_detail_map.get(model)?;
        let text = read(&details.uri)?;
        let parsed = cogitator_parser::pipeline::parse_document(&text, &ctx.dialect).ok()?;
        column_lineage(&parsed, &ctx.dialect)
    }
}

fn offset(text: &str, pos: &serde_json::Value) -> usize {
    let line = pos["line"].as_u64().unwrap() as usize;
    let character = pos["character"].as_u64().unwrap() as usize;
    let mut at = 0;
    for (i, l) in text.split_inclusive('\n').enumerate() {
        if i == line {
            return at + character.min(l.len());
        }
        at += l.len();
    }
    at
}

/// The action's changes applied: each file it touches as it would read
/// after, `(created)` or `(deleted)` beside it.
fn applied(action: &CodeAction) -> String {
    let wire = serde_json::to_value(action.edit.as_ref().unwrap()).unwrap();
    let base = cogitator_lsp::fs_path_to_uri(&root());
    let rel = |u: &str| {
        u.strip_prefix(&base)
            .unwrap_or(u)
            .trim_start_matches('/')
            .to_string()
    };
    let mut files: BTreeMap<String, (String, &str)> = BTreeMap::new();
    for change in wire["documentChanges"].as_array().unwrap() {
        match change["kind"].as_str() {
            Some("create") => {
                files.insert(
                    rel(change["uri"].as_str().unwrap()),
                    (String::new(), "created"),
                );
            }
            Some("delete") => {
                files.insert(
                    rel(change["uri"].as_str().unwrap()),
                    (String::new(), "deleted"),
                );
            }
            Some(other) => panic!("unexpected {other}"),
            None => {
                let u = change["textDocument"]["uri"].as_str().unwrap();
                let key = rel(u);
                let entry = files.entry(key).or_insert_with(|| {
                    let path = cogitator_lsp::uri_to_fs_path(u).unwrap();
                    (read(Path::new(&path)).unwrap(), "edited")
                });
                let mut edits: Vec<&serde_json::Value> =
                    change["edits"].as_array().unwrap().iter().collect();
                edits.sort_by_key(|e| std::cmp::Reverse(offset(&entry.0, &e["range"]["start"])));
                for e in edits {
                    let a = offset(&entry.0, &e["range"]["start"]);
                    let b = offset(&entry.0, &e["range"]["end"]);
                    entry.0.replace_range(a..b, e["newText"].as_str().unwrap());
                }
            }
        }
    }
    let mut out = format!("title: {}\nkind: {:?}\n", action.title, action.kind);
    for (file, (text, how)) in files {
        out.push_str(&format!("\n=== {file} ({how})\n{text}"));
    }
    out
}

fn scaffold(ctx: &DbtContext, rel: &str, needle: &str) -> Option<CodeAction> {
    let (docs, uri, range) = open(ctx, rel, needle);
    let types = |_: &str| None;
    let inputs = features::ScaffoldInputs {
        catalog: None,
        types: &types,
        read: &read,
    };
    features::staging_scaffold_action(ctx, docs.get(&uri).unwrap(), &uri, range, &inputs)
}

#[test]
fn a_source_call_scaffolds_its_staging_model() {
    let ctx = ctx();
    let action = scaffold(
        &ctx,
        "models/staging/bakery/stg_bakery__crusts.sql",
        "bakes",
    )
    .expect("a scaffold");
    insta::assert_snapshot!(applied(&action));
}

#[test]
fn a_source_entry_in_yaml_scaffolds_the_same_model() {
    let ctx = ctx();
    let from_sql = scaffold(
        &ctx,
        "models/staging/bakery/stg_bakery__crusts.sql",
        "bakes",
    );
    let from_yaml = scaffold(
        &ctx,
        "models/staging/bakery/_bakery__sources.yml",
        "Flour Kind",
    );
    assert_eq!(applied(&from_sql.unwrap()), applied(&from_yaml.unwrap()));
}

#[test]
fn a_staging_model_that_exists_is_not_scaffolded_again() {
    let ctx = ctx();
    assert!(scaffold(&ctx, "models/staging/bakery/stg_bakery__ovens.sql", "ovens").is_none());
}

#[test]
fn a_ref_without_known_columns_scaffolds_select_star() {
    let ctx = ctx();
    let action = scaffold(&ctx, "models/marts/crust_report.sql", "crust_hops_more").unwrap();
    insta::assert_snapshot!(applied(&action));
}

#[test]
fn an_extracted_cte_takes_the_pass_through_descriptions_along() {
    let ctx = ctx();
    let (docs, uri, range) = open(&ctx, "models/marts/crust_report.sql", "crusts as");
    let action =
        features::extract_cte_action(&ctx, docs.get(&uri).unwrap(), &uri, range, &read).unwrap();
    insta::assert_snapshot!(applied(&action));
}

fn inline(ctx: &DbtContext, rel: &str, needle: &str) -> Option<CodeAction> {
    let (docs, uri, range) = open(ctx, rel, needle);
    features::inline_model_action(ctx, docs.get(&uri).unwrap(), &uri, range, &read)
}

#[test]
fn a_small_model_is_inlined_into_an_existing_with() {
    let ctx = ctx();
    let action = inline(&ctx, "models/marts/crust_report.sql", "crust_hops_more").unwrap();
    insta::assert_snapshot!(applied(&action));
}

#[test]
fn a_model_is_inlined_under_a_new_with() {
    let ctx = ctx();
    let action = inline(&ctx, "models/marts/inline_plain.sql", "lonely").unwrap();
    insta::assert_snapshot!(applied(&action));
}

#[test]
fn a_model_with_tests_or_other_dependents_is_not_inlined() {
    let ctx = ctx();
    assert!(inline(&ctx, "models/marts/uses_tested.sql", "tested_model").is_none());
    assert!(inline(&ctx, "models/marts/shared_a.sql", "shared_model").is_none());
}

fn yaml_inherits(ctx: &DbtContext, model: &str) -> Vec<String> {
    let details = &ctx.model_detail_map[model];
    let schema = details.schema_uri.clone().unwrap();
    let text = read(&schema).unwrap();
    let lineage = lineage_of(ctx);
    let schema_uri = cogitator_lsp::fs_path_to_uri(&schema);
    features::inherit_description_actions(ctx, &text, &schema_uri, model, details, &lineage, &read)
        .iter()
        .map(applied)
        .collect()
}

#[test]
fn a_description_is_inherited_over_hops_and_keeps_its_doc_call() {
    let ctx = ctx();
    let actions = yaml_inherits(&ctx, "crust_relay");
    assert_eq!(actions.len(), 1, "{actions:#?}"); // loaf_id is documented
    insta::assert_snapshot!(actions[0]);
}

#[test]
fn a_computed_column_inherits_nothing() {
    let ctx = ctx();
    let actions = yaml_inherits(&ctx, "crust_double");
    assert_eq!(actions.len(), 1, "{actions:#?}");
    assert!(actions[0].contains("'crust_id'"), "{}", actions[0]);
}

#[test]
fn a_source_column_description_is_inherited() {
    let ctx = ctx();
    let actions = yaml_inherits(&ctx, "loaf_ids");
    assert_eq!(actions.len(), 1, "{actions:#?}");
    assert!(
        actions[0].contains("from source bakery.bakes.id"),
        "{}",
        actions[0]
    );
    assert!(actions[0].contains("description: \"The loaf's id.\""));
}

#[test]
fn a_select_item_inherits_into_a_new_properties_file() {
    let ctx = ctx();
    let (docs, uri, range) = open(&ctx, "models/marts/crust_hops.sql", "crust_id,");
    let lineage = lineage_of(&ctx);
    let action = features::inherit_description_sql_action(
        &ctx,
        docs.get(&uri).unwrap(),
        &uri,
        range,
        &lineage,
        &read,
    )
    .unwrap();
    insta::assert_snapshot!(applied(&action));
}

#[test]
fn a_select_item_inherits_into_the_existing_entry() {
    let ctx = ctx();
    let (docs, uri, range) = open(&ctx, "models/marts/crust_relay.sql", "crust_id,");
    let lineage = lineage_of(&ctx);
    let action = features::inherit_description_sql_action(
        &ctx,
        docs.get(&uri).unwrap(),
        &uri,
        range,
        &lineage,
        &read,
    )
    .unwrap();
    insta::assert_snapshot!(applied(&action));
    // a documented column is left alone, an expression offers nothing
    let (docs, uri, range) = open(&ctx, "models/marts/crust_relay.sql", "loaf_id from");
    assert!(features::inherit_description_sql_action(
        &ctx,
        docs.get(&uri).unwrap(),
        &uri,
        range,
        &lineage,
        &read
    )
    .is_none());
}

#[test]
fn the_warehouse_catalog_orders_the_columns_and_the_yaml_types_win() {
    use cogitator_analysis::artifacts::catalog::{CatalogColumn, CatalogIndex, CatalogTable};
    let ctx = ctx();
    let column = |name: &str, ty: &str, index: i64| CatalogColumn {
        name: name.into(),
        data_type: ty.into(),
        comment: None,
        index: Some(index),
    };
    let mut catalog = CatalogIndex::default();
    catalog.sources.insert(
        ("bakery".into(), "bakes".into()),
        CatalogTable {
            columns: vec![
                column("Flour Kind", "VARCHAR", 1),
                column("id", "BIGINT", 2),
                column("weightGrams", "DOUBLE", 3),
            ],
            ..Default::default()
        },
    );
    let (docs, uri, range) = open(
        &ctx,
        "models/staging/bakery/stg_bakery__crusts.sql",
        "bakes",
    );
    let types = |_: &str| None;
    let inputs = features::ScaffoldInputs {
        catalog: Some(&catalog),
        types: &types,
        read: &read,
    };
    let action =
        features::staging_scaffold_action(&ctx, docs.get(&uri).unwrap(), &uri, range, &inputs)
            .unwrap();
    let text = applied(&action);
    let sql = text.split("=== ").nth(1).unwrap();
    assert!(
        sql.contains(
            "        cast(\"Flour Kind\" as varchar) as flour_kind,\n        \
             cast(id as integer) as bake_id,\n        \
             cast(\"weightGrams\" as double) as weight_grams\n"
        ),
        "{sql}"
    );
}

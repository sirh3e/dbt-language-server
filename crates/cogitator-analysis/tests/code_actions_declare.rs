//! The declaring and governance code actions over the fixture: a source
//! or source table no YAML declares, a docs block no file holds, a ref
//! into another group's private model, and an enforced contract that
//! drifted from the SQL. The fixture declares no governance, so those
//! cases set it on the context, and the YAML the shell would read.

use std::path::{Path, PathBuf};

use cogitator_analysis::{build_context, features, DbtContext, Documents};
use cogitator_lsp::{CodeAction, Position, Range};
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn uri_of(rel: &str) -> String {
    cogitator_lsp::fs_path_to_uri(&fixture_root().join(rel))
}

fn read(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn at(text: &str, needle: &str) -> Range {
    let character = text.find(needle).expect("needle") as u32 + 1;
    Range::zero_width(Position { line: 0, character })
}

/// The one edit of an action's first document change, as JSON.
fn edit(action: &CodeAction) -> serde_json::Value {
    let wire = serde_json::to_value(action.edit.as_ref().unwrap()).unwrap();
    wire["documentChanges"][0].clone()
}

/// The text line by line, as it stands before line `line`.
fn line_before(path: &Path, line: u64) -> String {
    read(path)
        .unwrap()
        .lines()
        .nth(line as usize - 1)
        .unwrap()
        .to_string()
}

#[test]
fn an_undeclared_table_of_a_known_source_is_flagged_and_added() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let uri = uri_of("models/probe.sql");
    let text = "select * from {{ source('stripe', 'refunds') }}";
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    let doc = docs.get(&uri).unwrap();

    let found = features::dangling_ref_diagnostics(&ctx, doc);
    assert_eq!(
        found.iter().map(|d| d.code.as_str()).collect::<Vec<_>>(),
        vec!["dangling-source-table"]
    );
    assert!(
        found[0].message.contains("'refunds'"),
        "{}",
        found[0].message
    );

    let actions = features::declare_missing_actions(&ctx, doc, &uri, at(text, "refunds"), &read);
    assert_eq!(actions[0].title, "Add table 'refunds' to source 'stripe'");
    let change = edit(&actions[0]);
    assert_eq!(change["textDocument"]["uri"], uri_of("models/schema.yml"));
    assert_eq!(change["edits"][0]["newText"], "      - name: refunds\n");
    let line = change["edits"][0]["range"]["start"]["line"]
        .as_u64()
        .unwrap();
    assert_eq!(
        line_before(&fixture_root().join("models/schema.yml"), line),
        "      - name: payments"
    );
}

#[test]
fn an_undeclared_source_joins_the_projects_sources() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let uri = uri_of("models/probe.sql");
    let text = "select * from {{ source('crm', 'accounts') }}";
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    let actions = features::declare_missing_actions(
        &ctx,
        docs.get(&uri).unwrap(),
        &uri,
        at(text, "crm"),
        &read,
    );
    assert_eq!(
        actions[0].title,
        "Declare source 'crm' with table 'accounts'"
    );
    let change = edit(&actions[0]);
    assert_eq!(
        change["edits"][0]["newText"],
        "  - name: crm\n    tables:\n      - name: accounts\n"
    );
}

#[test]
fn a_missing_docs_block_is_appended_to_the_docs_file_beside_the_model() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let uri = uri_of("models/probe.sql");
    let text = "select 1 as x -- {{ doc('brand_new_doc') }}";
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    let actions = features::declare_missing_actions(
        &ctx,
        docs.get(&uri).unwrap(),
        &uri,
        at(text, "brand_new_doc"),
        &read,
    );
    assert_eq!(actions[0].title, "Create docs block 'brand_new_doc'");
    let change = edit(&actions[0]);
    assert_eq!(change["textDocument"]["uri"], uri_of("models/docs.md"));
    assert_eq!(
        change["edits"][0]["newText"],
        "\n{% docs brand_new_doc %}\n\n{% enddocs %}\n"
    );
}

/// stg_customers private to group `finance`, its YAML saying so.
fn governed() -> (DbtContext, impl Fn(&Path) -> Option<String>) {
    let mut ctx = build_context(fixture_root().to_str().unwrap());
    let stg = ctx.model_detail_map.get_mut("stg_customers").unwrap();
    stg.access = Some("private".into());
    stg.group = Some("finance".into());
    let staging = fixture_root().join("models/staging/schema.yml");
    let read = move |path: &Path| {
        let text = read(path)?;
        Some(if path == staging {
            // line by line: a Windows checkout has CRLF endings
            let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
            let mut out = String::new();
            for line in text.lines() {
                out.push_str(line);
                out.push_str(newline);
                if line == "  - name: stg_customers" {
                    out.push_str("    access: private");
                    out.push_str(newline);
                }
            }
            out
        } else {
            text
        })
    };
    (ctx, read)
}

#[test]
fn a_private_ref_offers_to_widen_the_target_or_join_its_group() {
    let (ctx, read) = governed();
    let uri = uri_of("models/customers.sql");
    let text = "select * from {{ ref('stg_customers') }}";
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    let actions = features::private_ref_actions(
        &ctx,
        docs.get(&uri).unwrap(),
        &uri,
        at(text, "stg_customers"),
        &read,
    );
    let titles: Vec<&str> = actions.iter().map(|a| a.title.as_str()).collect();
    assert_eq!(
        titles,
        vec![
            "Make 'stg_customers' protected",
            "Move 'customers' into group 'finance'"
        ]
    );
    assert!(actions.iter().all(|a| a.is_preferred.is_none()));
    let protect = edit(&actions[0]);
    assert_eq!(protect["edits"][0]["newText"], "protected");
    assert_eq!(protect["edits"][0]["range"]["start"]["line"], 4);
    assert_eq!(protect["edits"][0]["range"]["start"]["character"], 12);
    let join = edit(&actions[1]);
    assert_eq!(join["textDocument"]["uri"], uri_of("models/schema.yml"));
    assert_eq!(join["edits"][0]["newText"], "    group: finance\n");

    // inside the group, or into a public model: nothing to fix
    let (mut same, _) = governed();
    same.model_detail_map.get_mut("customers").unwrap().group = Some("finance".into());
    let doc = docs.get(&uri).unwrap();
    assert!(
        features::private_ref_actions(&same, doc, &uri, at(text, "stg_customers"), &read)
            .is_empty()
    );
}

#[test]
fn a_drifted_contract_offers_to_declare_and_to_drop_columns() {
    let mut ctx = build_context(fixture_root().to_str().unwrap());
    ctx.model_detail_map
        .get_mut("customers")
        .unwrap()
        .contract_enforced = true;
    let details = ctx.model_detail_map.get("customers").unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&details.uri);
    let text = "select customer_id, first_name, 1 as brand_new_col from {{ ref('stg_customers') }}";
    let mut docs = Documents::default();
    docs.open(uri.clone(), text, &ctx.dialect);
    let lineage =
        cogitator_analysis::lineage::column_lineage(&docs.get(&uri).unwrap().parsed, &ctx.dialect)
            .expect("lineage");
    let yaml = read(details.schema_uri.as_deref().unwrap()).unwrap();

    let add = features::add_missing_columns_action(&ctx, "customers", details, &lineage, &yaml)
        .expect("add");
    assert_eq!(
        add.title,
        "Add 1 undeclared column(s) of 'customers' to its contract"
    );
    assert_eq!(
        edit(&add)["edits"][0]["newText"],
        "      - name: brand_new_col\n        description: \"\"\n        data_type: \"\"\n"
    );

    let remove = features::remove_unproduced_columns_action("customers", details, &lineage, &yaml)
        .expect("remove");
    assert_eq!(
        remove.title,
        "Remove 5 contracted column(s) 'customers' never produces"
    );
    assert!(remove.is_preferred.is_none());
    // each deletion starts at its item's `- name:` line and keeps the
    // produced columns' items
    let lines: Vec<&str> = yaml.lines().collect();
    for e in edit(&remove)["edits"].as_array().unwrap() {
        let line = e["range"]["start"]["line"].as_u64().unwrap() as usize;
        assert!(
            lines[line].trim_start().starts_with("- name: "),
            "{}",
            lines[line]
        );
        assert!(!lines[line].contains("customer_id") && !lines[line].contains("first_name"));
    }

    // without a contract there is nothing to remove
    let free = build_context(fixture_root().to_str().unwrap());
    let details = free.model_detail_map.get("customers").unwrap();
    assert!(
        features::remove_unproduced_columns_action("customers", details, &lineage, &yaml).is_none()
    );
}

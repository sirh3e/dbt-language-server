//! Write-assist completion (local aliases/identifiers, relation columns,
//! SQL keywords) and the Snowflake dialect coverage.

use std::path::PathBuf;

use cogitator_analysis::lineage::{column_lineage, SourceRef};
use cogitator_analysis::{build_context, features, util, Documents};
use cogitator_dialects::Dialect;
use cogitator_lsp::{completion_kind, Position};
use cogitator_parser::{DocumentParser, JinjaSqlParser};
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

#[test]
fn write_assist_suggests_ctes_identifiers_columns_and_keywords() {
    let ctx = build_context(fixture_root().to_str().unwrap());
    let text = concat!(
        "with recent_orders as (\n",
        "    select * from {{ ref('customers') }} c\n",
        ")\n",
        "select \n",
        "from recent_orders\n",
    );
    let uri = "file:///tmp/assist.sql";
    let mut docs = Documents::default();
    docs.open(uri.to_string(), text, &ctx.dialect);
    let doc = docs.get(uri).unwrap();

    // cursor in plain SQL after "select " on line 3
    let items = features::completion(
        &ctx,
        doc,
        Position {
            line: 3,
            character: 7,
        },
    );

    let find = |label: &str| items.iter().find(|i| i.label == label);
    // local CTE
    let cte = find("recent_orders").expect("cte suggested");
    assert_eq!(cte.detail, "CTE in this file");
    // documented column of the ref'd model, with its YAML description
    let column = find("customer_id").expect("column of customers suggested");
    assert_eq!(column.kind, completion_kind::FIELD);
    assert_eq!(column.detail, "Column of customers");
    assert!(column.documentation.contains("unique identifier"));
    // buffer identifier (the alias `c`)
    let alias = find("c").expect("alias identifier suggested");
    assert_eq!(alias.detail, "identifier in this file");
    // SQL keyword incl. the Snowflake-specific set
    let qualify = find("qualify").expect("qualify keyword suggested");
    assert_eq!(qualify.kind, completion_kind::KEYWORD);
    // (dialect functions join these for snowflake/bigquery profiles —
    // duckdb ships no docs, faithful to Go; covered in the test below)
}

#[test]
fn snowflake_dialect_from_profiles() {
    let profiles = concat!(
        "analytics:\n",
        "  target: prod\n",
        "  outputs:\n",
        "    prod:\n",
        "      type: snowflake\n",
        "      account: xy12345\n",
    );
    assert_eq!(
        util::dialect_from_profiles(profiles, "analytics", None),
        Dialect::new("snowflake")
    );
}

#[test]
fn snowflake_lineage_parses_qualify() {
    let dialect = Dialect::new("snowflake");
    let text = concat!(
        "select id, amount\n",
        "from {{ ref('payments') }}\n",
        "qualify row_number() over (partition by id order by amount desc) = 1\n",
    );
    let doc = JinjaSqlParser.parse(text, &dialect).unwrap();
    let lineage = column_lineage(&doc, &dialect).expect("snowflake QUALIFY parses");
    assert_eq!(
        lineage.columns[0].sources,
        vec![SourceRef::Model {
            name: "payments".into(),
            column: "id".into()
        }]
    );
}

#[test]
fn snowflake_functions_documented_and_completable() {
    let dialect = Dialect::new("snowflake");
    assert!(dialect.function_docs().contains_key("listagg"));
    assert!(dialect
        .function_completion_items()
        .iter()
        .any(|i| i.label == "qualify" || i.label == "listagg"));
}

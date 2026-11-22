use cogitator_schemas::ColumnEntry;

use super::*;
use crate::context::{SourceDetails, SourceTableDetails};
use crate::lineage::ColumnLineage;

fn column(name: &str) -> ColumnEntry {
    ColumnEntry {
        name: name.to_string(),
        description: String::new(),
        name_pos: Default::default(),
        data_type: None,
        tests: Vec::new(),
    }
}

fn ctx() -> DbtContext {
    let mut ctx = DbtContext::default();
    ctx.model_detail_map.insert(
        "stg_orders".into(),
        ModelDetails {
            columns: vec![column("order_id"), column("status")],
            ..Default::default()
        },
    );
    let mut source = SourceDetails::default();
    source.tables.insert(
        "payments".into(),
        SourceTableDetails {
            columns: vec![column("id"), column("amount")],
            ..Default::default()
        },
    );
    ctx.source_detail_map.insert("raw".into(), source);
    ctx
}

const SQL: &str = "select o.order_id, p.amount\nfrom {{ ref('stg_orders') }} o\n\
join {{ source('raw', 'payments') }} p on p.id = o.order_id\n\
left join {{ ref('stg_orders') }} again on true\n";

fn action_text(yaml: &str, lineage: Option<&ModelLineage>) -> Option<String> {
    let details = ModelDetails {
        columns: vec![column("order_id"), column("amount")],
        ..Default::default()
    };
    let action = unit_test_scaffold_action(
        &ctx(),
        "orders",
        &details,
        SQL,
        yaml,
        "file:///p/models/schema.yml",
        lineage,
    )?;
    let edit = action.edit.unwrap();
    let DocumentChange::Edit(edit) = &edit.document_changes[0] else {
        panic!("an edit");
    };
    Some(apply(yaml, &edit.edits[0]))
}

fn apply(text: &str, edit: &cogitator_lsp::TextEdit) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let start = edit.range.start;
    let line = &mut lines[start.line as usize];
    let at = start.character as usize;
    let joined = format!("{}{}{}", &line[..at], edit.new_text, &line[at..]);
    *line = joined;
    lines.join("\n")
}

#[test]
fn inputs_are_deduplicated_in_order() {
    let parsed = JinjaSqlParser.parse(SQL, &Dialect::default()).unwrap();
    assert_eq!(
        model_inputs(&parsed),
        vec![
            Input::Ref("stg_orders".into()),
            Input::Source("raw".into(), "payments".into())
        ]
    );
}

#[test]
fn starts_a_unit_tests_list_from_documented_columns() {
    let yaml = "version: 2\nmodels:\n  - name: orders\n";
    let text = action_text(yaml, None).unwrap();
    assert_eq!(
        text,
        "version: 2\nmodels:\n  - name: orders\n\nunit_tests:\n  - name: test_orders\n    model: orders\n    given:\n      - input: ref('stg_orders')\n        rows:\n          - {order_id: null, status: null}\n      - input: source('raw', 'payments')\n        rows:\n          - {id: null, amount: null}\n    expect:\n      rows:\n        - {order_id: null, amount: null}\n"
    );
}

#[test]
fn lineage_narrows_inputs_and_names_outputs() {
    let lineage = ModelLineage {
        columns: vec![ColumnLineage {
            name: "order_id".into(),
            sources: vec![SourceRef::Model {
                name: "stg_orders".into(),
                column: "order_id".into(),
            }],
        }],
        origin: None,
    };
    let text = action_text("models:\n  - name: orders\n", Some(&lineage)).unwrap();
    assert!(
        text.contains("input: ref('stg_orders')\n        rows:\n          - {order_id: null}\n"),
        "{text}"
    );
    // no lineage traced to the source: its documented columns stand in
    assert!(text.contains("- {id: null, amount: null}"), "{text}");
    assert!(
        text.contains("expect:\n      rows:\n        - {order_id: null}\n"),
        "{text}"
    );
}

#[test]
fn appends_to_an_existing_list_with_a_fresh_name() {
    let yaml = "models:\n  - name: orders\nunit_tests:\n  - name: test_orders\n    model: orders\n";
    let text = action_text(yaml, None).unwrap();
    assert!(
        text.starts_with("models:\n  - name: orders\nunit_tests:\n  - name: test_orders\n    model: orders\n  - name: test_orders_2\n"),
        "{text}"
    );
    assert_eq!(text.matches("unit_tests:").count(), 1);
}

#[test]
fn an_inline_list_is_left_alone() {
    assert!(action_text("models: []\nunit_tests: []\n", None).is_none());
}

#[test]
fn rows_quote_keys_that_need_it() {
    assert_eq!(row(&["a".into(), "b c".into()]), "{a: null, \"b c\": null}");
    assert_eq!(unit_test_lines("t", "m", &[], &[])[2], "  given: []");
}

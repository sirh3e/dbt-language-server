//! Renaming names local to one file, end to end on the jaffle fixture:
//! a `set` var, a loop var, a macro parameter (with the keyword
//! arguments of its calls in other files), a CTE and a table alias —
//! and the var rename, which must still win on a `var('x')` call.

mod harness;
use harness::jaffle;
use serde_json::{json, Value};

fn position_of(text: &str, needle: &str, into: usize) -> Value {
    let (line, l) = text
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(needle))
        .unwrap_or_else(|| panic!("{needle} not in the text"));
    json!({"line": line, "character": l.find(needle).unwrap() + into})
}

fn open(lsp: &mut harness::client::Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
}

/// prepareRename's placeholder and the rename's edits, by file name.
fn rename(
    lsp: &mut harness::client::Lsp,
    uri: &str,
    at: Value,
    new: &str,
) -> (Value, Vec<(String, usize)>) {
    let params = json!({"textDocument": {"uri": uri}, "position": at});
    let prepared = lsp.request("textDocument/prepareRename", params.clone());
    let mut params = params;
    params["newName"] = json!(new);
    let renamed = lsp.request("textDocument/rename", params);
    let changes = renamed["result"]["documentChanges"]
        .as_array()
        .unwrap_or_else(|| panic!("no edit: {renamed}"))
        .iter()
        .map(|c| {
            let edits = c["edits"].as_array().unwrap();
            assert!(edits.iter().all(|e| e["newText"] == new), "{c}");
            let file = c["textDocument"]["uri"].as_str().unwrap();
            (file.rsplit('/').next().unwrap().to_string(), edits.len())
        })
        .collect();
    (prepared["result"]["placeholder"].clone(), changes)
}

fn file(project: &harness::fixture::TestProject, rel: &str) -> (String, String) {
    let path = project.root.join(rel);
    let text = std::fs::read_to_string(&path).unwrap();
    (cogitator_lsp::fs_path_to_uri(&path), text)
}

#[test]
fn a_set_var_and_a_loop_var_rename_in_their_scope() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    let (uri, text) = file(&project, "models/orders.sql");
    open(&mut lsp, &uri, &text);

    let at = position_of(&text, "set payment_methods", 4);
    let (placeholder, changes) = rename(&mut lsp, &uri, at, "methods");
    assert_eq!(placeholder, "payment_methods");
    // the `set` and the two loops that read it
    assert_eq!(changes, vec![("orders.sql".to_string(), 3)]);

    // the first loop's var: the target and its two uses — not the second
    // loop's var of the same name, nor the SQL column `payment_method`
    let at = position_of(&text, "for payment_method in", 4);
    let (placeholder, changes) = rename(&mut lsp, &uri, at, "method");
    assert_eq!(placeholder, "payment_method");
    assert_eq!(changes, vec![("orders.sql".to_string(), 3)]);

    let highlights = lsp.request(
        "textDocument/documentHighlight",
        json!({"textDocument": {"uri": uri}, "position": position_of(&text, "for payment_method in", 4)}),
    );
    let kinds: Vec<i64> = highlights["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h["kind"].as_i64().unwrap())
        .collect();
    assert_eq!(kinds, vec![3, 2, 2], "{highlights}");

    // `order_payments.{{ payment_method }}_amount` is a column the binder
    // cannot see: renaming the CTE is refused, not done in part
    let params = json!({"textDocument": {"uri": uri},
        "position": position_of(&text, "order_payments as", 0), "newName": "op"});
    let refused = lsp.request("textDocument/rename", params);
    assert!(
        refused["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("cannot follow")),
        "{refused}"
    );
}

#[test]
fn a_macro_parameter_renames_its_keyword_arguments_project_wide() {
    let project = jaffle();
    std::fs::write(
        project.root.join("models/kw_probe.sql"),
        "select {{ full_name('a', last_name='b') }} as n, {{ full_name('a', 'b') }} as m\n",
    )
    .unwrap();
    let (mut lsp, _) = project.booted();
    let (uri, text) = file(&project, "macros/jaffle_macros.sql");
    open(&mut lsp, &uri, &text);

    let at = position_of(&text, "last_name)", 0);
    let (placeholder, mut changes) = rename(&mut lsp, &uri, at.clone(), "surname");
    assert_eq!(placeholder, "last_name");
    changes.sort();
    // the header and the use; the keyword call; customers.sql passes it
    // positionally and is not touched
    assert_eq!(
        changes,
        vec![
            ("jaffle_macros.sql".to_string(), 2),
            ("kw_probe.sql".to_string(), 1)
        ]
    );

    let refs = lsp.request(
        "textDocument/references",
        json!({"textDocument": {"uri": uri}, "position": at,
               "context": {"includeDeclaration": true}}),
    );
    assert_eq!(refs["result"].as_array().unwrap().len(), 3, "{refs}");
}

#[test]
fn a_cte_and_an_alias_rename_through_the_binder() {
    let project = jaffle();
    std::fs::write(
        project.root.join("models/alias_probe.sql"),
        "select o.order_id, o.status from {{ ref('stg_orders') }} o where o.order_id > 0\n",
    )
    .unwrap();
    let (mut lsp, uri) = project.booted();
    let (_, text) = file(&project, "models/customers.sql");

    let at = position_of(&text, "customer_payments as", 0);
    let (placeholder, changes) = rename(&mut lsp, &uri, at, "payments_by_customer");
    assert_eq!(placeholder, "customer_payments");
    // definition, `left join`, and two qualified columns
    assert_eq!(changes, vec![("customers.sql".to_string(), 4)]);

    // a CTE may not take a model's name
    let params = json!({"textDocument": {"uri": uri},
        "position": position_of(&text, "customer_payments as", 0), "newName": "stg_orders"});
    let refused = lsp.request("textDocument/rename", params);
    assert!(
        refused["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("model")),
        "{refused}"
    );

    let (alias_uri, alias_text) = file(&project, "models/alias_probe.sql");
    open(&mut lsp, &alias_uri, &alias_text);
    let at = position_of(&alias_text, "}} o", 3);
    let (placeholder, changes) = rename(&mut lsp, &alias_uri, at, "ord");
    assert_eq!(placeholder, "o");
    assert_eq!(changes, vec![("alias_probe.sql".to_string(), 4)]);
}

#[test]
fn the_var_rename_still_wins_on_a_var_call() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let (_, text) = file(&project, "models/customers.sql");
    let at = position_of(&text, "jaffle_string')", 2);
    let (placeholder, changes) = rename(&mut lsp, &uri, at, "jaffle_text");
    assert_eq!(placeholder, "jaffle_string");
    let mut files: Vec<String> = changes.into_iter().map(|(f, _)| f).collect();
    files.sort();
    assert_eq!(files, vec!["customers.sql", "dbt_project.yml"]);
}

//! A CTE without the warehouse, end to end on copies of the jaffle
//! fixture: dbt.cteSql (the query that selects it, no limit), its "SQL"
//! lens and code action, and the hover card of a CTE's name.

mod harness;
use harness::{jaffle, Lsp};
use serde_json::{json, Value};

fn open(lsp: &mut Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
}

#[test]
fn the_sql_of_a_cte_is_its_query_without_a_limit() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    let uri = cogitator_lsp::fs_path_to_uri(&project.root.join("models/staging/stg_orders.sql"));

    let reply = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.cteSql", "arguments": [{"uri": uri, "cte": "renamed"}]}),
    );
    let result = &reply["result"];
    assert_eq!(result["model"], "stg_orders", "{reply}");
    assert_eq!(result["cte"], "renamed", "{reply}");
    assert_eq!(result["source"], "rendered", "{reply}");
    let sql = result["sql"].as_str().expect("sql").to_lowercase();
    assert!(
        sql.contains("raw_orders") && sql.contains("renamed"),
        "{sql}"
    );
    assert!(!sql.contains("limit") && !sql.contains("{{"), "{sql}");
    assert!(
        result["text"]
            .as_str()
            .unwrap()
            .starts_with("-- the CTE renamed of stg_orders"),
        "{reply}"
    );

    // a name the model has no CTE of says which it has
    let reply = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.cteSql", "arguments": [{"uri": uri, "cte": "renamd"}]}),
    );
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .contains("renamed"),
        "{reply}"
    );
}

#[test]
fn the_lens_and_the_code_action_open_the_sql() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    let path = project.root.join("models/staging/stg_orders.sql");
    let text = std::fs::read_to_string(&path).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    open(&mut lsp, &uri, &text);

    let lenses = lsp.wait_until("the SQL lenses", |lsp| {
        let reply = lsp.request(
            "textDocument/codeLens",
            json!({"textDocument": {"uri": uri}}),
        );
        let sql: Vec<Value> = reply["result"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|l| l["command"]["command"] == "dbt.cteSql")
            .collect();
        (!sql.is_empty()).then_some(sql)
    });
    assert!(
        lenses.iter().all(|l| l["command"]["title"] == "SQL"),
        "{lenses:?}"
    );
    assert_eq!(lenses.len(), 3, "two CTEs and the final select: {lenses:?}");

    // the cursor in `renamed` (line 12): the action names it
    let reply = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": uri},
               "range": {"start": {"line": 13, "character": 4}, "end": {"line": 13, "character": 4}},
               "context": {"diagnostics": []}}),
    );
    let actions = reply["result"].as_array().cloned().unwrap_or_default();
    let action = actions
        .iter()
        .find(|a| a["command"]["command"] == "dbt.cteSql")
        .unwrap_or_else(|| panic!("a CTE SQL action in {actions:?}"));
    assert_eq!(action["title"], "Show the SQL of CTE `renamed`", "{action}");
    let _ = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.cteSql", "arguments": [action["command"]["arguments"][0]]}),
    );
    let shown = lsp.wait_server_request("window/showDocument", |m| {
        m["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("preview/stg_orders.renamed.sql"))
    });
    let path = cogitator_lsp::uri_to_fs_path(shown["params"]["uri"].as_str().unwrap()).unwrap();
    assert!(std::fs::read_to_string(path)
        .unwrap()
        .contains("raw_orders"));
}

#[test]
fn hovering_a_cte_shows_its_columns_and_where_they_come_from() {
    let project = jaffle();
    let path = project.root.join("models/cte_probe.sql");
    let text =
        "with orders as (\n    select order_id, customer_id from {{ ref('stg_orders') }}\n),\n\
                big as (\n    select order_id from orders\n)\n\
                select * from big\n";
    std::fs::write(&path, text).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let (mut lsp, _) = project.booted();
    open(&mut lsp, &uri, text);
    // `orders` where `big` reads it (line 4, after "from ")
    let value = lsp.wait_until("the CTE hover", |lsp| {
        let reply = lsp.request(
            "textDocument/hover",
            json!({"textDocument": {"uri": uri}, "position": {"line": 4, "character": 27}}),
        );
        reply["result"]["contents"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    });
    assert!(value.starts_with("**CTE** `orders` · 2 columns"), "{value}");
    assert!(
        value.contains("| `order_id` | `stg_orders.order_id` |"),
        "{value}"
    );
}

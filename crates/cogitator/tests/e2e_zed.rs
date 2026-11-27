//! What a client with no command surface gets (Zed): every check here
//! speaks stock LSP only. A code action's command is run verbatim, as
//! the client would; nothing else sends a `dbt.*` command.
//!
//! - "Show column lineage of m.col" opens the column's lineage graph as
//!   a text file (window/showDocument), from the project-wide cache.
//! - textDocument/rename takes a column up- and downstream when the
//!   initializationOptions say `renameColumnDirection: "both"`.

mod harness;
use harness::{boot_with_options, jaffle, Lsp};
use serde_json::{json, Value};

fn position_of(text: &str, needle: &str, offset: u32) -> Value {
    let at = text.find(needle).expect("needle");
    let line = text[..at].matches('\n').count();
    let col = at - text[..at].rfind('\n').map_or(0, |i| i + 1);
    json!({"line": line, "character": col as u32 + offset})
}

fn new_texts(edit: &Value, file: &str) -> Vec<String> {
    edit["documentChanges"]
        .as_array()
        .unwrap_or_else(|| panic!("{edit}"))
        .iter()
        .filter(|c| {
            c["textDocument"]["uri"]
                .as_str()
                .is_some_and(|u| u.ends_with(file))
        })
        .flat_map(|c| c["edits"].as_array().cloned().unwrap_or_default())
        .map(|e| e["newText"].as_str().unwrap().to_string())
        .collect()
}

fn booted(options: Value) -> (harness::TestProject, Lsp, String, String) {
    let project = jaffle();
    let mut lsp = project.server().spawn();
    let uri = boot_with_options(&mut lsp, &project.root, json!({}), options);
    let text = std::fs::read_to_string(project.root.join("models/customers.sql")).unwrap();
    (project, lsp, uri, text)
}

#[test]
fn a_code_action_opens_a_columns_lineage_graph() {
    let (project, mut lsp, uri, text) = booted(json!({}));
    let at = position_of(&text, "customers.customer_id,", 10);
    let actions = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": uri}, "range": {"start": at, "end": at},
               "context": {"diagnostics": []}}),
    );
    let action = actions["result"]
        .as_array()
        .unwrap_or_else(|| panic!("{actions}"))
        .iter()
        .find(|a| a["title"] == "Show column lineage of customers.customer_id")
        .unwrap_or_else(|| panic!("no lineage action: {actions}"))
        .clone();
    // off a column there is none
    let off = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": uri},
               "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
               "context": {"diagnostics": []}}),
    );
    assert!(
        !off["result"].as_array().unwrap().iter().any(|a| a["title"]
            .as_str()
            .is_some_and(|t| t.starts_with("Show column lineage"))),
        "{off}"
    );

    // the client runs the action's command as it is
    let command = &action["command"];
    let answer = lsp.request(
        "workspace/executeCommand",
        json!({"command": command["command"], "arguments": command["arguments"]}),
    );
    assert_eq!(
        answer["result"]["start"], "model:customers::customer_id",
        "{answer}"
    );
    let shown = lsp.wait_server_request("window/showDocument", |m| {
        m["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with(".lineage.txt"))
    });
    let path = cogitator_lsp::uri_to_fs_path(shown["params"]["uri"].as_str().unwrap()).unwrap();
    assert!(
        std::path::Path::new(&path).starts_with(project.root.join("target/.cogitator/lineage")),
        "{path}"
    );
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(
        body.starts_with("Column lineage of customers.customer_id\nup- and downstream, 3 hops"),
        "{body}"
    );
    assert!(body.contains("▶ model  customers.customer_id"), "{body}");
    assert!(body.contains("stg_customers.customer_id"), "{body}");
    assert!(body.contains("models/staging/stg_customers.sql"), "{body}");
    assert!(
        body.contains("stg_customers.customer_id → customers.customer_id"),
        "{body}"
    );
}

#[test]
fn rename_goes_up_and_downstream_when_the_settings_say_both() {
    let rename = |lsp: &mut Lsp, uri: &str, at: &Value| {
        lsp.request(
            "textDocument/rename",
            json!({"textDocument": {"uri": uri}, "position": at, "newName": "cust_id"}),
        )
    };
    // the default: down from customers, stg_customers left alone
    let (_project, mut lsp, uri, text) = booted(json!({}));
    let at = position_of(&text, "customers.customer_id,", 10);
    let down = rename(&mut lsp, &uri, &at);
    assert!(
        new_texts(&down["result"], "stg_customers.sql").is_empty(),
        "{down}"
    );

    // renameColumnDirection "both": from where the column is defined
    let (_project, mut lsp, uri, text) = booted(json!({"renameColumnDirection": "both"}));
    let at = position_of(&text, "customers.customer_id,", 10);
    let prep = lsp.request(
        "textDocument/prepareRename",
        json!({"textDocument": {"uri": uri}, "position": at}),
    );
    assert_eq!(prep["result"]["placeholder"], "customer_id", "{prep}");
    let both = rename(&mut lsp, &uri, &at);
    assert_eq!(
        new_texts(&both["result"], "stg_customers.sql"),
        ["cust_id"],
        "{both}"
    );
    assert_eq!(
        new_texts(&both["result"], "/customers.sql"),
        ["cust_id"; 3],
        "{both}"
    );
    assert_eq!(
        new_texts(&both["result"], "staging/schema.yml"),
        ["cust_id"],
        "{both}"
    );

    // a word it does not know renames down, as without the option
    let (_project, mut lsp, uri, text) = booted(json!({"renameColumnDirection": "sideways"}));
    let at = position_of(&text, "customers.customer_id,", 10);
    let sideways = rename(&mut lsp, &uri, &at);
    assert!(
        new_texts(&sideways["result"], "stg_customers.sql").is_empty(),
        "{sideways}"
    );
}

//! The server's dialogs (for Zed): the "dbt…" code action opens the dbt
//! menu as `window/showMessageRequest` prompts, the answers walk it, and
//! the chosen command's answer opens as a document — end to end on copies
//! of the jaffle fixture, the harness pressing the buttons.

mod harness;
use harness::{jaffle, Lsp};
use serde_json::{json, Value};

/// A server whose client asked for the dialogs, as the Zed extension does.
fn booted_with_dialogs(project: &harness::fixture::TestProject, dialogs: bool) -> (Lsp, String) {
    let mut lsp = project.server().spawn();
    let model = project.root.join("models/customers.sql");
    let text = std::fs::read_to_string(&model).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&model);
    let _ = lsp.request(
        "initialize",
        json!({
            "rootUri": cogitator_lsp::fs_path_to_uri(&project.root),
            "capabilities": {"window": {
                "showMessage": {"messageActionItem": {"additionalPropertiesSupport": true}},
                "showDocument": {"support": true}
            }},
            "initializationOptions": {
                "profilesDir": project.root.join(".dbt").to_string_lossy(),
                "dialogs": dialogs
            },
        }),
    );
    lsp.notify("initialized", json!({}));
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
    lsp.wait_refresh(1);
    (lsp, uri)
}

fn menu_action(lsp: &mut Lsp, uri: &str) -> Option<Value> {
    let reply = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": uri},
               "range": {"start": {"line": 2, "character": 4}, "end": {"line": 2, "character": 4}},
               "context": {"diagnostics": []}}),
    );
    reply["result"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .find(|a| a["command"]["command"] == "dbt.menu")
}

/// Open the menu as the code action does, the buttons already queued.
fn open_menu(lsp: &mut Lsp, uri: &str, buttons: &[&str]) {
    lsp.answer_prompts(buttons);
    let action = menu_action(lsp, uri).expect("the dbt… action");
    let reply = lsp.request("workspace/executeCommand", action["command"].clone());
    assert_eq!(reply["result"], Value::Null, "{reply}");
}

fn titles(prompt: &Value) -> Vec<String> {
    prompt["params"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["title"].as_str().unwrap().to_string())
        .collect()
}

fn opened(lsp: &mut Lsp, tail: &str) -> std::path::PathBuf {
    let shown = lsp.wait_server_request("window/showDocument", |frame| {
        frame["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with(tail))
    });
    let uri = shown["params"]["uri"].as_str().unwrap();
    std::path::PathBuf::from(cogitator_lsp::uri_to_fs_path(uri).unwrap())
}

#[test]
fn the_menu_is_offered_only_to_a_client_that_asks_for_it() {
    let project = jaffle();
    let (mut lsp, uri) = booted_with_dialogs(&project, false);
    assert!(menu_action(&mut lsp, &uri).is_none());
    drop(lsp);
    let (mut lsp, uri) = booted_with_dialogs(&project, true);
    let action = menu_action(&mut lsp, &uri).expect("offered");
    assert_eq!(action["title"], "dbt… (every dbt command)");
    assert_eq!(action["command"]["arguments"][0]["uri"], uri.as_str());
    // the plain boot (Neovim, Emacs, VS Code) never sees it
    drop(lsp);
    let (mut lsp, uri) = project.booted();
    assert!(menu_action(&mut lsp, &uri).is_none());
}

#[test]
fn a_ctes_sql_is_one_walk_through_the_menu() {
    let project = jaffle();
    let (mut lsp, _) = booted_with_dialogs(&project, true);
    let path = project.root.join("models/staging/stg_orders.sql");
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1,
               "text": std::fs::read_to_string(&path).unwrap()}}),
    );
    open_menu(
        &mut lsp,
        &uri,
        &["Model ›", "A CTE's SQL, no warehouse ›", "renamed"],
    );
    let root = lsp.wait_server_request("window/showMessageRequest", |_| true);
    assert_eq!(root["params"]["message"], "dbt — stg_orders");
    assert_eq!(
        titles(&root),
        vec!["Model ›", "Project ›", "Session ›", "Cancel"]
    );
    let model = lsp.wait_server_request("window/showMessageRequest", |_| true);
    assert!(titles(&model).contains(&"More…".to_string()), "{model}");
    let ctes = lsp.wait_server_request("window/showMessageRequest", |_| true);
    assert_eq!(
        titles(&ctes),
        vec!["source", "renamed", "the final select", "Cancel"]
    );
    let shown = opened(&mut lsp, "stg_orders.renamed.sql");
    let sql = std::fs::read_to_string(shown).unwrap();
    assert!(sql.starts_with("-- the CTE renamed of stg_orders"), "{sql}");
    assert!(sql.trim_end().ends_with("renamed;"), "{sql}");
}

#[test]
fn a_commands_error_is_said() {
    let project = jaffle();
    // customers.sql calls a package macro the offline renderer lacks
    let (mut lsp, uri) = booted_with_dialogs(&project, true);
    open_menu(
        &mut lsp,
        &uri,
        &["Model ›", "A CTE's SQL, no warehouse ›", "customer_orders"],
    );
    let said = lsp.wait_notification("window/showMessage", |frame| {
        frame["params"]["message"]
            .as_str()
            .is_some_and(|m| m.starts_with("customer_orders: "))
    });
    assert_eq!(said["params"]["type"], 1, "an error: {said}");
}

#[test]
fn an_answer_opens_as_a_document_after_two_pages() {
    let project = jaffle();
    let (mut lsp, uri) = booted_with_dialogs(&project, true);
    open_menu(&mut lsp, &uri, &["Model ›", "More…", "More…", "Its card"]);
    let path = opened(&mut lsp, "dialogs/modelInfo.customers.md");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with("# Its card\n"), "{text}");
    assert!(text.contains("- **model**: customers"), "{text}");
    assert!(
        path.starts_with(project.root.join("target/.cogitator/dialogs")),
        "{}",
        path.display()
    );
}

#[test]
fn a_question_can_come_from_a_commands_answer() {
    let project = jaffle();
    let (mut lsp, uri) = booted_with_dialogs(&project, true);
    open_menu(&mut lsp, &uri, &["Session ›", "Switch profile / target ›"]);
    let targets = lsp.wait_server_request("window/showMessageRequest", |frame| {
        frame["params"]["message"] == "Switch to which profile and target?"
    });
    let titles = titles(&targets);
    assert!(titles.iter().any(|t| t.contains(" › ")), "{titles:?}");
    assert_eq!(titles.last().map(String::as_str), Some("Cancel"));
}

#[test]
fn a_closed_prompt_ends_the_dialog_and_nothing_leaks_to_the_client() {
    let project = jaffle();
    let (mut lsp, uri) = booted_with_dialogs(&project, true);
    // the root prompt is closed (no button queued): nothing more happens,
    // and a command the dialogs run never answers the client
    open_menu(&mut lsp, &uri, &[]);
    let _root = lsp.wait_server_request("window/showMessageRequest", |_| true);
    open_menu(&mut lsp, &uri, &["Session ›", "What the server knows"]);
    let _ = opened(&mut lsp, "dialogs/status.customers.md");
    // the server still answers requests in order afterwards
    let reply = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.relation", "arguments": [{"uri": uri}]}),
    );
    assert!(reply.get("result").is_some(), "{reply}");
    assert!(
        reply["id"].as_i64().unwrap() > 0,
        "only the client's own ids reach it: {reply}"
    );
}

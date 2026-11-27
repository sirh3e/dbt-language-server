//! dbt.dagText: the project DAG and a model's neighbourhood drawn as text,
//! over the wire and through the dbt menu (Zed's dialogs), end to end on
//! copies of the jaffle fixture.

mod harness;
use harness::{jaffle, Lsp};
use serde_json::{json, Value};

fn dag_text(lsp: &mut Lsp, arg: Value) -> Value {
    lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.dagText", "arguments": [arg]}),
    )
}

#[test]
fn the_project_is_drawn_left_to_right() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    let reply = dag_text(&mut lsp, json!({}));
    let result = &reply["result"];
    let text = result["text"].as_str().expect("text");
    assert!(text.starts_with("dbt DAG — the project: "), "{text}");
    assert_eq!(result["truncated"], false, "{reply}");
    assert!(result["nodes"].as_u64().unwrap() > 3, "{reply}");
    // every model is a box, and edges end in arrows
    for model in ["stg_orders", "orders", "customers"] {
        assert!(text.contains(&format!("│ {model} ")), "{model} in {text}");
    }
    assert!(text.contains("──▸│"), "{text}");
}

#[test]
fn a_depth_draws_the_models_neighbourhood_and_show_opens_it() {
    let project = jaffle();
    let (mut lsp, _) =
        project.booted_with(&[], json!({"window": {"showDocument": {"support": true}}}));
    let path = project.root.join("models/staging/stg_orders.sql");
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let reply = dag_text(&mut lsp, json!({"uri": uri, "depth": 1, "show": true}));
    let text = reply["result"]["text"].as_str().expect("text").to_string();
    assert!(
        text.starts_with("dbt DAG — stg_orders, 1 hop up and down: "),
        "{text}"
    );
    assert!(text.contains("│ orders "), "{text}");
    let whole = dag_text(&mut lsp, json!({}));
    assert!(
        reply["result"]["nodes"].as_u64() < whole["result"]["nodes"].as_u64(),
        "a neighbourhood is less than the project: {text}"
    );
    let shown = lsp.wait_server_request("window/showDocument", |frame| {
        frame["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("dag/stg_orders.dag.txt"))
    });
    let file = cogitator_lsp::uri_to_fs_path(shown["params"]["uri"].as_str().unwrap()).unwrap();
    assert_eq!(std::fs::read_to_string(file).unwrap(), text);
}

#[test]
fn an_unknown_model_is_an_error() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    let reply = dag_text(&mut lsp, json!({"model": "no_such_model"}));
    assert!(
        reply["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("no_such_model")),
        "{reply}"
    );
}

#[test]
fn the_menu_opens_the_project_dag() {
    let project = jaffle();
    let mut lsp = project.server().spawn();
    let model = project.root.join("models/customers.sql");
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
                "dialogs": true
            },
        }),
    );
    lsp.notify("initialized", json!({}));
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1,
               "text": std::fs::read_to_string(&model).unwrap()}}),
    );
    lsp.wait_refresh(1);
    lsp.answer_prompts(&["Project ›", "More…", "The project DAG (text)"]);
    lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.menu", "arguments": [{"uri": uri,
               "position": {"line": 0, "character": 0}}]}),
    );
    let shown = lsp.wait_server_request("window/showDocument", |frame| {
        frame["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("dag/project.dag.txt"))
    });
    let file = cogitator_lsp::uri_to_fs_path(shown["params"]["uri"].as_str().unwrap()).unwrap();
    let text = std::fs::read_to_string(file).unwrap();
    assert!(text.starts_with("dbt DAG — the project: "), "{text}");
}

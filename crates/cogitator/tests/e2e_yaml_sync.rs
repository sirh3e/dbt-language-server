//! `dbt.syncYaml` over the wire: the plan as a preview, the same plan
//! applied through workspace/applyEdit, narrowing by uri, and the
//! single-model "Sync schema YAML" code action.

mod harness;
use harness::jaffle;
use serde_json::json;

fn model<'a>(plan: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    plan["result"]["models"]
        .as_array()
        .and_then(|models| models.iter().find(|m| m["model"] == name))
        .unwrap_or_else(|| panic!("no {name} in {plan}"))
}

fn strings(value: &serde_json::Value) -> Vec<&str> {
    value
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default()
}

#[test]
fn sync_yaml_previews_then_applies() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();

    let preview = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.syncYaml", "arguments": [{}]}),
    );
    assert_eq!(preview["result"]["applied"], false, "{preview}");
    assert!(
        preview["result"]["changes"].as_u64().unwrap_or(0) > 0,
        "{preview}"
    );
    let stg = model(&preview, "stg_customers");
    assert_eq!(
        strings(&stg["added"]),
        ["first_name", "last_name"],
        "{preview}"
    );
    // a versioned model and its version file are left alone, with reasons
    let versioned = model(&preview, "customer_orders");
    assert!(versioned["skipped"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("versioned"));
    let file = preview["result"]["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["uri"].as_str().unwrap().ends_with("staging/schema.yml"))
        .expect("staging schema in the plan");
    assert!(file["diff"]
        .as_str()
        .unwrap()
        .contains("+      - name: first_name"));

    // narrowed to the model of a uri
    let stg_uri =
        cogitator_lsp::fs_path_to_uri(&project.root.join("models/staging/stg_orders.sql"));
    let one = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.syncYaml", "arguments": [{"uri": stg_uri}]}),
    );
    let names: Vec<&str> = one["result"]["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|m| m["model"].as_str())
        .collect();
    assert_eq!(names, ["stg_orders"], "{one}");

    // applied: the same edit reaches the client
    let applied = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.syncYaml", "arguments": [{"apply": true, "models": ["stg_customers"]}]}),
    );
    assert_eq!(applied["result"]["applied"], true, "{applied}");
    let edit = lsp.wait_server_request("workspace/applyEdit", |m| {
        m["params"]["label"] == "Sync schema YAML"
    });
    let change = &edit["params"]["edit"]["documentChanges"][0];
    assert!(change["textDocument"]["uri"]
        .as_str()
        .unwrap()
        .ends_with("staging/schema.yml"));
    let inserted = change["edits"][0]["newText"].as_str().unwrap();
    assert!(
        inserted.contains("- name: first_name\n") && inserted.contains("- name: last_name\n"),
        "{edit}"
    );
}

#[test]
fn a_model_offers_its_own_sync_as_a_code_action() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    let path = project.root.join("models/staging/stg_customers.sql");
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1,
            "text": std::fs::read_to_string(&path).unwrap()}}),
    );
    let actions = lsp.request(
        "textDocument/codeAction",
        json!({
            "textDocument": {"uri": uri},
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 0}},
            "context": {"diagnostics": []}
        }),
    );
    let sync = actions["result"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| {
            a["title"]
                .as_str()
                .is_some_and(|t| t.starts_with("Sync schema YAML of 'stg_customers'"))
        })
        .unwrap_or_else(|| panic!("no sync action in {actions}"));
    assert_eq!(
        sync["title"],
        "Sync schema YAML of 'stg_customers' (2 changes)"
    );
    assert!(sync["edit"]["documentChanges"][0]["textDocument"]["uri"]
        .as_str()
        .unwrap()
        .ends_with("staging/schema.yml"));
}

//! The selector engine end to end: `dbt.select`, selectors.yml hover,
//! lenses, completion and diagnostics, and `workspace/symbol`'s `tag:`.

mod harness;
use harness::jaffle;
use serde_json::json;

fn names(response: &serde_json::Value) -> Vec<String> {
    response["result"]["nodes"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|n| n["name"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn dbt_select_follows_dbt_semantics() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    let select = |lsp: &mut harness::Lsp, selector: &str| {
        lsp.request(
            "workspace/executeCommand",
            json!({"command": "dbt.select", "arguments": [{"selector": selector}]}),
        )
    };

    let nightly = select(&mut lsp, "selector:nightly");
    let picked = names(&nightly);
    assert_eq!(picked[0], "customers", "{nightly}");
    assert!(
        picked.contains(&"unique_customers_customer_id".to_string()),
        "{nightly}"
    );
    assert_eq!(nightly["result"]["nodes"][0]["kind"], "model");
    assert!(nightly["result"]["nodes"][0]["uri"]
        .as_str()
        .unwrap()
        .ends_with("customers.sql"));

    let upstream = select(&mut lsp, "+stg_orders,resource_type:seed");
    assert_eq!(names(&upstream), vec!["raw_orders"], "{upstream}");

    let state = select(&mut lsp, "state:modified");
    assert_eq!(state["result"]["count"], 0, "{state}");
    assert_eq!(
        state["result"]["issues"].as_array().unwrap().len(),
        1,
        "{state}"
    );

    let bad = select(&mut lsp, "@+orders");
    assert!(
        bad["error"]["message"]
            .as_str()
            .unwrap()
            .contains("cannot be combined"),
        "{bad}"
    );

    let tagged = lsp.request("workspace/symbol", json!({ "query": "tag:nightly" }));
    assert_eq!(tagged["result"][0]["name"], "customers", "{tagged}");
}

#[test]
fn selectors_yml_has_lenses_hover_completion_and_diagnostics() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    let path = project.root.join("selectors.yml");
    let text = format!(
        "{}  - name: ghost\n    definition: 'tag:no_such_tag selector:nowhere'\n  - name: draft\n    definition:\n      method: \n",
        std::fs::read_to_string(&path).unwrap()
    );
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "yaml", "version": 1, "text": text}}),
    );

    let lenses = lsp.request(
        "textDocument/codeLens",
        json!({"textDocument": {"uri": uri}}),
    );
    let titles: Vec<&str> = lenses["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["command"]["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles.len(), 4, "{lenses}");
    assert!(titles[0].ends_with("nodes match"), "{lenses}");
    assert_eq!(lenses["result"][0]["command"]["command"], "dbt.select");

    let hover = lsp.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": uri}, "position": {"line": 4, "character": 6}}),
    );
    let contents = hover["result"]["contents"].as_str().unwrap();
    assert!(contents.starts_with("**selector `nightly`**"), "{hover}");
    assert!(contents.contains("model `customers`"), "{hover}");

    let lines = text.lines().count() as u32;
    let completion = lsp.request(
        "textDocument/completion",
        json!({"textDocument": {"uri": uri}, "position": {"line": lines - 1, "character": 14}}),
    );
    let labels: Vec<&str> = completion["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap())
        .collect();
    assert!(
        labels.contains(&"tag") && labels.contains(&"config.materialized"),
        "{completion}"
    );

    let diagnostics = lsp.request(
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": uri}}),
    );
    let messages: Vec<&str> = diagnostics["result"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["message"].as_str().unwrap())
        .collect();
    assert!(
        messages.contains(&"selectors.yml defines no selector `nowhere`"),
        "{diagnostics}"
    );
    assert!(
        !messages
            .iter()
            .any(|m| m.contains("`nightly` matches no node")),
        "{diagnostics}"
    );
}

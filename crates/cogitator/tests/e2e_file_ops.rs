//! End-to-end suite (specs/rust-port/05 §7) — the workspace file
//! operations (will/didRename, will/didDelete, didCreate) and
//! textDocument/declaration.

mod harness;
use harness::jaffle;

fn position_of(text: &str, needle: &str, offset: u32) -> serde_json::Value {
    let at = text.find(needle).expect("needle");
    let line = text[..at].matches('\n').count();
    let col = at - text[..at].rfind('\n').map_or(0, |i| i + 1);
    serde_json::json!({"line": line, "character": col as u32 + offset})
}

fn refreshes(lsp: &mut harness::Lsp) -> u64 {
    lsp.wait_status(|_| true)["refresh"]["count"]
        .as_u64()
        .unwrap_or(0)
}

fn lens_titles(lsp: &mut harness::Lsp, uri: &str) -> Vec<String> {
    lsp.request(
        "textDocument/codeLens",
        serde_json::json!({"textDocument": {"uri": uri}}),
    )["result"]
        .as_array()
        .map(|l| {
            l.iter()
                .filter_map(|x| x["command"]["title"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn file_operations_are_advertised_and_answered() {
    let project = jaffle();
    let (mut lsp, _customers) = project.booted();
    let uri = |rel: &str| cogitator_lsp::fs_path_to_uri(&project.root.join(rel));

    // 1. willRename of a model: the refs and the YAML entry, no file op
    let renamed = lsp.request(
        "workspace/willRenameFiles",
        serde_json::json!({"files": [{
            "oldUri": uri("models/staging/stg_orders.sql"),
            "newUri": uri("models/staging/stg_order_rows.sql")}]}),
    );
    let changes = renamed["result"]["documentChanges"]
        .as_array()
        .unwrap_or_else(|| panic!("{renamed}"));
    assert!(changes.iter().all(|c| c.get("kind").is_none()), "{renamed}");
    let touched: Vec<&str> = changes
        .iter()
        .filter_map(|c| c["textDocument"]["uri"].as_str())
        .collect();
    for file in ["models/customers.sql", "models/staging/schema.yml"] {
        assert!(touched.contains(&uri(file).as_str()), "{file}: {renamed}");
    }
    assert!(changes
        .iter()
        .flat_map(|c| c["edits"].as_array().unwrap())
        .all(|e| e["newText"] == "stg_order_rows"));

    // 2. a move that keeps the name changes nothing
    let moved = lsp.request(
        "workspace/willRenameFiles",
        serde_json::json!({"files": [{
            "oldUri": uri("models/staging/stg_orders.sql"),
            "newUri": uri("models/stg_orders.sql")}]}),
    );
    assert_eq!(moved["result"]["documentChanges"], serde_json::json!([]));

    // 3. willDelete: an empty edit, and one warning naming the dependents
    let deleted = lsp.request(
        "workspace/willDeleteFiles",
        serde_json::json!({"files": [{"uri": uri("models/staging/stg_payments.sql")}]}),
    );
    assert_eq!(deleted["result"]["documentChanges"], serde_json::json!([]));
    let warning = lsp.wait_notification("window/showMessage", |m| {
        m["params"]["message"]
            .as_str()
            .is_some_and(|t| t.contains("stg_payments"))
    });
    let text = warning["params"]["message"].as_str().unwrap();
    assert_eq!(warning["params"]["type"], 2, "{warning}");
    assert!(
        text.contains("customers") && text.contains("orders"),
        "{text}"
    );

    // 4. didCreate: the new model joins the context with its scaffold lens
    let new_model = project.root.join("models/brand_new.sql");
    std::fs::write(&new_model, "select 1 as id\n").unwrap();
    let before = refreshes(&mut lsp);
    lsp.notify(
        "workspace/didCreateFiles",
        serde_json::json!({"files": [{"uri": uri("models/brand_new.sql")}]}),
    );
    lsp.wait_refresh(before + 1);
    let titles = lens_titles(&mut lsp, &uri("models/brand_new.sql"));
    assert!(
        titles.iter().any(|t| t == "Extract schema YAML"),
        "{titles:?}"
    );

    // 5. didDelete: it leaves the context again
    std::fs::remove_file(&new_model).unwrap();
    let before = refreshes(&mut lsp);
    lsp.notify(
        "workspace/didDeleteFiles",
        serde_json::json!({"files": [{"uri": uri("models/brand_new.sql")}]}),
    );
    lsp.wait_refresh(before + 1);
    assert!(lens_titles(&mut lsp, &uri("models/brand_new.sql")).is_empty());

    // 6. didRename: the context follows the new name
    let old = project.root.join("models/staging/stg_customers.sql");
    std::fs::rename(&old, project.root.join("models/staging/stg_clients.sql")).unwrap();
    let before = refreshes(&mut lsp);
    lsp.notify(
        "workspace/didRenameFiles",
        serde_json::json!({"files": [{
            "oldUri": uri("models/staging/stg_customers.sql"),
            "newUri": uri("models/staging/stg_clients.sql")}]}),
    );
    lsp.wait_refresh(before + 1);
    let titles = lens_titles(&mut lsp, &uri("models/staging/stg_clients.sql"));
    assert!(titles.iter().any(|t| t == "dbt run"), "{titles:?}");
    lsp.shutdown();
}

#[test]
fn declaration_is_the_yaml_entry_and_definition_the_sql() {
    let project = jaffle();
    let (mut lsp, customers_uri) = project.booted();
    let customers = std::fs::read_to_string(project.root.join("models/customers.sql")).unwrap();
    let staging_schema =
        cogitator_lsp::fs_path_to_uri(&project.root.join("models/staging/schema.yml"));

    // a ref: declared by its YAML entry, defined by its SQL
    let at_ref = position_of(&customers, "stg_orders'", 1);
    let decl = lsp.request(
        "textDocument/declaration",
        serde_json::json!({"textDocument": {"uri": customers_uri}, "position": at_ref}),
    );
    assert_eq!(decl["result"]["uri"], staging_schema.as_str(), "{decl}");
    let def = lsp.request(
        "textDocument/definition",
        serde_json::json!({"textDocument": {"uri": customers_uri}, "position": at_ref}),
    );
    assert!(
        def["result"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("stg_orders.sql")),
        "{def}"
    );

    // a YAML column entry: definition is where the model's SQL makes it
    let schema_path = project.root.join("models/schema.yml");
    let schema = std::fs::read_to_string(&schema_path).unwrap();
    let schema_uri = cogitator_lsp::fs_path_to_uri(&schema_path);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {
            "uri": schema_uri, "languageId": "yaml", "version": 1, "text": schema}}),
    );
    let at_column = position_of(&schema, "name: first_name", 7);
    let def = lsp.request(
        "textDocument/definition",
        serde_json::json!({"textDocument": {"uri": schema_uri}, "position": at_column}),
    );
    assert_eq!(def["result"]["uri"], customers_uri.as_str(), "{def}");
    let line = def["result"]["range"]["start"]["line"].as_u64().unwrap() as usize;
    assert!(
        customers.lines().nth(line).unwrap().contains("first_name"),
        "{def}"
    );
    let decl = lsp.request(
        "textDocument/declaration",
        serde_json::json!({"textDocument": {"uri": schema_uri}, "position": at_column}),
    );
    assert_eq!(decl["result"]["uri"], schema_uri.as_str(), "{decl}");
    lsp.shutdown();
}

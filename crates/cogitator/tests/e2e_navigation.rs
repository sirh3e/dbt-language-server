//! End-to-end suite (specs/rust-port/05 §7) — project-wide navigation
//! beyond `ref()`: references for sources, macros, docs and columns,
//! macro rename, the macro call hierarchy's outgoing calls, and links.

mod harness;
use harness::jaffle;

fn position_of(text: &str, needle: &str, offset: u32) -> serde_json::Value {
    let at = text.find(needle).expect("needle");
    let line = text[..at].matches('\n').count();
    let col = at - text[..at].rfind('\n').map_or(0, |i| i + 1);
    serde_json::json!({"line": line, "character": col as u32 + offset})
}

fn uris(response: &serde_json::Value) -> Vec<String> {
    response["result"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|l| l["uri"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// references with and without the declaration, for a source table, a
/// package macro called qualified, and a docs block.
#[test]
fn references_for_sources_macros_and_docs() {
    let project = jaffle();
    let (mut lsp, customers_uri) = project.booted();
    let customers = std::fs::read_to_string(project.root.join("models/customers.sql")).unwrap();

    let macro_refs = |lsp: &mut harness::Lsp, include: bool| {
        lsp.request(
            "textDocument/references",
            serde_json::json!({"textDocument": {"uri": customers_uri},
                "position": position_of(&customers, "add_values", 2),
                "context": {"includeDeclaration": include}}),
        )
    };
    let without = uris(&macro_refs(&mut lsp, false));
    assert!(
        without.iter().all(|u| u.ends_with("customers.sql")),
        "{without:?}"
    );
    assert!(!without.is_empty());
    let with = uris(&macro_refs(&mut lsp, true));
    assert!(
        with.iter()
            .any(|u| u.ends_with("jaffle_package_macros.sql")),
        "{with:?}"
    );

    let stg = project.root.join("models/staging/stg_stripe_payments.sql");
    let stg_text = std::fs::read_to_string(&stg).unwrap();
    let stg_uri = cogitator_lsp::fs_path_to_uri(&stg);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": stg_uri, "languageId": "sql", "version": 1,
            "text": stg_text}}),
    );
    let source = lsp.request(
        "textDocument/references",
        serde_json::json!({"textDocument": {"uri": stg_uri},
            "position": position_of(&stg_text, "payments'", 1),
            "context": {"includeDeclaration": true}}),
    );
    let found = uris(&source);
    assert!(found.iter().any(|u| u.ends_with("schema.yml")), "{source}");
    assert!(
        found.iter().any(|u| u.ends_with("stg_stripe_payments.sql")),
        "{source}"
    );

    let docs_md = project.root.join("models/docs.md");
    let docs_text = std::fs::read_to_string(&docs_md).unwrap();
    let docs_uri = cogitator_lsp::fs_path_to_uri(&docs_md);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": docs_uri, "languageId": "markdown",
            "version": 1, "text": docs_text}}),
    );
    let docs = lsp.request(
        "textDocument/references",
        serde_json::json!({"textDocument": {"uri": docs_uri},
            "position": position_of(&docs_text, "orders_status", 1)}),
    );
    assert_eq!(uris(&docs).len(), 1, "{docs}");
    assert!(uris(&docs)[0].ends_with("schema.yml"), "{docs}");

    lsp.shutdown();
}

/// A macro rename edits the header, the calls and the `macros:` YAML
/// entry; a package macro is refused with an error carrying the reason.
#[test]
fn macro_rename_and_outgoing_calls() {
    let project = jaffle();
    std::fs::write(
        project.root.join("macros/schema.yml"),
        "version: 2\n\nmacros:\n  - name: full_name\n    description: first and last\n",
    )
    .unwrap();
    std::fs::write(
        project.root.join("macros/wrapper.sql"),
        "{% macro wrapper() %}{{ full_name('a', 'b') }}{% endmacro %}\n",
    )
    .unwrap();
    let (mut lsp, customers_uri) = project.booted();
    let customers = std::fs::read_to_string(project.root.join("models/customers.sql")).unwrap();
    let at = position_of(&customers, "full_name", 1);

    let prep = lsp.request(
        "textDocument/prepareRename",
        serde_json::json!({"textDocument": {"uri": customers_uri}, "position": at}),
    );
    assert_eq!(prep["result"]["placeholder"], "full_name", "{prep}");

    let edit = lsp.request(
        "textDocument/rename",
        serde_json::json!({"textDocument": {"uri": customers_uri}, "position": at,
            "newName": "person_name"}),
    );
    let edited: Vec<&str> = edit["result"]["documentChanges"]
        .as_array()
        .expect("documentChanges")
        .iter()
        .filter_map(|c| c["textDocument"]["uri"].as_str())
        .collect();
    for file in [
        "jaffle_macros.sql",
        "customers.sql",
        "wrapper.sql",
        "macros/schema.yml",
    ] {
        assert!(edited.iter().any(|u| u.ends_with(file)), "{file}: {edit}");
    }

    let refused = lsp.request(
        "textDocument/rename",
        serde_json::json!({"textDocument": {"uri": customers_uri},
            "position": position_of(&customers, "add_values", 2), "newName": "x"}),
    );
    assert_eq!(refused["error"]["code"], -32803, "{refused}");
    assert!(
        refused["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("jaffle_package")),
        "{refused}"
    );

    // the call hierarchy from the wrapper's header: it calls full_name,
    // and full_name's callers include the wrapper macro
    let wrapper = project.root.join("macros/wrapper.sql");
    let wrapper_text = std::fs::read_to_string(&wrapper).unwrap();
    let wrapper_uri = cogitator_lsp::fs_path_to_uri(&wrapper);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": wrapper_uri, "languageId": "sql",
            "version": 1, "text": wrapper_text}}),
    );
    let items = lsp.request(
        "textDocument/prepareCallHierarchy",
        serde_json::json!({"textDocument": {"uri": wrapper_uri},
            "position": position_of(&wrapper_text, "wrapper(", 1)}),
    );
    let item = items["result"][0].clone();
    assert_eq!(item["name"], "wrapper", "{items}");
    let outgoing = lsp.request(
        "callHierarchy/outgoingCalls",
        serde_json::json!({"item": item}),
    );
    assert_eq!(
        outgoing["result"][0]["to"]["name"], "full_name",
        "{outgoing}"
    );

    let callee = outgoing["result"][0]["to"].clone();
    let incoming = lsp.request(
        "callHierarchy/incomingCalls",
        serde_json::json!({"item": callee}),
    );
    let callers: Vec<&str> = incoming["result"]
        .as_array()
        .expect("incoming")
        .iter()
        .filter_map(|c| c["from"]["name"].as_str())
        .collect();
    assert!(callers.contains(&"customers"), "{incoming}");
    assert!(callers.contains(&"wrapper"), "{incoming}");

    lsp.shutdown();
}

/// documentLink in YAML: `{{ doc() }}` and the plain `to: ref('…')` of a
/// relationship test both link to their definitions.
#[test]
fn yaml_document_links() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let schema = project.root.join("models/schema.yml");
    let text = std::fs::read_to_string(&schema).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&schema);
    lsp.notify(
        "textDocument/didOpen",
        serde_json::json!({"textDocument": {"uri": uri, "languageId": "yaml", "version": 1,
            "text": text}}),
    );
    let links = lsp.request(
        "textDocument/documentLink",
        serde_json::json!({"textDocument": {"uri": uri}}),
    );
    let targets: Vec<&str> = links["result"]
        .as_array()
        .expect("links")
        .iter()
        .filter_map(|l| l["target"].as_str())
        .collect();
    assert!(targets.iter().any(|t| t.ends_with("docs.md")), "{links}");
    assert!(
        targets.iter().any(|t| t.ends_with("models/customers.sql")),
        "{links}"
    );
    lsp.shutdown();
}

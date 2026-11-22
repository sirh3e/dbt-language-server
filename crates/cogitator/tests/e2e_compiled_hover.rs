//! Hover and definition on compiled SQL: a line of
//! `target/compiled/<project>/models/…` maps back to the Jinja source it
//! came from.

mod harness;
use harness::jaffle;
use serde_json::json;

#[test]
fn a_compiled_line_hovers_and_jumps_to_its_source() {
    let project = jaffle();
    let source = std::fs::read_to_string(project.root.join("models/customers.sql")).unwrap();
    // dbt's compile, as far as this model goes: the refs rendered
    let compiled = regex_free_render(&source);
    let path = project
        .root
        .join("target/compiled/jaffle_shop/models/customers.sql");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, &compiled).unwrap();

    let (mut lsp, model_uri) = project.booted();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": compiled}}),
    );
    let line = compiled
        .lines()
        .position(|l| l.contains("\"stg_orders\""))
        .unwrap();
    let source_line = source
        .lines()
        .position(|l| l.contains("ref('stg_orders')"))
        .unwrap();
    let hover = lsp.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": 4}}),
    );
    let contents = hover["result"]["contents"].as_str().unwrap_or_default();
    assert!(contents.contains("**Compiled from**"), "{hover}");
    assert!(contents.contains("models/customers.sql"), "{hover}");
    assert!(contents.contains("ref('stg_orders')"), "{hover}");
    assert!(
        contents.contains(&format!("#L{}", source_line + 1)),
        "{hover}"
    );

    let definition = lsp.request(
        "textDocument/definition",
        json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": 4}}),
    );
    assert_eq!(definition["result"]["uri"], model_uri, "{definition}");
    assert_eq!(
        definition["result"]["range"]["start"]["line"], source_line,
        "{definition}"
    );

    // a copied-through line maps to itself
    let copied = compiled
        .lines()
        .position(|l| l.contains("group by customer_id"))
        .unwrap();
    let expected = source
        .lines()
        .position(|l| l.contains("group by customer_id"))
        .unwrap();
    let definition = lsp.request(
        "textDocument/definition",
        json!({"textDocument": {"uri": uri}, "position": {"line": copied, "character": 0}}),
    );
    assert_eq!(
        definition["result"]["range"]["start"]["line"], expected,
        "{definition}"
    );
}

/// `{{ ref('x') }}` → `"jaffle"."main"."x"`, as dbt compiles it.
fn regex_free_render(source: &str) -> String {
    let mut out = String::new();
    let mut rest = source;
    while let Some(i) = rest.find("{{ ref('") {
        out.push_str(&rest[..i]);
        let after = &rest[i + "{{ ref('".len()..];
        let name_end = after.find('\'').unwrap();
        out.push_str(&format!("\"jaffle\".\"main\".\"{}\"", &after[..name_end]));
        let close = after.find("}}").unwrap();
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    out
}

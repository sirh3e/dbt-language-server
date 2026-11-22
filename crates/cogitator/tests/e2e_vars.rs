//! Project vars end to end on the jaffle fixture: the value after each
//! var() as an inlay hint, definition and references, an override in
//! cogitator.yml (hot) and `inlay.vars: false`, and var() coloured in
//! YAML by dbt.highlight.

mod harness;
use harness::{jaffle, write_config};
use serde_json::{json, Value};

/// customers.sql's `{{ var('jaffle_string') }}`: its line and the
/// character inside the name.
fn var_position(uri: &str) -> (u64, u64) {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap();
    let text = std::fs::read_to_string(path).unwrap();
    let (line, l) = text
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("var('jaffle_string')"))
        .unwrap();
    (line as u64, (l.find("jaffle_string").unwrap() + 2) as u64)
}

fn var_hints(lsp: &mut harness::client::Lsp, uri: &str) -> Vec<Value> {
    let response = lsp.request(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": uri},
               "range": {"start": {"line": 0, "character": 0}, "end": {"line": 200, "character": 0}}}),
    );
    response["result"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .filter(|h| h["label"].as_str().is_some_and(|l| l.starts_with('=')))
        .cloned()
        .collect()
}

/// The 0-based line of `key:` in a file.
fn key_line(path: &std::path::Path, key: &str) -> u64 {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .position(|l| l.trim_start().starts_with(&format!("{key}:")))
        .unwrap() as u64
}

#[test]
fn a_var_shows_its_value_and_goes_to_its_key() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let (line, character) = var_position(&uri);

    let hints = var_hints(&mut lsp, &uri);
    assert_eq!(hints.len(), 1, "{hints:?}");
    assert_eq!(hints[0]["label"], "= 'jaffle'");
    assert_eq!(hints[0]["position"]["line"], line);
    assert!(
        hints[0]["tooltip"]
            .as_str()
            .is_some_and(|t| t.contains("dbt_project.yml")),
        "{hints:?}"
    );

    let definition = lsp.request(
        "textDocument/definition",
        json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}}),
    );
    let target = &definition["result"];
    assert!(
        target["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("/dbt_project.yml")),
        "{definition}"
    );
    assert_eq!(
        target["range"]["start"]["line"],
        key_line(&project.root.join("dbt_project.yml"), "jaffle_string"),
        "{definition}"
    );

    let references = lsp.request(
        "textDocument/references",
        json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character},
               "context": {"includeDeclaration": true}}),
    );
    let files: Vec<String> = references["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            l["uri"]
                .as_str()
                .unwrap()
                .rsplit('/')
                .next()
                .unwrap()
                .to_string()
        })
        .collect();
    assert!(files.contains(&"customers.sql".to_string()), "{references}");
    assert!(
        files.contains(&"dbt_project.yml".to_string()),
        "{references}"
    );
}

#[test]
fn an_override_in_cogitator_yml_wins_and_hints_can_be_turned_off() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let (line, character) = var_position(&uri);

    write_config(&project, &mut lsp, "vars:\n  jaffle_string: overridden\n");
    let label = lsp.wait_until("the override reaches the hint", |lsp| {
        let hints = var_hints(lsp, &uri);
        let label = hints.first()?["label"].as_str()?.to_string();
        label.contains("overridden").then_some(label)
    });
    assert_eq!(label, "= 'overridden' (override)");
    let definition = lsp.request(
        "textDocument/definition",
        json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}}),
    );
    assert!(
        definition["result"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with("/cogitator.yml")),
        "{definition}"
    );
    assert_eq!(definition["result"]["range"]["start"]["line"], 1);

    write_config(&project, &mut lsp, "inlay:\n  vars: false\n");
    lsp.wait_until("var hints are off", |lsp| {
        var_hints(lsp, &uri).is_empty().then_some(())
    });
}

#[test]
fn a_var_in_yaml_is_coloured_and_unresolved_when_undefined() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let text = "models:\n  - name: customers\n    config:\n      where: \"{{ var('jaffle_string') }} and {{ var('nope') }}\"\n";
    let response = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.highlight", "arguments": [{"text": text, "language": "yaml"}]}),
    );
    let capture_at = |word: &str| {
        let column = text
            .lines()
            .nth(3)
            .unwrap()
            .find(&format!("'{word}'"))
            .unwrap()
            + 1;
        response["result"]["spans"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| {
                s["range"]["start"]["line"] == 3
                    && s["range"]["start"]["character"] == column as u64
                    && s["layer"] == "dbt"
            })
            .map(|s| s["capture"].as_str().unwrap().to_string())
    };
    assert_eq!(
        capture_at("jaffle_string").as_deref(),
        Some("variable.dbt.var"),
        "{response}"
    );
    assert_eq!(
        capture_at("nope").as_deref(),
        Some("variable.dbt.var.unresolved"),
        "{response}"
    );
}

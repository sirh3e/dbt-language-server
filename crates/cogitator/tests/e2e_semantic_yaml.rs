//! `textDocument/semanticTokens` for YAML property files
//! (specs/highlighting §12): the dbt-aware captures `dbt.highlight`
//! answers, through the standard channel a stock client colours.

mod harness;
use harness::jaffle;
use serde_json::{json, Value};

/// `(line, character, length, type, modifiers)` of every token.
fn decode(response: &Value) -> Vec<(u32, u32, u32, String, Vec<String>)> {
    let data: Vec<u32> = response["result"]["data"]
        .as_array()
        .unwrap_or_else(|| panic!("data: {response}"))
        .iter()
        .map(|v| v.as_u64().unwrap() as u32)
        .collect();
    let (mut line, mut col) = (0, 0);
    data.as_chunks::<5>()
        .0
        .iter()
        .map(|c| {
            line += c[0];
            col = if c[0] == 0 { col + c[1] } else { c[1] };
            let ty = cogitator_lsp::SEMANTIC_TOKEN_TYPES[c[3] as usize].to_string();
            let modifiers = cogitator_lsp::SEMANTIC_TOKEN_MODIFIERS
                .iter()
                .enumerate()
                .filter(|(i, _)| c[4] & (1 << i) != 0)
                .map(|(_, m)| m.to_string())
                .collect();
            (line, col, c[2], ty, modifiers)
        })
        .collect()
}

fn has(
    tokens: &[(u32, u32, u32, String, Vec<String>)],
    line: u32,
    ty: &str,
    modifiers: &[&str],
) -> bool {
    tokens
        .iter()
        .any(|t| t.0 == line && t.3 == ty && modifiers.iter().all(|m| t.4.iter().any(|x| x == m)))
}

#[test]
fn a_property_file_gets_its_dbt_captures_as_semantic_tokens() {
    let project = jaffle();
    std::fs::write(
        project.root.join("models/extra.yml"),
        "version: 2\nmodels:\n  - name: orders\n    columns:\n      - name: customer_id\n        data_tests:\n          - relationships:\n              to: ref('no_such_model')\n              field: id\n",
    )
    .unwrap();
    let (mut lsp, _uri) = project.booted();
    let schema = cogitator_lsp::fs_path_to_uri(&project.root.join("models/schema.yml"));
    let response = lsp.request(
        "textDocument/semanticTokens/full",
        json!({"textDocument": {"uri": schema}}),
    );
    let tokens = decode(&response);
    // `  - name: customers` (line 3): a model's declaration
    assert!(
        has(&tokens, 3, "type", &["declaration", "dbt"]),
        "{tokens:?}"
    );
    // `      - name: customer_id` (line 8): a column
    assert!(has(&tokens, 8, "property", &["dbt"]), "{tokens:?}");
    // `          - unique` (line 11): a test
    assert!(has(&tokens, 11, "function", &["dbt"]), "{tokens:?}");
    // no YAML base-layer token: the client colours YAML syntax itself
    assert!(
        !tokens.iter().any(|t| t.4.is_empty() && t.3 == "property"),
        "{tokens:?}"
    );

    let extra = cogitator_lsp::fs_path_to_uri(&project.root.join("models/extra.yml"));
    let response = lsp.request(
        "textDocument/semanticTokens/full",
        json!({"textDocument": {"uri": extra}}),
    );
    let tokens = decode(&response);
    assert!(
        has(&tokens, 7, "type", &["dbt", "unresolved"]),
        "ref('no_such_model'): {tokens:?}"
    );
    assert!(has(&tokens, 8, "parameter", &["dbt"]), "{tokens:?}");
}

#[test]
fn a_range_answers_the_tokens_of_its_lines() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let schema = cogitator_lsp::fs_path_to_uri(&project.root.join("models/schema.yml"));
    let response = lsp.request(
        "textDocument/semanticTokens/range",
        json!({
            "textDocument": {"uri": schema},
            "range": {"start": {"line": 3, "character": 0}, "end": {"line": 3, "character": 99}}
        }),
    );
    let tokens = decode(&response);
    assert!(!tokens.is_empty(), "{response}");
    assert!(tokens.iter().all(|t| t.0 == 3), "{tokens:?}");

    // the SQL stream of a model takes ranges too
    let model = cogitator_lsp::fs_path_to_uri(&project.root.join("models/customers.sql"));
    let full = decode(&lsp.request(
        "textDocument/semanticTokens/full",
        json!({"textDocument": {"uri": model}}),
    ));
    let ranged = decode(&lsp.request(
        "textDocument/semanticTokens/range",
        json!({
            "textDocument": {"uri": model},
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": 2, "character": 0}}
        }),
    ));
    let expected: Vec<_> = full.into_iter().filter(|t| t.0 <= 2).collect();
    assert_eq!(ranged, expected);
}

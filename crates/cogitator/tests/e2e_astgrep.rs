//! ast-grep end to end: `dbt.astGrep` over the open model and over the
//! project, a rewrite applied through workspace/applyEdit, and the
//! project's sgconfig.yml rules as diagnostics with their fixes.

mod harness;
use harness::jaffle;
use serde_json::{json, Value};

fn grep(lsp: &mut harness::client::Lsp, arg: Value) -> Value {
    lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.astGrep", "arguments": [arg]}),
    )
}

#[test]
fn a_pattern_finds_the_refs_of_the_open_model_with_their_metavariables() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let response = grep(&mut lsp, json!({"uri": uri, "pattern": "{{ ref($M) }}"}));
    let result = &response["result"];
    assert_eq!(result["language"], "dbt", "{response}");
    let matches = result["matches"].as_array().expect("matches");
    assert_eq!(
        matches.len(),
        4,
        "customers.sql refs four staging models: {response}"
    );
    assert_eq!(matches[0]["text"], "{{ ref('stg_customers') }}");
    assert_eq!(matches[0]["env"]["M"], "'stg_customers'");
    assert_eq!(matches[0]["range"]["start"]["line"], 2);
    assert_eq!(matches[0]["uri"], uri);
}

#[test]
fn without_a_uri_the_whole_project_is_searched() {
    let project = jaffle();
    let (mut lsp, _uri) = project.booted();
    let response = grep(&mut lsp, json!({"pattern": "{{ ref('stg_orders') }}"}));
    let matches = response["result"]["matches"].as_array().expect("matches");
    let files: std::collections::BTreeSet<&str> =
        matches.iter().map(|m| m["uri"].as_str().unwrap()).collect();
    assert!(
        files.len() >= 2,
        "stg_orders is ref'd by several models: {response}"
    );
    assert!(files.iter().all(|f| f.ends_with(".sql")), "{response}");
    assert_eq!(response["result"]["truncated"], false);
}

#[test]
fn a_rewrite_is_previewed_and_applied_on_request() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let arg = json!({"uri": uri, "pattern": "{{ ref($M) }}", "rewrite": "{{ ref('core', $M) }}"});
    let preview = grep(&mut lsp, arg.clone());
    assert_eq!(
        preview["result"]["matches"][0]["replacement"], "{{ ref('core', 'stg_customers') }}",
        "{preview}"
    );

    let mut apply = arg;
    apply["apply"] = json!(true);
    let id = lsp.request_nowait(
        "workspace/executeCommand",
        json!({"command": "dbt.astGrep", "arguments": [apply]}),
    );
    let edit = lsp.wait_server_request("workspace/applyEdit", |_| true);
    let changes = edit["params"]["edit"]["documentChanges"]
        .as_array()
        .expect("changes");
    assert_eq!(changes.len(), 1, "{edit}");
    assert_eq!(changes[0]["edits"].as_array().unwrap().len(), 4, "{edit}");
    assert_eq!(
        changes[0]["edits"][0]["newText"],
        "{{ ref('core', 'stg_customers') }}"
    );
    let _ = lsp.response_for(id);
}

#[test]
fn a_bad_language_or_no_pattern_is_refused_with_guidance() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let response = grep(&mut lsp, json!({"uri": uri}));
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("pattern")),
        "{response}"
    );
    let response = grep(
        &mut lsp,
        json!({"uri": uri, "pattern": "x", "language": "rust"}),
    );
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("rust")),
        "{response}"
    );
}

const SGCONFIG: &str = "ruleDirs:\n  - rules\n";
const RULE: &str = r#"id: select-star-of-a-ref
language: dbt
severity: warning
message: "select * from a ref: name the columns"
note: a new column upstream changes this model silently
rule:
  pattern: "{{ ref('stg_customers') }}"
fix: "{{ ref('stg_customers_v2') }}"
"#;

fn diagnostics(lsp: &mut harness::client::Lsp, uri: &str) -> Vec<Value> {
    let pulled = lsp.request(
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": uri}}),
    );
    pulled["result"]["items"].as_array().expect("items").clone()
}

#[test]
fn the_projects_rules_are_diagnostics_and_their_fixes_quickfixes() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let ours = |items: &[Value]| {
        items
            .iter()
            .filter(|d| d["source"] == "ast-grep")
            .cloned()
            .collect::<Vec<_>>()
    };
    assert!(
        ours(&diagnostics(&mut lsp, &uri)).is_empty(),
        "no sgconfig.yml, no rules"
    );

    std::fs::write(project.root.join("sgconfig.yml"), SGCONFIG).unwrap();
    std::fs::create_dir_all(project.root.join("rules")).unwrap();
    std::fs::write(project.root.join("rules/select-star.yml"), RULE).unwrap();

    let found = ours(&diagnostics(&mut lsp, &uri));
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0]["code"], "select-star-of-a-ref");
    assert_eq!(found[0]["severity"], 2);
    assert!(
        found[0]["message"]
            .as_str()
            .unwrap()
            .contains("upstream changes"),
        "the note: {found:?}"
    );
    assert_eq!(found[0]["range"]["start"]["line"], 2);

    let actions = lsp.request(
        "textDocument/codeAction",
        json!({"textDocument": {"uri": uri},
               "range": found[0]["range"],
               "context": {"diagnostics": found}}),
    );
    let fix = actions["result"]
        .as_array()
        .expect("actions")
        .iter()
        .find(|a| {
            a["title"]
                .as_str()
                .is_some_and(|t| t.contains("select-star-of-a-ref"))
        })
        .unwrap_or_else(|| panic!("the rule's quickfix: {actions}"))
        .clone();
    assert_eq!(fix["kind"], "quickfix");
    assert_eq!(
        fix["edit"]["documentChanges"][0]["edits"][0]["newText"],
        "{{ ref('stg_customers_v2') }}"
    );

    // a changed rule file is read again on the next pull
    std::fs::write(
        project.root.join("rules/select-star.yml"),
        RULE.replace("severity: warning", "severity: off"),
    )
    .unwrap();
    assert!(
        ours(&diagnostics(&mut lsp, &uri)).is_empty(),
        "the rule is off now"
    );
}

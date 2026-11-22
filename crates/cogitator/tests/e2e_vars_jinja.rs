//! Vars and Jinja beyond values, end to end on the jaffle fixture:
//! renaming a var (calls and key at once), unused-var hints on
//! dbt_project.yml, env_var() hints and warnings, and the inline Jinja
//! preview on hover and (opt-in) as inlay hints.

mod harness;
use harness::{jaffle, write_config};
use serde_json::{json, Value};

fn position_of(text: &str, needle: &str, into: usize) -> (u64, u64) {
    let (line, l) = text
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(needle))
        .unwrap();
    (line as u64, (l.find(needle).unwrap() + into) as u64)
}

fn open(lsp: &mut harness::client::Lsp, uri: &str, text: &str) {
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "sql", "version": 1, "text": text}}),
    );
}

fn hints(lsp: &mut harness::client::Lsp, uri: &str) -> Vec<String> {
    let response = lsp.request(
        "textDocument/inlayHint",
        json!({"textDocument": {"uri": uri},
               "range": {"start": {"line": 0, "character": 0}, "end": {"line": 200, "character": 0}}}),
    );
    response["result"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|h| h["label"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn renaming_a_var_rewrites_its_calls_and_its_key() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let text = std::fs::read_to_string(project.root.join("models/customers.sql")).unwrap();
    let (line, character) = position_of(&text, "jaffle_string')", 2);
    let at =
        json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}});

    let prepared = lsp.request("textDocument/prepareRename", at.clone());
    assert_eq!(
        prepared["result"]["placeholder"], "jaffle_string",
        "{prepared}"
    );

    let mut params = at;
    params["newName"] = json!("jaffle_text");
    let renamed = lsp.request("textDocument/rename", params);
    let changes = renamed["result"]["documentChanges"].as_array().unwrap();
    let mut files: Vec<String> = changes
        .iter()
        .map(|c| {
            let u = c["textDocument"]["uri"].as_str().unwrap();
            assert!(
                c["edits"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|e| e["newText"] == "jaffle_text"),
                "{c}"
            );
            u.rsplit('/').next().unwrap().to_string()
        })
        .collect();
    files.sort();
    assert_eq!(files, vec!["customers.sql", "dbt_project.yml"], "{renamed}");
}

#[test]
fn unused_vars_are_hinted_on_dbt_project_yml() {
    let project = jaffle();
    let (mut lsp, _) = project.booted();
    let path = project.root.join("dbt_project.yml");
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    lsp.notify(
        "textDocument/didOpen",
        json!({"textDocument": {"uri": uri, "languageId": "yaml", "version": 1,
               "text": std::fs::read_to_string(&path).unwrap()}}),
    );
    let published = lsp.wait_notification("textDocument/publishDiagnostics", |n| {
        n["params"]["uri"] == uri
    });
    let unused: Vec<&Value> = published["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "unused-var")
        .collect();
    let names: Vec<&str> = unused
        .iter()
        .map(|d| d["message"].as_str().unwrap().split('\'').nth(1).unwrap())
        .collect();
    assert_eq!(names, vec!["global_count", "jaffle_number"], "{published}");
    assert!(unused
        .iter()
        .all(|d| d["severity"] == 4 && d["tags"] == json!([1])));
}

#[test]
fn env_vars_show_their_values_and_warn_when_unset() {
    let project = jaffle();
    let (mut lsp, _) = project.booted_with(
        &[
            ("COGITATOR_E2E_SET", "here"),
            ("COGITATOR_E2E_TOKEN", "s3cret"),
        ],
        json!({}),
    );
    let path = project.root.join("models/env_probe.sql");
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let text =
        "select '{{ env_var(\"COGITATOR_E2E_SET\") }}', '{{ env_var(\"COGITATOR_E2E_TOKEN\") }}', \
                '{{ env_var(\"COGITATOR_E2E_UNSET\") }}'\n";
    open(&mut lsp, &uri, text);
    let published = lsp.wait_notification("textDocument/publishDiagnostics", |n| {
        n["params"]["uri"] == uri
    });
    let unset: Vec<&Value> = published["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "env-var-unset")
        .collect();
    assert_eq!(unset.len(), 1, "{published}");
    assert!(unset[0]["message"]
        .as_str()
        .unwrap()
        .contains("COGITATOR_E2E_UNSET"));

    let labels: Vec<String> = hints(&mut lsp, &uri)
        .into_iter()
        .filter(|l| l.starts_with('='))
        .collect();
    assert_eq!(labels, vec!["= 'here'", "= '••••••'", "= ? unset"]);

    let (line, character) = position_of(text, "COGITATOR_E2E_SET", 2);
    let hover = lsp.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}}),
    );
    let contents = hover["result"]["contents"].as_str().unwrap();
    assert!(
        contents.contains("env_var COGITATOR_E2E_SET: here"),
        "{hover}"
    );

    write_config(&project, &mut lsp, "inlay:\n  env_vars: false\n");
    lsp.wait_until("env_var hints are off", |lsp| {
        hints(lsp, &uri)
            .iter()
            .all(|l| !l.starts_with('='))
            .then_some(())
    });
}

#[test]
fn a_jinja_expression_previews_on_hover_and_as_an_opt_in_hint() {
    let project = jaffle();
    let (mut lsp, uri) = project.booted();
    let text = std::fs::read_to_string(project.root.join("models/customers.sql")).unwrap();
    let (line, character) = position_of(&text, "{{ ref('stg_customers') }}", 1);
    let hover = lsp.request(
        "textDocument/hover",
        json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}}),
    );
    let contents = hover["result"]["contents"].as_str().unwrap();
    assert!(contents.contains("Renders to (preview):"), "{hover}");
    assert!(contents.contains("stg_customers"), "{hover}");

    assert!(
        !hints(&mut lsp, &uri).iter().any(|l| l.starts_with('⇒')),
        "jinja hints are off by default"
    );
    write_config(&project, &mut lsp, "inlay:\n  jinja: true\n");
    let label = lsp.wait_until("jinja hints are on", |lsp| {
        hints(lsp, &uri)
            .into_iter()
            .find(|l| l.starts_with('⇒') && l.contains("stg_customers"))
    });
    assert!(label.starts_with("⇒ "), "{label}");
}

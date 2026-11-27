//! The dialogs' finders (for Zed, whose buttons carry no free text): fixed
//! `dbt.search` queries about the open model, fixed `dbt.astGrep` patterns,
//! and switching the profiles directory to one whose profiles.yml exists —
//! end to end on copies of the jaffle fixture, the harness pressing the
//! buttons.

mod harness;
use harness::{jaffle, Lsp};
use serde_json::{json, Value};

/// A server whose client asked for the dialogs (as the Zed extension does),
/// with `model` open.
fn booted(project: &harness::fixture::TestProject, model: &str) -> (Lsp, String) {
    let mut lsp = project.server().spawn();
    let path = project.root.join(model);
    let uri = cogitator_lsp::fs_path_to_uri(&path);
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
               "text": std::fs::read_to_string(&path).unwrap()}}),
    );
    lsp.wait_refresh(1);
    (lsp, uri)
}

/// Open the menu as the "dbt…" code action does, the buttons already queued.
fn open_menu(lsp: &mut Lsp, uri: &str, buttons: &[&str]) {
    lsp.answer_prompts(buttons);
    let reply = lsp.request(
        "workspace/executeCommand",
        json!({"command": "dbt.menu",
               "arguments": [{"uri": uri, "position": {"line": 0, "character": 0}}]}),
    );
    assert_eq!(reply["result"], Value::Null, "{reply}");
}

fn opened(lsp: &mut Lsp, tail: &str) -> String {
    let shown = lsp.wait_server_request("window/showDocument", |frame| {
        frame["params"]["uri"]
            .as_str()
            .is_some_and(|u| u.ends_with(tail))
    });
    let uri = shown["params"]["uri"].as_str().unwrap();
    std::fs::read_to_string(cogitator_lsp::uri_to_fs_path(uri).unwrap()).unwrap()
}

fn titles(prompt: &Value) -> Vec<String> {
    prompt["params"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["title"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn who_reads_a_model_is_a_button_and_its_matches_link_their_lines() {
    let project = jaffle();
    let (mut lsp, uri) = booted(&project, "models/staging/stg_payments.sql");
    open_menu(
        &mut lsp,
        &uri,
        &[
            "Model ›",
            "More…",
            "More…",
            "Find in the project ›",
            "Who reads it",
        ],
    );
    let text = opened(&mut lsp, "dialogs/search.md");
    assert!(text.starts_with("# Who reads it\n"), "{text}");
    assert!(text.contains("Query: `reads stg_payments`"), "{text}");
    assert!(text.contains("## models/customers.sql"), "{text}");
    assert!(text.contains("/models/customers.sql#L"), "{text}");
    assert!(text.contains("## Not searched"), "{text}");
}

#[test]
fn which_models_use_a_column_is_asked_from_the_models_columns() {
    let project = jaffle();
    let (mut lsp, uri) = booted(&project, "models/staging/stg_payments.sql");
    open_menu(
        &mut lsp,
        &uri,
        &[
            "Model ›",
            "More…",
            "More…",
            "Find in the project ›",
            "Which models use which of its columns ›",
            "amount",
        ],
    );
    let columns = lsp.wait_server_request("window/showMessageRequest", |frame| {
        frame["params"]["message"] == "Which models use which of its columns?"
    });
    assert!(titles(&columns).contains(&"amount".to_string()));
    let text = opened(&mut lsp, "dialogs/search.md");
    assert!(
        text.contains("Query: `uses column stg_payments.amount`"),
        "{text}"
    );
    assert!(text.contains("models searched."), "{text}");
}

#[test]
fn a_jinja_pattern_is_found_over_the_project_or_in_the_file() {
    let project = jaffle();
    let (mut lsp, uri) = booted(&project, "models/customers.sql");
    open_menu(
        &mut lsp,
        &uri,
        &["Project ›", "Find a Jinja pattern ›", "Every ref()"],
    );
    let text = opened(&mut lsp, "dialogs/astGrep.md");
    assert!(text.starts_with("# Every ref()\n"), "{text}");
    assert!(text.contains("## models/customers.sql"), "{text}");
    assert!(text.contains("## models/orders.sql"), "{text}");
    assert!(text.contains("$M = "), "the metavariable is shown: {text}");

    open_menu(
        &mut lsp,
        &uri,
        &[
            "Model ›",
            "More…",
            "More…",
            "Find a Jinja pattern in this file ›",
            "Every ref()",
        ],
    );
    let text = opened(&mut lsp, "dialogs/astGrep.customers.md");
    assert!(text.contains("in 1 files"), "{text}");
    assert!(text.contains("## models/customers.sql"), "{text}");
    assert!(!text.contains("## models/orders.sql"), "{text}");
}

#[test]
fn the_profiles_directory_is_switched_to_one_whose_profiles_yml_exists() {
    let project = jaffle();
    // a second profiles.yml, in the (sandboxed) home's .dbt
    let home_dbt = project.home.join(".dbt");
    std::fs::create_dir_all(&home_dbt).unwrap();
    std::fs::copy(
        project.root.join(".dbt/profiles.yml"),
        home_dbt.join("profiles.yml"),
    )
    .unwrap();
    let (mut lsp, uri) = booted(&project, "models/customers.sql");
    let in_use = format!("{} ✓", project.root.join(".dbt").display());
    let other = home_dbt.display().to_string();
    open_menu(
        &mut lsp,
        &uri,
        &["Session ›", "Switch the profiles directory ›", &other],
    );
    let dirs = lsp.wait_server_request("window/showMessageRequest", |frame| {
        frame["params"]["message"] == "Use the profiles.yml of which directory?"
    });
    // the project's profiles.yml (none) is not offered
    assert_eq!(titles(&dirs), vec![in_use, other.clone(), "Cancel".into()]);
    let said = lsp.wait_notification("window/showMessage", |frame| {
        frame["params"]["message"]
            .as_str()
            .is_some_and(|m| m.starts_with("Profiles directory: "))
    });
    assert!(
        said["params"]["message"].as_str().unwrap().contains(&other),
        "{said}"
    );
}

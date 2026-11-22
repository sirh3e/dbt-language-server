use std::path::{Path, PathBuf};

use super::*;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn uri(rel: &str) -> String {
    cogitator_lsp::fs_path_to_uri(&root().join(rel))
}

fn ctx() -> DbtContext {
    crate::build_context(root().to_str().unwrap())
}

fn disk(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

/// (file the edit touches, new texts of its edits)
fn summary(edit: &WorkspaceEdit) -> Vec<(String, Vec<String>)> {
    edit.document_changes
        .iter()
        .map(|c| match c {
            DocumentChange::Edit(e) => (
                e.text_document.uri.rsplit('/').next().unwrap().to_string(),
                e.edits.iter().map(|t| t.new_text.clone()).collect(),
            ),
            other => panic!("only text edits: {other:?}"),
        })
        .collect()
}

#[test]
fn renames_edit_what_dbt_sees_and_nothing_else() {
    let ctx = ctx();
    for (old, new, touched) in [
        // a model renamed: its dependent's ref and its YAML entry
        (
            "models/staging/stg_orders.sql",
            "models/staging/stg_order_rows.sql",
            vec![
                ("customer_orders_v1.sql", 1),
                ("customers.sql", 1),
                ("orders.sql", 1),
                ("schema.yml", 1),
            ],
        ),
        // moved to another folder, same name: nothing changes
        (
            "models/staging/stg_orders.sql",
            "models/stg_orders.sql",
            vec![],
        ),
        // a macro file, a properties file, a folder
        ("macros/full_name.sql", "macros/whole_name.sql", vec![]),
        ("models/schema.yml", "models/properties.yml", vec![]),
        ("models/staging", "models/base", vec![]),
        // not a model name, another kind of file
        ("models/customers.sql", "models/bad-name.sql", vec![]),
        ("models/customers.sql", "models/customers.py", vec![]),
    ] {
        let edit = will_rename_files(&ctx, &[(uri(old), uri(new))], &disk);
        let got: Vec<(String, usize)> = summary(&edit)
            .into_iter()
            .map(|(file, texts)| {
                assert!(texts.iter().all(|t| !t.is_empty()));
                (file, texts.len())
            })
            .collect();
        let mut got_files: Vec<String> = got.iter().map(|(f, _)| f.clone()).collect();
        got_files.sort();
        let mut want: Vec<String> = touched.iter().map(|(f, _)| f.to_string()).collect();
        want.sort();
        assert_eq!(got_files, want, "{old} → {new}: {got:?}");
    }
}

#[test]
fn two_renames_in_one_request_share_one_edit_per_file() {
    let ctx = ctx();
    let edit = will_rename_files(
        &ctx,
        &[
            (uri("models/customers.sql"), uri("models/clients.sql")),
            (uri("models/orders.sql"), uri("models/sales.sql")),
        ],
        &disk,
    );
    let summary = summary(&edit);
    let schema: Vec<&Vec<String>> = summary
        .iter()
        .filter(|(f, _)| f == "schema.yml")
        .map(|(_, t)| t)
        .collect();
    assert_eq!(schema.len(), 1, "{summary:?}");
    assert!(schema[0].contains(&"clients".to_string()));
    assert!(schema[0].contains(&"sales".to_string()));
}

#[test]
fn deletes_name_the_dependents_left_dangling() {
    let ctx = ctx();
    for (deleted, want) in [
        (
            vec!["models/customers.sql"],
            vec![("customers", vec!["order_counts"])],
        ),
        (
            vec!["models/staging/stg_payments.sql"],
            vec![("stg_payments", vec!["customers", "orders"])],
        ),
        (vec!["macros/full_name.sql"], vec![]),
    ] {
        let uris: Vec<String> = deleted.iter().map(|r| uri(r)).collect();
        let got = deleted_with_dependents(&ctx, &uris);
        let got: Vec<(String, Vec<String>)> =
            got.into_iter().map(|m| (m.model, m.dependents)).collect();
        let want: Vec<(String, Vec<String>)> = want
            .into_iter()
            .map(|(m, d)| (m.to_string(), d.into_iter().map(str::to_string).collect()))
            .collect();
        // the fixture may grow dependents: compare the deleted models and
        // that every wanted dependent is named
        assert_eq!(
            got.iter().map(|(m, _)| m).collect::<Vec<_>>(),
            want.iter().map(|(m, _)| m).collect::<Vec<_>>(),
            "{deleted:?}: {got:?}"
        );
        for ((_, g), (_, w)) in got.iter().zip(&want) {
            assert!(w.iter().all(|d| g.contains(d)), "{deleted:?}: {got:?}");
        }
    }
}

#[test]
fn dependents_deleted_with_it_do_not_count() {
    let ctx = ctx();
    let uris: Vec<String> = [
        "models/staging/stg_payments.sql",
        "models/customers.sql",
        "models/orders.sql",
    ]
    .iter()
    .map(|r| uri(r))
    .collect();
    let got = deleted_with_dependents(&ctx, &uris);
    assert!(got.iter().all(|m| m.model != "stg_payments"), "{got:?}");
    assert!(got.iter().all(|m| m
        .dependents
        .iter()
        .all(|d| d != "customers" && d != "orders")));
}

#[test]
fn a_deleted_folder_deletes_its_models() {
    let ctx = ctx();
    let got = deleted_with_dependents(&ctx, &[uri("models/staging")]);
    assert!(got.iter().any(|m| m.model == "stg_orders"), "{got:?}");
    assert!(got
        .iter()
        .all(|m| m.dependents.iter().all(|d| !d.starts_with("stg_"))));
}

#[test]
fn the_warning_names_models_and_dependents() {
    let one = DeletedModel {
        model: "orders".into(),
        dependents: vec!["a".into(), "b".into()],
    };
    for (models, want) in [
        (vec![], None),
        (
            vec![one.clone()],
            Some("Deleting a model with dependents: `orders` is still ref()'d by a, b. Those refs will dangle."),
        ),
        (
            vec![one.clone(), DeletedModel { model: "x".into(), dependents: vec!["y".into()] }],
            Some("Deleting models with dependents: `orders` is still ref()'d by a, b; `x` is still ref()'d by y. Those refs will dangle."),
        ),
    ] {
        assert_eq!(delete_warning(&models).as_deref(), want);
    }
}

#[test]
fn what_touches_the_project() {
    for (u, want) in [
        ("file:///p/models/a.sql", true),
        ("file:///p/models/a.yml", true),
        ("file:///p/seeds/a.csv", true),
        ("file:///p/models/sub", true),
        ("file:///p/models/notes.txt", false),
        ("", false),
    ] {
        assert_eq!(touches_project(u), want, "{u}");
    }
}

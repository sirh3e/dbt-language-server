//! Var rename by scope, unused-var hints, and env_var() hints, hover,
//! definition and diagnostics, over a small project written per test.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cogitator_analysis::context::{ContextBuilder, VarOverride};
use cogitator_analysis::features;
use cogitator_analysis::{DbtContext, Documents};
use cogitator_lsp::{fs_path_to_uri, DocumentChange, Position, Range};
use pretty_assertions::assert_eq;

fn project(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("cogitator-vars-more-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let write = |rel: &str, text: &str| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write(
        "dbt_project.yml",
        "name: shop\nprofile: shop\nmodel-paths: [models]\n\
         vars:\n  shared: root\n  unused_top: 1\n  shop:\n    scoped: 2\n    shop_unused: 3\n  utils:\n    tuned: 3\n    not_read: 4\n",
    );
    write(
        "models/orders.sql",
        "select {{ var('shared') }}, {{ var('scoped') }}, {{ env_var('COGITATOR_T_SET') }},\n\
         {{ env_var('COGITATOR_T_UNSET') }}, {{ env_var('COGITATOR_T_DOT') }},\n\
         {{ env_var('COGITATOR_T_UNSET', 'dev') }}, {{ env_var('COGITATOR_T_PASSWORD') }}\n",
    );
    write(".env", "# local\nCOGITATOR_T_DOT=from-dot\n");
    write(
        "dbt_packages/utils/dbt_project.yml",
        "name: utils\nmodel-paths: [models]\nvars:\n  tuned: 0\n",
    );
    write(
        "dbt_packages/utils/models/pkg.sql",
        "select {{ var('tuned') }}, {{ var('shared') }}\n",
    );
    root
}

fn build(root: &Path, overrides: Vec<VarOverride>) -> DbtContext {
    let mut builder = ContextBuilder::default();
    builder.set_var_overrides(overrides);
    let mut ctx = builder.build(root.to_str().unwrap());
    ctx.env.process = HashMap::from([
        ("COGITATOR_T_SET".into(), "prod".into()),
        ("COGITATOR_T_PASSWORD".into(), "hunter2".into()),
    ]);
    ctx
}

fn uri(root: &Path, rel: &str) -> String {
    fs_path_to_uri(&root.join(rel))
}

fn open(ctx: &DbtContext, root: &Path, rel: &str) -> (Documents, String) {
    let mut docs = Documents::default();
    let u = uri(root, rel);
    let text = std::fs::read_to_string(root.join(rel)).unwrap();
    docs.open(u.clone(), &text, &ctx.dialect);
    (docs, u)
}

fn at(docs: &Documents, uri: &str, needle: &str) -> Position {
    let text = &docs.get(uri).unwrap().text;
    let byte = text.find(needle).unwrap() + 1;
    let line = text[..byte].matches('\n').count() as u32;
    let start = text[..byte].rfind('\n').map_or(0, |i| i + 1);
    Position {
        line,
        character: (byte - start) as u32,
    }
}

/// file name → the edited texts, applied.
fn applied(edit: &cogitator_lsp::WorkspaceEdit) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for change in &edit.document_changes {
        let DocumentChange::Edit(e) = change else {
            continue;
        };
        let path = cogitator_lsp::uri_to_fs_path(&e.text_document.uri).unwrap();
        let mut text = std::fs::read_to_string(&path).unwrap();
        let mut edits = e.edits.clone();
        edits.sort_by_key(|t| std::cmp::Reverse((t.range.start.line, t.range.start.character)));
        for t in edits {
            text = cogitator_analysis::util::apply_incremental_change(&text, t.range, &t.new_text);
        }
        let rel = path.split("cogitator-vars-more-").nth(1).unwrap();
        let rel = rel.split_once('/').unwrap().1.to_string();
        out.push((rel, text));
    }
    out.sort();
    out
}

#[test]
fn a_rename_from_a_call_moves_the_scoped_key_and_its_calls() {
    let root = project("rename-scoped");
    let ctx = build(&root, Vec::new());
    let (docs, orders) = open(&ctx, &root, "models/orders.sql");
    let doc = docs.get(&orders);
    let pos = at(&docs, &orders, "scoped");
    let prep = features::prepare_rename_var(&ctx, doc, &orders, pos).unwrap();
    assert_eq!(prep.placeholder, "scoped");
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let edit = features::rename_var(&ctx, doc, &orders, pos, "scoped2", &read).unwrap();
    let files = applied(&edit);
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["dbt_project.yml", "models/orders.sql"]);
    assert!(files[0].1.contains("    scoped2: 2\n"), "{}", files[0].1);
    assert!(
        files[1].1.contains("{{ var('scoped2') }}"),
        "{}",
        files[1].1
    );
}

#[test]
fn a_root_global_seen_by_a_package_renames_its_calls_too() {
    let root = project("rename-global");
    let ctx = build(&root, Vec::new());
    let (docs, project) = open(&ctx, &root, "dbt_project.yml");
    let doc = docs.get(&project);
    let pos = at(&docs, &project, "shared:");
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let edit = features::rename_var(&ctx, doc, &project, pos, "common", &read).unwrap();
    let files = applied(&edit);
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "dbt_packages/utils/models/pkg.sql",
            "dbt_project.yml",
            "models/orders.sql"
        ]
    );
    // the package's own `tuned` and the root's block for it are untouched
    assert!(files[0].1.contains("var('common')") && files[0].1.contains("var('tuned')"));
    // a name a renamed scope already sees is refused
    assert!(features::rename_var(&ctx, doc, &project, pos, "scoped", &read).is_none());
}

#[test]
fn a_package_scoped_block_key_renames_only_that_package() {
    let root = project("rename-pkg");
    let ctx = build(&root, Vec::new());
    let (docs, project) = open(&ctx, &root, "dbt_project.yml");
    let doc = docs.get(&project);
    let pos = at(&docs, &project, "tuned:");
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let edit = features::rename_var(&ctx, doc, &project, pos, "tuning", &read).unwrap();
    let files = applied(&edit);
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    // the root's vars.utils.tuned wins in utils; utils' own `tuned: 0`
    // is seen only by utils, which resolves to the root's block → moves
    assert_eq!(
        names,
        vec![
            "dbt_packages/utils/dbt_project.yml",
            "dbt_packages/utils/models/pkg.sql",
            "dbt_project.yml"
        ]
    );
}

#[test]
fn unused_keys_are_hints_with_a_removal() {
    let root = project("unused");
    let ctx = build(&root, Vec::new());
    let (docs, project) = open(&ctx, &root, "dbt_project.yml");
    let text = &docs.get(&project).unwrap().text;
    let diags = features::unused_var_diagnostics(&ctx, &project, text);
    let names: Vec<String> = diags
        .iter()
        .map(|d| d.message.split('\'').nth(1).unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["unused_top", "shop_unused", "not_read"]);
    assert!(diags
        .iter()
        .all(|d| d.severity == 4 && d.tags == Some(vec![1])));
    let titles: Vec<String> = diags
        .iter()
        .filter_map(|d| match &d.data.as_ref()?.fix {
            cogitator_lsp::FixKind::MultiEdit { fixes } => Some(fixes[0].title.clone()),
            _ => None,
        })
        .collect();
    assert!(
        titles.iter().any(|t| t == "Remove unused var 'unused_top'"),
        "{titles:?}"
    );
    // other files never get them
    let orders = uri(&root, "models/orders.sql");
    assert!(features::unused_var_diagnostics(&ctx, &orders, "").is_empty());
}

#[test]
fn env_vars_show_their_value_source_and_warn_when_dbt_would_fail() {
    let root = project("env");
    let ctx = build(&root, Vec::new());
    let (docs, orders) = open(&ctx, &root, "models/orders.sql");
    let doc = docs.get(&orders).unwrap();
    let labels: Vec<String> = features::inlay_hints(&ctx, doc, &orders, Range::default(), None)
        .into_iter()
        .map(|h| h.label)
        .filter(|l| l.starts_with('='))
        .collect();
    assert_eq!(
        labels,
        vec![
            "= 'root'",
            "= 2",
            "= 'prod'",
            "= ? unset",
            "= ? unset",
            "= 'dev' (default)",
            "= '••••••'"
        ]
    );
    let hover = features::hover(&ctx, doc, at(&docs, &orders, "COGITATOR_T_DOT"));
    assert!(
        hover.contains(".env line 2") && hover.contains("does not read .env"),
        "{hover}"
    );
    let def = features::definition(&ctx, doc, &orders, at(&docs, &orders, "COGITATOR_T_DOT"));
    assert!(def.uri.ends_with("/.env"), "{def:?}");
    assert_eq!(def.range.start.line, 1);
    let warned: Vec<String> = features::env_var_diagnostics(&ctx, doc)
        .into_iter()
        .map(|d| d.message.split('\'').nth(1).unwrap().to_string())
        .collect();
    assert_eq!(warned, vec!["COGITATOR_T_UNSET", "COGITATOR_T_DOT"]);
}

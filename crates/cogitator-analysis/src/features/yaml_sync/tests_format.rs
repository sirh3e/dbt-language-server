//! What the sync leaves as it was: comments, quoting, key order, blank
//! lines and other models' entries, byte for byte outside the edits.

use pretty_assertions::assert_eq;

use super::test_project::{base, Project};
use super::*;

const WRITTEN: &str = "\
# Staging models. Keep this comment.
version: 2

models:
  - name: other   # not a model of the project: left alone
    description: 'Single quoted, kept.'

  - name: stg_orders
    description: \"Orders, as loaded.\"
    config:
      tags: ['staging']   # inline list
    columns:
      # the key
      - name: id
        description: \"\"   # filled in
        data_tests:
          - unique
      - {name: customer, description: ''}
      - name: \"status\"

# trailing comment
";

#[test]
fn only_the_edited_spans_change() {
    let mut files = base();
    files[2].1 = WRITTEN;
    let project = Project::new(&files);
    let plan = project.sync(&SyncOptions::default());
    let text = project.read("models/staging/stg_orders.yml");
    insta::assert_snapshot!(text);
    // every line of the original is still there, in order
    let mut rest = text.lines();
    for line in WRITTEN.lines() {
        if line.contains("description: \"\"") {
            continue; // the one value the sync filled
        }
        assert!(
            rest.any(|l| l == line),
            "lost or reordered {line:?} in:\n{text}"
        );
    }
    let stg = plan
        .models
        .iter()
        .find(|m| m.model == "stg_orders")
        .expect("summary");
    // a flow-style item is not edited, and says so
    assert!(
        stg.skipped
            .iter()
            .any(|s| s.column.as_deref() == Some("customer") && s.reason.contains("flow style")),
        "{stg:#?}"
    );
    let file = plan
        .files
        .iter()
        .find(|f| f.path.ends_with("stg_orders.yml"))
        .expect("file");
    // the edits are hunks, not a rewrite of the file
    assert!(
        file.edits.iter().all(|e| e.range.start.line > 2),
        "{file:#?}"
    );
}

#[test]
fn a_file_without_a_final_newline_keeps_none() {
    let mut files = base();
    files[2].1 =
        "models:\n  - name: stg_orders\n    columns:\n      - name: id\n        description: Key.";
    let project = Project::new(&files);
    project.sync(&SyncOptions::default());
    let text = project.read("models/staging/stg_orders.yml");
    assert!(text.starts_with("models:\n  - name: stg_orders\n    columns:\n      - name: id\n        description: Key.\n      - name: amount\n"), "{text}");
}

#[test]
fn line_edits_rebuild_the_synced_text() {
    let mut files = base();
    files[2].1 = WRITTEN;
    let project = Project::new(&files);
    let plan = project.plan(&SyncOptions {
        prune: true,
        reorder: true,
        ..SyncOptions::default()
    });
    for file in plan.files.iter().filter(|f| !f.created) {
        let old = std::fs::read_to_string(&file.path).expect("old");
        assert_eq!(text::apply(&old, &file.edits), file.new_text);
    }
}

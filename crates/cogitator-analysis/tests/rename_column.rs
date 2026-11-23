//! Column rename and column references over a small project written to a
//! temporary directory: a chain of five consumers (models, a snapshot, a
//! singular test), YAML entries, tests, constraints, unit-test rows and a
//! semantic model, a re-alias that stops the walk, a declined lineage
//! that refuses, and a collision.

use std::path::{Path, PathBuf};

use cogitator_analysis::{build_context, features, Documents};
use cogitator_lsp::{DocumentChange, Position, WorkspaceEdit};

const PROJECT: &[(&str, &str)] = &[
    (
        "dbt_project.yml",
        "name: chain\nconfig-version: 2\nprofile: chain\nmodel-paths: [\"models\"]\nsnapshot-paths: [\"snapshots\"]\ntest-paths: [\"tests\"]\n",
    ),
    (
        "models/a.sql",
        "with s as (select * from {{ ref('raw') }})\nselect s.id, s.v as amount from s\n",
    ),
    ("models/b.sql", "select a.id, a.amount from {{ ref('a') }} a\n"),
    (
        "models/c.sql",
        "with b as (select * from {{ ref('b') }})\nselect * from b\n",
    ),
    ("models/d.sql", "select id, amount as total from {{ ref('c') }}\n"),
    (
        "models/f.sql",
        "select s.amount, s.id from {{ ref('snap') }} s where s.amount > 0\n",
    ),
    (
        "snapshots/snap.sql",
        "{% snapshot snap %}\nselect * from {{ ref('c') }}\n{% endsnapshot %}\n",
    ),
    (
        "tests/no_negative.sql",
        "select * from {{ ref('b') }} where amount < 0\n",
    ),
    (
        "models/schema.yml",
        r#"version: 2

models:
  - name: a
    constraints:
      - type: primary_key
        columns: [id, amount]
    columns:
      - name: amount
        description: the amount
        data_tests:
          - not_null
  - name: b
    columns:
      - name: amount
        data_tests:
          - relationships:
              to: ref('a')
              field: amount
  - name: c
    columns:
      - name: amount
  - name: d
    columns:
      - name: total

unit_tests:
  - name: b_passes_amount
    model: b
    given:
      - input: ref('a')
        rows:
          - {id: 1, amount: 2}
    expect:
      rows:
        - {id: 1, amount: 2}

semantic_models:
  - name: things
    model: ref('c')
    dimensions:
      - name: amount
        type: categorical
    measures:
      - name: total_amount
        expr: amount
        agg: sum
"#,
    ),
];

fn project(tag: &str, extra: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cogitator-rename-column-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    for (rel, text) in PROJECT.iter().chain(extra) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    root
}

/// The position of the first `needle` in a file, `offset` characters in.
fn at(root: &Path, rel: &str, needle: &str, offset: u32) -> (String, String, Position) {
    let text = std::fs::read_to_string(root.join(rel)).unwrap();
    let i = text.find(needle).unwrap();
    let line = text[..i].matches('\n').count() as u32;
    let col = (i - text[..i].rfind('\n').map_or(0, |j| j + 1)) as u32;
    let uri = cogitator_lsp::fs_path_to_uri(&root.join(rel));
    (
        uri,
        text,
        Position {
            line,
            character: col + offset,
        },
    )
}

/// Every file the edit touches, rewritten (byte columns, one line each).
fn apply(edit: &WorkspaceEdit) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for change in &edit.document_changes {
        let DocumentChange::Edit(doc) = change else {
            panic!("{change:?}")
        };
        let path = cogitator_lsp::uri_to_fs_path(&doc.text_document.uri).unwrap();
        let mut lines: Vec<String> = std::fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect();
        let mut edits = doc.edits.clone();
        edits.sort_by_key(|e| std::cmp::Reverse((e.range.start.line, e.range.start.character)));
        for e in edits {
            assert_eq!(e.range.start.line, e.range.end.line);
            let line = &mut lines[e.range.start.line as usize];
            line.replace_range(
                e.range.start.character as usize..e.range.end.character as usize,
                &e.new_text,
            );
        }
        let name = Path::new(&path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        out.push((name, lines.join("\n") + "\n"));
    }
    out.sort();
    out
}

#[test]
fn a_column_rename_walks_every_consumer_that_keeps_the_name() {
    let root = project("chain", &[]);
    let ctx = build_context(root.to_str().unwrap());
    let mut docs = Documents::default();
    let (uri, text, pos) = at(&root, "models/a.sql", "as amount", 3);
    docs.open(uri.clone(), &text, &ctx.dialect);
    let read = |p: &Path| std::fs::read_to_string(p).ok();

    let prep = features::prepare_rename_column(&ctx, docs.get(&uri), &uri, pos).unwrap();
    assert_eq!(prep.placeholder, "amount");

    let edit = features::rename_column_at(&ctx, docs.get(&uri), &uri, pos, "value", &read)
        .unwrap()
        .unwrap();
    let files = apply(&edit);
    let get = |name: &str| {
        files
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.as_str())
            .unwrap_or_else(|| panic!("{name}: {files:#?}"))
    };
    assert_eq!(
        get("a.sql"),
        "with s as (select * from {{ ref('raw') }})\nselect s.id, s.v as value from s\n"
    );
    assert_eq!(get("b.sql"), "select a.id, a.value from {{ ref('a') }} a\n");
    assert_eq!(
        get("d.sql"),
        "select id, value as total from {{ ref('c') }}\n"
    );
    assert_eq!(
        get("f.sql"),
        "select s.value, s.id from {{ ref('snap') }} s where s.value > 0\n"
    );
    assert_eq!(
        get("no_negative.sql"),
        "select * from {{ ref('b') }} where value < 0\n"
    );
    let yaml = get("schema.yml");
    for expected in [
        "columns: [id, value]",
        "  - name: a\n    constraints",
        "      - name: value\n        description: the amount",
        "              field: value",
        "          - {id: 1, value: 2}\n    expect:\n      rows:\n        - {id: 1, value: 2}",
        "      - name: amount\n        expr: value\n        type: categorical",
        "        expr: value\n        agg: sum",
        "  - name: c\n    columns:\n      - name: value",
        "      - name: total",
    ] {
        assert!(yaml.contains(expected), "{expected}\n---\n{yaml}");
    }
    // c and the snapshot pass it through `select *`: nothing to rewrite
    let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        !names.contains(&"c.sql") && !names.contains(&"snap.sql"),
        "{names:?}"
    );

    // references agree with the rename: every file it edits, d included
    let refs = features::references_at(&ctx, docs.get(&uri), &uri, pos, true);
    let mut ref_files: Vec<String> = refs
        .iter()
        .map(|l| l.uri.rsplit('/').next().unwrap().to_string())
        .collect();
    ref_files.sort();
    ref_files.dedup();
    assert_eq!(
        ref_files,
        [
            "a.sql",
            "b.sql",
            "d.sql",
            "f.sql",
            "no_negative.sql",
            "schema.yml"
        ]
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_rename_refuses_what_it_cannot_follow_and_collisions() {
    let root = project(
        "refuse",
        &[(
            "models/g.sql",
            "{% if var('x', true) %}\nselect amount from {{ ref('c') }}\n{% endif %}\n",
        )],
    );
    let ctx = build_context(root.to_str().unwrap());
    let mut docs = Documents::default();
    let (uri, text, pos) = at(&root, "models/a.sql", "as amount", 3);
    docs.open(uri.clone(), &text, &ctx.dialect);
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let err = features::rename_column_at(&ctx, docs.get(&uri), &uri, pos, "value", &read)
        .unwrap()
        .unwrap_err();
    assert!(
        err.contains("g.sql") && err.contains("control blocks"),
        "{err}"
    );
    // references still show the word in the file rename refuses
    let refs = features::references_at(&ctx, docs.get(&uri), &uri, pos, false);
    assert!(refs.iter().any(|l| l.uri.ends_with("g.sql")), "{refs:?}");
    let _ = std::fs::remove_dir_all(&root);

    let root = project(
        "collide",
        &[(
            "models/h.sql",
            "select c.amount, c.id as deep from {{ ref('c') }} c\n",
        )],
    );
    let ctx = build_context(root.to_str().unwrap());
    let mut docs = Documents::default();
    let (uri, text, pos) = at(&root, "models/a.sql", "as amount", 3);
    docs.open(uri.clone(), &text, &ctx.dialect);
    for (new, why) in [
        ("id", "already a column"),
        ("deep", "already a column of `h`"),
        ("2x", "not a plain column name"),
    ] {
        let err = features::rename_column_at(&ctx, docs.get(&uri), &uri, pos, new, &read)
            .unwrap()
            .unwrap_err();
        assert!(err.contains(why), "{new}: {err}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

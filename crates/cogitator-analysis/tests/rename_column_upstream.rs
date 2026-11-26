//! `dbt.renameColumn`'s two directions over a small project in a temporary
//! directory and over jaffle_shop (read in place: nothing is written): a
//! chain of three models from a source, a CTE pass-through, an aliased
//! use, a `select *` pass-through, a re-alias that stops the walk, the
//! schema and source YAML, and every refusal the upstream climb makes.

use std::path::{Path, PathBuf};

use cogitator_analysis::{build_context, features, Documents};
use cogitator_lsp::{DocumentChange, Position, WorkspaceEdit};
use features::ColumnRename;
use features::ColumnRenameDirection::{Both, Down};

const PROJECT: &[(&str, &str)] = &[
    (
        "dbt_project.yml",
        "name: lineage\nconfig-version: 2\nprofile: lineage\nmodel-paths: [\"models\"]\nseed-paths: [\"seeds\"]\n",
    ),
    (
        "models/sources.yml",
        "version: 2\n\nsources:\n  - name: raw\n    tables:\n      - name: orders\n        columns:\n          - name: amount\n",
    ),
    (
        "models/stg.sql",
        "select id, amount from {{ source('raw', 'orders') }}\n",
    ),
    (
        "models/a.sql",
        "with s as (select * from {{ ref('stg') }})\nselect s.id, s.amount from s\n",
    ),
    ("models/b.sql", "select x.id, x.amount from {{ ref('a') }} x\n"),
    ("models/c.sql", "select * from {{ ref('b') }}\n"),
    ("models/d.sql", "select amount as total, id from {{ ref('c') }}\n"),
    ("models/e.sql", "select total from {{ ref('d') }}\n"),
    (
        "models/z.sql",
        "select id, amount + 1 as plus from {{ ref('stg') }}\n",
    ),
    (
        "models/fee.sql",
        "select id, amount * 0.1 as cut from {{ ref('stg') }}\n",
    ),
    ("models/f.sql", "select cut, id from {{ ref('fee') }}\n"),
    ("seeds/codes.csv", "code,label\na,b\n"),
    ("models/sc.sql", "select code, label from {{ ref('codes') }}\n"),
    ("models/sc2.sql", "select code from {{ ref('sc') }}\n"),
    (
        "models/schema.yml",
        "version: 2\n\nmodels:\n  - name: stg\n    columns:\n      - name: amount\n        description: the amount\n  - name: a\n    columns:\n      - name: amount\n  - name: b\n    columns:\n      - name: amount\n        data_tests:\n          - not_null\n  - name: c\n    columns:\n      - name: amount\n  - name: d\n    columns:\n      - name: total\n",
    ),
];

fn project(tag: &str, extra: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "cogitator-rename-upstream-{tag}-{}",
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
    let i = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} in {rel}"));
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

/// The rename at `needle` of `rel`, `offset` characters in.
fn rename(
    root: &Path,
    rel: &str,
    needle: &str,
    offset: u32,
    new: &str,
    direction: features::ColumnRenameDirection,
) -> Result<ColumnRename, String> {
    let ctx = build_context(root.to_str().unwrap());
    let mut docs = Documents::default();
    let (uri, text, pos) = at(root, rel, needle, offset);
    docs.open(uri.clone(), &text, &ctx.dialect);
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    features::rename_column_directed(&ctx, docs.get(&uri), &uri, pos, new, direction, &read)
        .unwrap_or_else(|| panic!("no column at {needle} in {rel}"))
}

/// (cursor file, needle, offset, direction, origin, files expected as
/// rewritten (name, every substring it must hold), files left alone)
type Case = (
    &'static str,
    &'static str,
    u32,
    features::ColumnRenameDirection,
    &'static str,
    &'static [(&'static str, &'static [&'static str])],
    &'static [&'static str],
);

const CASES: &[Case] = &[
    // a pass-through in the middle, up- and downstream: from the model that
    // reads the source, every model it passes through, a re-alias stops it
    (
        "models/b.sql",
        "x.amount",
        2,
        Both,
        "stg",
        &[
            (
                "stg.sql",
                &["select id, amount as value from {{ source('raw', 'orders') }}"],
            ),
            ("a.sql", &["select s.id, s.value from s"]),
            ("b.sql", &["select x.id, x.value from {{ ref('a') }} x"]),
            ("d.sql", &["select value as total, id from {{ ref('c') }}"]),
            ("z.sql", &["select id, value + 1 as plus"]),
            // fee reads it, and outputs its own `cut`
            ("fee.sql", &["select id, value * 0.1 as cut from"]),
            (
                "schema.yml",
                &[
                    "  - name: stg\n    columns:\n      - name: value\n        description: the amount",
                    "  - name: a\n    columns:\n      - name: value",
                    "  - name: b\n    columns:\n      - name: value\n        data_tests",
                    "  - name: c\n    columns:\n      - name: value",
                    "      - name: total",
                ],
            ),
        ],
        // c passes it through `*`; e reads `total`; the source keeps its
        // column; f reads fee's `cut`
        &["c.sql", "e.sql", "sources.yml", "f.sql"],
    ),
    // the same cursor downstream only: an alias where the name enters b
    (
        "models/b.sql",
        "x.amount",
        2,
        Down,
        "b",
        &[
            ("b.sql", &["select x.id, x.amount as value from {{ ref('a') }} x"]),
            ("d.sql", &["select value as total"]),
            (
                "schema.yml",
                &[
                    "  - name: stg\n    columns:\n      - name: amount",
                    "  - name: b\n    columns:\n      - name: value",
                    "  - name: c\n    columns:\n      - name: value",
                ],
            ),
        ],
        &["stg.sql", "a.sql", "c.sql", "z.sql"],
    ),
    // from the start of the CTE pass-through: the same chain as from b
    (
        "models/a.sql",
        "s.amount",
        2,
        Both,
        "stg",
        &[
            ("stg.sql", &["amount as value"]),
            ("b.sql", &["x.value"]),
            ("z.sql", &["value + 1"]),
        ],
        &["c.sql", "f.sql"],
    ),
    // a model that computes the column defines it: the climb stops there
    (
        "models/f.sql",
        "cut",
        0,
        Both,
        "fee",
        &[
            ("fee.sql", &["select id, amount * 0.1 as value from"]),
            ("f.sql", &["select value, id from {{ ref('fee') }}"]),
        ],
        &["stg.sql", "z.sql"],
    ),
    // a seed's column is its CSV's: the model reading it gets the alias
    (
        "models/sc2.sql",
        "code",
        0,
        Both,
        "sc",
        &[
            ("sc.sql", &["select code as value, label from {{ ref('codes') }}"]),
            ("sc2.sql", &["select value from {{ ref('sc') }}"]),
        ],
        &["codes.csv"],
    ),
];

#[test]
fn a_column_rename_reaches_up_or_down_as_asked() {
    let root = project("cases", &[]);
    for (rel, needle, offset, direction, origin, rewritten, untouched) in CASES {
        let what = format!("{rel} {needle} {direction:?}");
        let done = rename(&root, rel, needle, *offset, "value", *direction)
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(done.origin.model, *origin, "{what}");
        assert_eq!(
            done.models.first().map(String::as_str),
            Some(*origin),
            "{what}"
        );
        let files = apply(&done.edit);
        for (name, holds) in *rewritten {
            let text = files
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, t)| t.as_str())
                .unwrap_or_else(|| panic!("{what}: {name} not edited: {files:#?}"));
            for h in *holds {
                assert!(text.contains(h), "{what}: {name} lacks {h:?}\n---\n{text}");
            }
        }
        for name in *untouched {
            assert!(
                !files.iter().any(|(n, _)| n == name),
                "{what}: {name} edited: {files:#?}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_climb_refuses_what_lineage_cannot_tell() {
    let extra: &[(&str, &str)] = &[
        // two stars may both pass `amount`
        (
            "models/both.sql",
            "select * from {{ ref('a') }} p cross join {{ ref('z') }} q\n",
        ),
        // control blocks hide which column it reads
        (
            "models/jin.sql",
            "{% if var('x', true) %}\nselect id, amount from {{ ref('a') }}\n{% endif %}\n",
        ),
        // a Jinja call generates the column list
        (
            "models/gen.sql",
            "select {{ dbt_utils.star(ref('a')) }} from {{ ref('a') }}\n",
        ),
        (
            "models/both.yml",
            "version: 2\n\nmodels:\n  - name: both\n    columns:\n      - name: amount\n",
        ),
        (
            "models/jin.yml",
            "version: 2\n\nmodels:\n  - name: jin\n    columns:\n      - name: amount\n",
        ),
        (
            "models/gen.yml",
            "version: 2\n\nmodels:\n  - name: gen\n    columns:\n      - name: amount\n",
        ),
    ];
    let root = project("refuse", extra);
    for (model, why) in [
        ("both", "over 2 relations"),
        ("jin", "control blocks"),
        ("gen", "outputs no column named `amount`"),
    ] {
        let yml = format!("models/{model}.yml");
        let err = rename(&root, &yml, "- name: amount", 8, "value", Both);
        let err = err.unwrap_err();
        assert!(
            err.contains("cannot be traced upstream") && err.contains(why),
            "{model}: {err}"
        );
    }
    let _ = std::fs::remove_dir_all(&root);
}

fn jaffle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

#[test]
fn jaffle_customer_id_is_renamed_where_it_is_defined() {
    let root = jaffle();
    let done = rename(
        &root,
        "models/customers.sql",
        "customers.customer_id,",
        10,
        "client_id",
        Both,
    )
    .unwrap();
    assert_eq!(done.origin.model, "stg_customers");
    assert_eq!(done.origin.chain, ["customers", "stg_customers"]);
    let files = apply(&done.edit);
    let get = |name: &str| {
        files
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.as_str())
            .unwrap_or_else(|| panic!("{name}: {files:#?}"))
    };
    assert!(get("stg_customers.sql").contains("id as client_id,"));
    let customers = get("customers.sql");
    assert!(customers.contains("customers.client_id,"), "{customers}");
    assert!(
        customers.contains("on customers.client_id = customer_orders.customer_id"),
        "{customers}"
    );
    // the orders' customer_id is another column
    assert!(!files.iter().any(|(n, _)| n == "stg_orders.sql"));

    // downstream only, the same cursor adds an alias in customers alone
    let down = rename(
        &root,
        "models/customers.sql",
        "customers.customer_id,",
        10,
        "client_id",
        Down,
    )
    .unwrap();
    let files = apply(&down.edit);
    assert!(
        !files.iter().any(|(n, _)| n == "stg_customers.sql"),
        "{files:#?}"
    );
}

#[test]
fn jaffle_a_name_inside_jinja_is_refused() {
    // `{{ full_name('first_name', 'last_name') }}` spells it in a string
    let err = rename(
        &jaffle(),
        "models/customers.sql",
        "customers.first_name",
        10,
        "given_name",
        Both,
    )
    .unwrap_err();
    assert!(
        err.contains("customers.sql") && err.contains("passes the name to Jinja"),
        "{err}"
    );
}

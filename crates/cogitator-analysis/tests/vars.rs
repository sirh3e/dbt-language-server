//! Project vars as a file sees them (03 §4): the root project's, a
//! package's own and its block in the root project, overrides; and the
//! surfaces that read them — inlay hints, definition, hover,
//! references, diagnostics — over a small project written per test.

use std::path::{Path, PathBuf};

use cogitator_analysis::context::{ContextBuilder, VarOverride, VarSource};
use cogitator_analysis::features;
use cogitator_analysis::{DbtContext, Documents};
use cogitator_lsp::{fs_path_to_uri, Position, Range};
use pretty_assertions::assert_eq;

/// A project `shop` with a package `utils`, in a fresh directory.
fn project(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("cogitator-vars-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let write = |rel: &str, text: &str| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    write(
        "dbt_project.yml",
        "name: shop\nprofile: shop\nmodel-paths: [models]\nmacro-paths: [macros]\n\
         vars:\n  since: '2024-01-01'\n  shared: root\n  shop:\n    scoped: 2\n  utils:\n    tuned: 3\n",
    );
    write(
        "models/orders.sql",
        "select * from t where d > '{{ var(\"since\") }}' and n = {{ var('scoped') }}\n\
         and m = {{ var('missing', 7) }} and o = {{ var('gone') }}\n",
    );
    write("models/more.sql", "select {{ var('since') }} as s\n");
    write(
        "dbt_packages/utils/dbt_project.yml",
        "name: utils\nmodel-paths: [models]\nvars:\n  tuned: 0\n  own: 1\n",
    );
    write(
        "dbt_packages/utils/models/pkg.sql",
        "select {{ var('tuned') }}, {{ var('own') }}, {{ var('shared') }}, {{ var('scoped') }}\n",
    );
    root
}

fn build(root: &Path, overrides: Vec<VarOverride>) -> DbtContext {
    let mut builder = ContextBuilder::default();
    builder.set_var_overrides(overrides);
    builder.build(root.to_str().unwrap())
}

fn uri(root: &Path, rel: &str) -> String {
    fs_path_to_uri(&root.join(rel))
}

fn open(ctx: &DbtContext, docs: &mut Documents, uri: &str, root: &Path, rel: &str) {
    let text = std::fs::read_to_string(root.join(rel)).unwrap();
    docs.open(uri.to_string(), &text, &ctx.dialect);
}

fn labels(ctx: &DbtContext, docs: &Documents, uri: &str) -> Vec<String> {
    features::inlay_hints(ctx, docs.get(uri).unwrap(), uri, Range::default(), None)
        .into_iter()
        .map(|h| h.label)
        .collect()
}

/// A cursor one character into the first `needle` of a document.
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

#[test]
fn hints_say_each_value_its_default_or_that_it_is_undefined() {
    let root = project("hints");
    let ctx = build(&root, Vec::new());
    let mut docs = Documents::default();
    let orders = uri(&root, "models/orders.sql");
    open(&ctx, &mut docs, &orders, &root, "models/orders.sql");
    assert_eq!(
        labels(&ctx, &docs, &orders),
        vec!["= '2024-01-01'", "= 2", "= 7 (default)", "= ? undefined"]
    );
    // a package's file sees its own vars, its block in the root project
    // over them, and the root's globals; the root project's own block
    // stays the root project's
    let pkg = uri(&root, "dbt_packages/utils/models/pkg.sql");
    open(
        &ctx,
        &mut docs,
        &pkg,
        &root,
        "dbt_packages/utils/models/pkg.sql",
    );
    assert_eq!(
        labels(&ctx, &docs, &pkg),
        vec!["= 3 (utils)", "= 1 (utils)", "= 'root'", "= ? undefined"]
    );
}

#[test]
fn an_override_wins_and_says_so() {
    let root = project("override");
    let ctx = build(
        &root,
        vec![VarOverride {
            name: "since".into(),
            value: serde_json::json!("2025-06-01"),
            uri: root.join("cogitator.yml"),
            range: Range::default(),
        }],
    );
    assert_eq!(ctx.variable_detail_map["since"].source, VarSource::Override);
    let mut docs = Documents::default();
    let orders = uri(&root, "models/orders.sql");
    open(&ctx, &mut docs, &orders, &root, "models/orders.sql");
    assert_eq!(labels(&ctx, &docs, &orders)[0], "= '2025-06-01' (override)");
    let def = features::definition(
        &ctx,
        docs.get(&orders).unwrap(),
        &orders,
        at(&docs, &orders, "since"),
    );
    assert!(def.uri.ends_with("/cogitator.yml"), "{}", def.uri);
}

#[test]
fn definition_lands_on_the_key_that_decides() {
    let root = project("definition");
    let ctx = build(&root, Vec::new());
    let mut docs = Documents::default();
    let orders = uri(&root, "models/orders.sql");
    open(&ctx, &mut docs, &orders, &root, "models/orders.sql");
    let doc = docs.get(&orders).unwrap();
    // `since:` is line 5 of dbt_project.yml, `scoped:` (under shop:) line 8
    let since = features::definition(&ctx, doc, &orders, at(&docs, &orders, "since"));
    assert!(since.uri.ends_with("/dbt_project.yml"), "{}", since.uri);
    assert_eq!(
        (since.range.start.line, since.range.start.character),
        (5, 2)
    );
    assert_eq!(since.range.end.character, 7, "the key's whole range");
    let scoped = features::definition(&ctx, doc, &orders, at(&docs, &orders, "scoped"));
    assert_eq!(
        (scoped.range.start.line, scoped.range.start.character),
        (8, 4)
    );
    // undefined with a default: the default argument
    let missing = features::definition(&ctx, doc, &orders, at(&docs, &orders, "missing"));
    assert_eq!(missing.uri, orders);
    let seven = at(&docs, &orders, "7)"); // one into the needle
    assert_eq!(missing.range.start.character, seven.character - 1);
    assert_eq!(missing.range.end.character, seven.character);
    // a package's var: the package's own dbt_project.yml
    let pkg = uri(&root, "dbt_packages/utils/models/pkg.sql");
    open(
        &ctx,
        &mut docs,
        &pkg,
        &root,
        "dbt_packages/utils/models/pkg.sql",
    );
    let own = features::definition(&ctx, docs.get(&pkg).unwrap(), &pkg, at(&docs, &pkg, "own"));
    assert!(
        own.uri.ends_with("/dbt_packages/utils/dbt_project.yml"),
        "{}",
        own.uri
    );
    assert_eq!(own.range.start.line, 4);
}

#[test]
fn hover_and_diagnostics_read_the_scope() {
    let root = project("hover");
    let ctx = build(&root, Vec::new());
    let mut docs = Documents::default();
    let orders = uri(&root, "models/orders.sql");
    open(&ctx, &mut docs, &orders, &root, "models/orders.sql");
    let doc = docs.get(&orders).unwrap();
    assert_eq!(
        features::hover(&ctx, doc, at(&docs, &orders, "since")),
        "since: 2024-01-01\n\ndbt_project.yml:6"
    );
    assert_eq!(
        features::hover(&ctx, doc, at(&docs, &orders, "missing")),
        "missing: 7\n\nundefined; the default applies"
    );
    let unknown: Vec<String> = features::dangling_ref_diagnostics(&ctx, doc)
        .into_iter()
        .map(|d| d.message)
        .collect();
    assert_eq!(
        unknown.len(),
        1,
        "only the defaultless undefined var: {unknown:?}"
    );
    assert!(unknown[0].contains("'gone'"), "{unknown:?}");
}

#[test]
fn references_find_every_use_of_the_same_definition() {
    let root = project("references");
    let ctx = build(&root, Vec::new());
    let mut docs = Documents::default();
    let orders = uri(&root, "models/orders.sql");
    open(&ctx, &mut docs, &orders, &root, "models/orders.sql");
    let mut found: Vec<String> = features::references(
        &ctx,
        docs.get(&orders),
        &orders,
        at(&docs, &orders, "since"),
    )
    .into_iter()
    .map(|l| {
        format!(
            "{}:{}",
            l.uri.rsplit('/').next().unwrap(),
            l.range.start.line
        )
    })
    .collect();
    found.sort();
    assert_eq!(
        found,
        vec!["dbt_project.yml:5", "more.sql:0", "orders.sql:0"]
    );

    // from the key in dbt_project.yml: the same uses
    let project_uri = uri(&root, "dbt_project.yml");
    open(&ctx, &mut docs, &project_uri, &root, "dbt_project.yml");
    let from_key = features::references(
        &ctx,
        docs.get(&project_uri),
        &project_uri,
        Position {
            line: 5,
            character: 3,
        },
    );
    assert_eq!(from_key.len(), 3, "{from_key:?}");

    // `tuned` in the package resolves to root's vars.utils, which is
    // not what the root project sees (it has no `tuned`)
    let pkg = uri(&root, "dbt_packages/utils/models/pkg.sql");
    open(
        &ctx,
        &mut docs,
        &pkg,
        &root,
        "dbt_packages/utils/models/pkg.sql",
    );
    let tuned = features::references(&ctx, docs.get(&pkg), &pkg, at(&docs, &pkg, "tuned"));
    let mut tuned: Vec<String> = tuned
        .into_iter()
        .map(|l| {
            format!(
                "{}:{}",
                l.uri.rsplit('/').next().unwrap(),
                l.range.start.line
            )
        })
        .collect();
    tuned.sort();
    assert_eq!(tuned, vec!["dbt_project.yml:10", "pkg.sql:0"]);
}

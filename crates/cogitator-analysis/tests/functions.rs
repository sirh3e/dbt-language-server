//! dbt user-defined functions (`functions/`, `{{ function('…') }}`,
//! dbt 1.11+) over `testdata/udf_shop`: the context, hover, definition,
//! completion, the dangling check, links, references and semantic tokens.

use std::path::{Path, PathBuf};

use cogitator_analysis::{build_context, features, DbtContext, Documents};
use cogitator_lsp::{fs_path_to_uri, Position, SEMANTIC_TOKEN_TYPES};
use pretty_assertions::assert_eq;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/udf_shop")
        .canonicalize()
        .unwrap()
}

fn ctx() -> DbtContext {
    build_context(root().to_str().unwrap())
}

/// The fixture model, opened from disk.
fn open(docs: &mut Documents, ctx: &DbtContext, rel: &str) -> String {
    let path = root().join(rel);
    let uri = fs_path_to_uri(&path);
    let text = std::fs::read_to_string(&path).unwrap();
    docs.open(uri.clone(), &text, &ctx.dialect);
    uri
}

/// The position just inside the quoted name `name` on the line holding it.
fn at(docs: &Documents, uri: &str, name: &str) -> Position {
    let text = &docs.get(uri).unwrap().text;
    let (line, content) = text
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains(&format!("'{name}'")) || l.contains(&format!("\"{name}\"")))
        .unwrap();
    let column = content.find(name).unwrap() + 1;
    Position {
        line: line as u32,
        character: column as u32,
    }
}

fn file(rel: &str) -> String {
    fs_path_to_uri(&root().join(rel))
}

#[test]
fn the_context_holds_every_function_once() {
    let ctx = ctx();
    let mut names: Vec<&str> = ctx.function_detail_map.keys().map(String::as_str).collect();
    names.sort();
    // the overload's body is no function of its own
    assert_eq!(
        names,
        vec!["is_positive_int", "masked_email", "pkg_only", "slugify"]
    );
    let own = &ctx.function_detail_map["is_positive_int"];
    assert_eq!(own.project_name, "udf_shop", "the own project wins");
    assert_eq!(own.language, Some("sql"));
    let slugify = &ctx.function_detail_map["slugify"];
    assert_eq!(slugify.language, Some("python"));
    assert!(slugify.entry.is_none());
    let masked = &ctx.function_detail_map["masked_email"];
    assert!(masked.body.is_none());
    assert_eq!(ctx.function_detail_map["pkg_only"].project_name, "udf_pkg");
}

#[test]
fn hover_shows_the_signature_and_the_description() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = open(&mut docs, &ctx, "models/checked_orders.sql");
    let doc = docs.get(&uri).unwrap();
    let text = features::hover(&ctx, doc, at(&docs, &uri, "is_positive_int"));
    assert!(
        text.starts_with("```sql\nis_positive_int(a_string string) -> boolean\n```"),
        "{text}"
    );
    assert!(
        text.contains("Whether a string holds a positive integer"),
        "{text}"
    );
    assert!(text.contains("- `a_string`: The text to check"), "{text}");
    assert!(
        text.contains("**Returns** True for a positive integer"),
        "{text}"
    );
    assert!(
        text.contains("type: scalar · volatility: deterministic · language: sql · overloads: 1"),
        "{text}"
    );
    // on the call word too
    let word = Position {
        character: at(&docs, &uri, "is_positive_int").character - 8,
        ..at(&docs, &uri, "is_positive_int")
    };
    assert_eq!(features::hover(&ctx, doc, word), text);

    let python = features::hover(&ctx, doc, at(&docs, &uri, "slugify"));
    assert!(python.starts_with("```python\nslugify(…)\n```"), "{python}");
    assert!(python.contains("No `functions:` entry"), "{python}");
}

#[test]
fn definition_goes_to_the_body_else_the_entry() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = open(&mut docs, &ctx, "models/checked_orders.sql");
    let doc = docs.get(&uri).unwrap();
    let body = features::definition(&ctx, doc, &uri, at(&docs, &uri, "is_positive_int"));
    assert_eq!(body.uri, file("functions/is_positive_int.sql"));

    let entry = features::definition(&ctx, doc, &uri, at(&docs, &uri, "masked_email"));
    assert_eq!(entry.uri, file("functions/schema.yml"));
    assert_eq!(
        entry.range.start,
        Position {
            line: 21,
            character: 10
        }
    );
    let package = features::definition(&ctx, doc, &uri, at(&docs, &uri, "pkg_only"));
    assert_eq!(
        package.uri,
        file("dbt_packages/udf_pkg/functions/pkg_only.sql")
    );
}

#[test]
fn completion_offers_the_functions() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = "file:///tmp/new.sql".to_string();
    let line = "select {{ function('is";
    docs.open(uri.clone(), line, &ctx.dialect);
    let items = features::completion(
        &ctx,
        docs.get(&uri).unwrap(),
        Position {
            line: 0,
            character: line.len() as u32,
        },
    );
    let mut labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    labels.sort();
    assert_eq!(
        labels,
        vec!["is_positive_int", "masked_email", "pkg_only", "slugify"]
    );
    let item = items.iter().find(|i| i.label == "is_positive_int").unwrap();
    assert_eq!(item.detail, "is_positive_int(a_string string) -> boolean");
    assert_eq!(item.insert_text, "is_positive_int') }}");
    let documentation =
        features::resolve_completion_documentation(&ctx, item.data.as_ref().unwrap());
    assert!(documentation.is_some_and(|d| d.contains("Whether a string holds a positive integer")));
}

#[test]
fn a_function_nobody_defines_is_flagged() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = open(&mut docs, &ctx, "models/checked_orders.sql");
    let diags = features::dangling_function_diagnostics(&ctx, docs.get(&uri).unwrap());
    assert_eq!(diags.len(), 1, "{diags:?}");
    assert_eq!(diags[0].code, "dangling-function");
    assert!(
        diags[0].message.contains("did you mean 'is_positive_int'"),
        "{}",
        diags[0].message
    );
    // and in the per-document unresolved list
    let unresolved = features::document_unresolved(&ctx, &uri, docs.get(&uri).unwrap());
    assert!(
        unresolved
            .iter()
            .any(|u| u.kind == features::UnresolvedKind::Function && u.name == "is_postive_int"),
        "{unresolved:?}"
    );
    // nothing is judged before a project is read
    let empty =
        features::dangling_function_diagnostics(&DbtContext::default(), docs.get(&uri).unwrap());
    assert!(empty.is_empty());
}

#[test]
fn every_resolved_call_is_a_link() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = open(&mut docs, &ctx, "models/checked_orders.sql");
    let links = features::document_links(&ctx, docs.get(&uri).unwrap());
    let mut tooltips: Vec<String> = links.iter().filter_map(|l| l.tooltip.clone()).collect();
    tooltips.sort();
    assert_eq!(
        tooltips,
        vec![
            "function is_positive_int",
            "function masked_email",
            "function pkg_only",
            "function slugify"
        ]
    );
}

#[test]
fn references_find_every_call_and_the_declarations() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = open(&mut docs, &ctx, "models/checked_orders.sql");
    let doc = docs.get(&uri).unwrap();
    let pos = at(&docs, &uri, "is_positive_int");
    let mut found: Vec<String> = features::references_at(&ctx, Some(doc), &uri, pos, true)
        .into_iter()
        .map(|l| relative(&l.uri))
        .collect();
    found.sort();
    assert_eq!(
        found,
        vec![
            "functions/is_positive_int.sql",
            "functions/schema.yml",
            "models/amounts.sql",
            "models/checked_orders.sql",
        ]
    );
    // the same from the YAML entry's name
    let yaml = open(&mut docs, &ctx, "functions/schema.yml");
    let from_yaml = features::references_at(
        &ctx,
        docs.get(&yaml),
        &yaml,
        Position {
            line: 3,
            character: 12,
        },
        false,
    );
    assert_eq!(from_yaml.len(), 2, "{from_yaml:?}");
}

#[test]
fn the_name_is_a_function_token() {
    let ctx = ctx();
    let mut docs = Documents::default();
    let uri = "file:///tmp/t.sql".to_string();
    docs.open(uri.clone(), "{{ function('slugify') }}(x)", &ctx.dialect);
    let tokens = features::semantic_tokens(docs.get(&uri).unwrap());
    let function = SEMANTIC_TOKEN_TYPES
        .iter()
        .position(|t| *t == "function")
        .unwrap() as u32;
    // (delta line, delta column, length, type, modifiers) per token
    let has = tokens
        .data
        .chunks(5)
        .any(|t| t[2] == "slugify".len() as u32 && t[3] == function);
    assert!(has, "{:?}", tokens.data);
}

fn relative(uri: &str) -> String {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap();
    // the root as a URI spells it: canonical paths are verbatim on Windows
    // (\\?\C:\…), a URI's never is
    let root = cogitator_lsp::uri_to_fs_path(&fs_path_to_uri(&root())).unwrap();
    Path::new(&path)
        .strip_prefix(&root)
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

#[test]
fn a_body_file_is_known_as_one() {
    let ctx = ctx();
    for (rel, want) in [
        ("functions/is_positive_int.sql", true),
        // an overload's body is no function, but it is a body
        ("functions/is_positive_int_numeric.sql", true),
        ("functions/text/slugify.py", true),
        ("dbt_packages/udf_pkg/functions/pkg_only.sql", true),
        ("models/checked_orders.sql", false),
    ] {
        assert_eq!(ctx.is_function_body(&file(rel)), want, "{rel}");
    }
}

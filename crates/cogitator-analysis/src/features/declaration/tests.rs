use std::path::{Path, PathBuf};

use cogitator_lsp::Position;

use super::*;
use crate::Documents;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

fn uri(rel: &str) -> String {
    cogitator_lsp::fs_path_to_uri(&root().join(rel))
}

fn disk(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

fn at(text: &str, needle: &str, offset: u32) -> Position {
    let i = text.find(needle).unwrap_or_else(|| panic!("{needle}"));
    let line = text[..i].matches('\n').count() as u32;
    let col = (i - text[..i].rfind('\n').map_or(0, |j| j + 1)) as u32;
    Position {
        line,
        character: col + offset,
    }
}

/// (file name, line) of a location, with the text it covers.
fn place(loc: &Location) -> (String, u32, String) {
    let path = cogitator_lsp::uri_to_fs_path(&loc.uri).unwrap();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let line = text
        .lines()
        .nth(loc.range.start.line as usize)
        .unwrap_or("");
    let covered = line
        .get(loc.range.start.character as usize..loc.range.end.character as usize)
        .unwrap_or("")
        .to_string();
    (
        loc.uri.rsplit('/').next().unwrap().to_string(),
        loc.range.start.line,
        covered,
    )
}

#[test]
fn declarations_and_definitions_by_what_the_cursor_names() {
    let ctx = crate::build_context(root().to_str().unwrap());
    let mut docs = Documents::default();
    // (file, needle, offset, declaration file + covered text, definition file)
    for (rel, needle, offset, decl, def) in [
        // a ref: declared in YAML, defined by its SQL
        (
            "models/customers.sql",
            "stg_orders'",
            1,
            ("schema.yml", "stg_orders"),
            "stg_orders.sql",
        ),
        // a model file anywhere: its own entry
        (
            "models/customers.sql",
            "with customers",
            0,
            ("schema.yml", "customers"),
            "customers.sql",
        ),
        // a documented column in SQL: declared by its YAML column,
        // definition unchanged (the select item of the file)
        (
            "models/customers.sql",
            "first_name,",
            2,
            ("schema.yml", "first_name"),
            "customers.sql",
        ),
        // a YAML column entry: declared there, defined by the model's SQL
        (
            "models/schema.yml",
            "name: first_name",
            7,
            ("schema.yml", "first_name"),
            "customers.sql",
        ),
        // a source: one place for both
        (
            "models/staging/stg_stripe_payments.sql",
            "payments'",
            1,
            ("schema.yml", ""),
            "schema.yml",
        ),
    ] {
        let text = std::fs::read_to_string(root().join(rel)).unwrap();
        let u = uri(rel);
        docs.open(u.clone(), &text, &ctx.dialect);
        let doc = docs.get(&u).unwrap();
        let pos = at(&text, needle, offset);
        let declared = declaration(&ctx, doc, &u, pos);
        let (file, _, covered) = place(&declared);
        assert_eq!(file, decl.0, "declaration at {needle}");
        if decl.1.is_empty() {
            // one place: exactly what definition answers
            assert_eq!(declared, definition(&ctx, doc, &u, pos), "{needle}");
        } else {
            assert_eq!(covered, decl.1, "declaration at {needle}");
        }
        let definition = match yaml_column_target(&ctx, doc, &u, pos) {
            Some((model, column)) => {
                yaml_column_definition(&ctx, &model, &column, Some((&u, &doc.text)), &disk)
                    .expect("a produced column")
            }
            None => definition(&ctx, doc, &u, pos),
        };
        assert_eq!(place(&definition).0, def, "definition at {needle}");
    }
}

#[test]
fn production_is_the_last_select_item_naming_the_column() {
    let parse = |sql: &str| JinjaSqlParser.parse(sql, &Default::default()).unwrap();
    for (sql, column, want_line) in [
        // the final select's item wins over the CTE's
        (
            "with a as (\nselect id,\nx from t\n)\nselect\na.id,\na.x\nfrom a",
            "id",
            Some(5),
        ),
        // an alias, also last in its list
        ("select\nsum(v) as total\nfrom t", "total", Some(1)),
        // a qualified item before `from`
        ("select\na.x\nfrom a", "x", Some(1)),
        // only mentioned (a filter): its first mention
        ("select *\nfrom t\nwhere\nx > 1", "x", Some(3)),
        // quoted and other case
        ("select\n\"ID\",\nv from t", "id", Some(1)),
        ("select * from t", "x", None),
    ] {
        let got = production_range(&parse(sql), column).map(|r| r.start.line);
        assert_eq!(got, want_line, "{column} in {sql:?}");
    }
}

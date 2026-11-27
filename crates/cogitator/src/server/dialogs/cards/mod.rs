//! Cards: a command's answer written for a person, the way Neovim draws it
//! (`explorer/card.lua`, `deps.lua`, `:DbtQuery`), for a client that only
//! opens files (Zed). Markdown with `file://` links, which Zed opens from a
//! markdown buffer, and a query as a `.sql` file. Pure; an answer whose
//! shape a card does not know is `None`, and the dialogs fall back to the
//! generic [`render`](super::render).

mod deps;
mod model;
mod query;

use serde_json::Value;

/// Which card an answer is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    /// `dbt.modelInfo` (and `dbt.modelGraph`, whose answer is a part of it):
    /// what the model is, its files, versions, neighbours, columns, tests.
    Model,
    /// `dbt.nodeInfo`: a model's card, or a macro's, a generic test's, a
    /// source's…
    Node,
    /// `dbt.modelDependencies`: how the model is built, what it uses and
    /// what uses it, directly and everything.
    Dependencies,
    /// `dbt.relation`: a query of the relation, as a `.sql` file.
    Query,
}

/// A card: its text and the file extension it is written with.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub extension: &'static str,
    pub text: String,
}

/// The rows a query of a relation asks for.
pub const QUERY_LIMIT: u32 = 100;

/// `answer` as a card, or `None` when it is not the shape the card draws.
/// `root` is the project's root as a `file://` URI, to show files by their
/// path in the project.
pub fn render(kind: CardKind, answer: &Value, root: Option<&str>) -> Option<Page> {
    let md = |text: Option<String>| {
        text.map(|text| Page {
            extension: "md",
            text,
        })
    };
    match kind {
        CardKind::Model => md(model::model(answer, root)),
        CardKind::Node => md(model::node(answer, root)),
        CardKind::Dependencies => md(deps::card(answer, root)),
        CardKind::Query => query::query(answer, QUERY_LIMIT).map(|text| Page {
            extension: "sql",
            text,
        }),
    }
}

/// A non-empty string field.
fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// An array field, empty when absent or `null`.
fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

/// Text that stays itself inside a link's label or a table's cell. Only
/// what would break one is escaped, so the raw file still reads well (an
/// underscore inside a word is no emphasis in CommonMark).
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '[' | ']' | '|') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A file by its path in the project (`models/orders.sql`), else by its
/// file system path.
fn path_of(uri: &str, root: Option<&str>) -> String {
    if let Some(rest) = root.and_then(|root| {
        let root = root.trim_end_matches('/');
        uri.strip_prefix(root)?.strip_prefix('/')
    }) {
        return percent_decoded(rest);
    }
    cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string())
}

/// `%20` and its kind back to characters, for a label (the link keeps the
/// URI as it is).
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (bytes[i], bytes.get(i + 1), bytes.get(i + 2)) {
            (b'%', Some(&h), Some(&l)) if hex(h).is_some() && hex(l).is_some() => {
                out.push((hex(h).unwrap_or(0) * 16 + hex(l).unwrap_or(0)) as u8);
                i += 3;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A markdown link to `uri`, or the label alone without one.
fn link(label: &str, uri: Option<&str>) -> String {
    match uri.filter(|u| !u.is_empty()) {
        // `<…>` only where the URI would end the link early
        Some(uri) if uri.contains(['(', ')', ' ']) => format!("[{}](<{uri}>)", escape(label)),
        Some(uri) => format!("[{}]({uri})", escape(label)),
        None => escape(label),
    }
}

/// A link to a file, labelled by its path.
fn file_link(uri: &str, root: Option<&str>) -> String {
    link(&path_of(uri, root), Some(uri))
}

/// A section heading with how many it holds.
fn heading(out: &mut String, title: &str, count: usize) {
    out.push_str(&format!("\n## {title} ({count})\n\n"));
}

/// `None` → nothing, a string → itself.
fn description(out: &mut String, value: &Value) {
    if let Some(d) = text(value, "description") {
        out.push('\n');
        out.push_str(d.trim());
        out.push('\n');
    }
}

/// `#tag` codes on one line.
fn tags(out: &mut String, tags: &[Value]) {
    let tags: Vec<String> = tags
        .iter()
        .filter_map(Value::as_str)
        .map(|t| format!("`#{t}`"))
        .collect();
    if !tags.is_empty() {
        out.push('\n');
        out.push_str(&tags.join(" "));
        out.push('\n');
    }
}

/// A column's type as shown: the dialect's spelling, else what is known
/// of it; `?` after a nullable one (Neovim's `paint.type_text`).
fn type_text(column: &Value) -> String {
    let spelled = text(column, "dataType")
        .or_else(|| text(column, "description"))
        .unwrap_or("unknown");
    let nullable = column.get("nullable").and_then(Value::as_bool) == Some(true);
    format!("{spelled}{}", if nullable { "?" } else { "" })
}

/// What a column's type rests on, in a word.
fn basis(column: &Value) -> &str {
    match text(column, "basis") {
        Some("proven") => "proven",
        Some("declared") => "declared, not enforced",
        _ => "unknown",
    }
}

/// Typed columns as a table.
fn columns_table(out: &mut String, columns: &[Value]) {
    out.push_str("| column | type | basis |\n|---|---|---|\n");
    for c in columns {
        let name = text(c, "name").unwrap_or("?");
        out.push_str(&format!(
            "| `{}` | `{}` | {} |\n",
            name.replace('|', "\\|"),
            type_text(c).replace('|', "\\|"),
            basis(c)
        ));
    }
}

/// What is said where types are not inferred.
const NOT_INFERRED: &str =
    "column types not inferred yet — they come with the project's schema job";

/// A card's text, ending in one newline.
fn finished(mut out: String) -> String {
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn a_file_is_shown_by_its_path_in_the_project() {
        let cases = [
            (
                "file:///p/models/orders.sql",
                Some("file:///p"),
                "models/orders.sql",
            ),
            (
                "file:///p/models/orders.sql",
                Some("file:///p/"),
                "models/orders.sql",
            ),
            (
                "file:///p/models/my%20file.sql",
                Some("file:///p"),
                "models/my file.sql",
            ),
        ];
        for (uri, root, shown) in cases {
            assert_eq!(path_of(uri, root), shown, "{uri}");
        }
        // outside the project: its file system path, never a stripped prefix
        let elsewhere = path_of("file:///p2/a.sql", Some("file:///p"));
        assert!(
            elsewhere.ends_with("a.sql") && elsewhere.contains("p2"),
            "{elsewhere}"
        );
    }

    #[test]
    fn a_type_is_its_spelling_else_what_is_known() {
        let cases = [
            (json!({"dataType": "integer"}), "integer"),
            (json!({"dataType": "integer", "nullable": true}), "integer?"),
            (json!({"description": "a number"}), "a number"),
            (json!({"dataType": "", "description": ""}), "unknown"),
        ];
        for (column, shown) in cases {
            assert_eq!(type_text(&column), shown, "{column}");
        }
    }

    #[test]
    fn a_label_keeps_its_markdown_characters() {
        assert_eq!(
            link("a_b [x]", Some("file:///a")),
            r"[a_b \[x\]](file:///a)"
        );
        assert_eq!(
            link("a", Some("file:///a (1).sql")),
            "[a](<file:///a (1).sql>)"
        );
        assert_eq!(link("plain", None), "plain");
        assert_eq!(link("plain", Some("")), "plain");
    }

    #[test]
    fn an_unknown_shape_falls_back() {
        for kind in [
            CardKind::Model,
            CardKind::Node,
            CardKind::Dependencies,
            CardKind::Query,
        ] {
            assert_eq!(render(kind, &json!({"other": 1}), None), None, "{kind:?}");
        }
    }
}

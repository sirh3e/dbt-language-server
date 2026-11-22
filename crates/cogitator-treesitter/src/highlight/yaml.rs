//! YAML highlighting composed by the server (spec §3–§5): tree-sitter-yaml
//! has no injections query, so the Jinja inside values, the SQL of the keys
//! that hold SQL and the dbt-aware captures are worked out here, over the
//! YAML tree, and handed back as byte spans for the merge of §3.3.

use super::{grammar_spans, ByteSpan, HighlightLanguage, Layer};

/// Which YAML file of a project the text is: decides the dbt-aware
/// captures (§5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YamlFile {
    /// A property file: models, sources, tests, macros, exposures…
    Properties,
    /// `dbt_project.yml`
    Project,
    /// `profiles.yml`
    Profiles,
    /// `packages.yml` or `dependencies.yml`
    Packages,
    /// `selectors.yml`
    Selectors,
}

impl YamlFile {
    /// The kind of a YAML file, by its name.
    pub fn of_path(path: &str) -> Self {
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
        match name {
            "dbt_project.yml" | "dbt_project.yaml" => Self::Project,
            "profiles.yml" | "profiles.yaml" => Self::Profiles,
            "packages.yml" | "packages.yaml" | "dependencies.yml" | "dependencies.yaml" => {
                Self::Packages
            }
            "selectors.yml" | "selectors.yaml" => Self::Selectors,
            _ => Self::Properties,
        }
    }
}

/// A reference a YAML value makes, for the resolver (§5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    /// `ref('model')` (a package, if named, is dropped)
    Model(String),
    /// `source('source', 'table')`
    Source(String, String),
    /// `doc('name')`
    Doc(String),
}

fn no_config_key(_: &str) -> bool {
    false
}

fn everything_resolves(_: &Reference) -> bool {
    true
}

/// What the project knows, for the dbt-aware captures.
pub struct YamlContext<'a> {
    pub file: YamlFile,
    /// Whether a key is one of dbt's config keys (from dbt's JSON schemas).
    pub config_key: &'a (dyn Fn(&str) -> bool + Sync),
    /// Whether a reference resolves in the project; one that does not gets
    /// its capture with `.unresolved`.
    pub resolves: &'a (dyn Fn(&Reference) -> bool + Sync),
}

impl Default for YamlContext<'_> {
    fn default() -> Self {
        Self {
            file: YamlFile::Properties,
            config_key: &no_config_key,
            resolves: &everything_resolves,
        }
    }
}

/// Every dbt-aware capture (without `.unresolved`), for `capture_names`.
pub const DBT_CAPTURES: &[&str] = &[
    "function.dbt.macro",
    "function.dbt.selector",
    "function.dbt.test",
    "keyword.dbt.config",
    "module.dbt.source",
    "property.dbt.column",
    "string.special.dbt.doc",
    "type.dbt.analysis",
    "type.dbt.exposure",
    "type.dbt.metric",
    "type.dbt.model",
    "type.dbt.profile",
    "type.dbt.reference",
    "type.dbt.saved_query",
    "type.dbt.seed",
    "type.dbt.semantic_model",
    "type.dbt.snapshot",
    "type.dbt.source_table",
    "variable.parameter.dbt",
];

/// A step of a key path: a mapping key, or a sequence item (`*`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seg {
    Key(String),
    Item,
}

impl Seg {
    fn is(&self, name: &str) -> bool {
        match self {
            Seg::Key(k) => k == name || name == "*",
            Seg::Item => name == "*",
        }
    }

    fn key(&self) -> Option<&str> {
        match self {
            Seg::Key(k) => Some(k),
            Seg::Item => None,
        }
    }
}

/// A scalar of the document: a key or a value, where it is, and what is
/// around it.
#[derive(Debug, Clone)]
struct Scalar {
    /// A key's path is the path of its mapping; a value's ends with its key
    /// (or `*` for a sequence item).
    path: Vec<Seg>,
    key: bool,
    kind: &'static str,
    start: usize,
    end: usize,
    /// A value of a mapping that also says `format: sql` (unit-test rows).
    format_sql: bool,
}

const SCALARS: &[&str] = &[
    "plain_scalar",
    "double_quote_scalar",
    "single_quote_scalar",
    "block_scalar",
];

fn kind_of(node: tree_sitter::Node) -> Option<&'static str> {
    SCALARS.iter().copied().find(|k| *k == node.kind())
}

/// The innermost scalar of a key or value node.
fn scalar_in(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    if kind_of(node).is_some() {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = scalar_in(child) {
            return Some(found);
        }
    }
    None
}

/// A stretch of scalar content and where it lies in the document.
#[derive(Debug, Clone, Copy)]
struct Piece {
    content: usize,
    doc: usize,
    len: usize,
}

/// A scalar's content (quotes and a block's header and indentation
/// removed) and the map back to the document.
fn content(src: &str, kind: &str, start: usize, end: usize) -> (String, Vec<Piece>) {
    let text = &src[start..end];
    match kind {
        "double_quote_scalar" | "single_quote_scalar" if text.len() >= 2 => (
            text[1..text.len() - 1].to_string(),
            vec![Piece {
                content: 0,
                doc: start + 1,
                len: text.len() - 2,
            }],
        ),
        "block_scalar" => {
            // the header line (`|`, `>-`, `|2`…), then indented lines
            let Some(newline) = text.find('\n') else {
                return (String::new(), Vec::new());
            };
            let body = &text[newline + 1..];
            let indent = body
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.len() - l.trim_start_matches(' ').len())
                .min()
                .unwrap_or(0);
            let mut out = String::new();
            let mut pieces = Vec::new();
            let mut at = start + newline + 1;
            for line in body.split_inclusive('\n') {
                let skip = indent.min(line.len() - line.trim_start_matches(' ').len());
                let kept = &line[skip..];
                pieces.push(Piece {
                    content: out.len(),
                    doc: at + skip,
                    len: kept.len(),
                });
                out.push_str(kept);
                at += line.len();
            }
            (out, pieces)
        }
        _ => (
            text.to_string(),
            vec![Piece {
                content: 0,
                doc: start,
                len: text.len(),
            }],
        ),
    }
}

/// Content bytes `start..end` as document byte ranges.
fn to_doc(pieces: &[Piece], start: usize, end: usize) -> Vec<(usize, usize)> {
    pieces
        .iter()
        .filter_map(|p| {
            let s = start.max(p.content);
            let e = end.min(p.content + p.len);
            (s < e).then(|| (p.doc + (s - p.content), p.doc + (e - p.content)))
        })
        .collect()
}

/// The text of a key node, without quotes.
fn key_text(src: &str, node: tree_sitter::Node) -> String {
    scalar_in(node)
        .map(|s| {
            let kind = kind_of(s).unwrap_or("plain_scalar");
            content(src, kind, s.start_byte(), s.end_byte()).0
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Walk the YAML tree, recording every scalar with its path.
fn walk(
    src: &str,
    node: tree_sitter::Node,
    path: &mut Vec<Seg>,
    format_sql: bool,
    out: &mut Vec<Scalar>,
) {
    if let Some(kind) = kind_of(node) {
        out.push(Scalar {
            path: path.clone(),
            key: false,
            kind,
            start: node.start_byte(),
            end: node.end_byte(),
            format_sql,
        });
        return;
    }
    let mut cursor = node.walk();
    match node.kind() {
        "block_mapping" | "flow_mapping" => {
            let pairs: Vec<_> = node
                .named_children(&mut cursor)
                .filter(|c| matches!(c.kind(), "block_mapping_pair" | "flow_pair"))
                .collect();
            let says_sql = pairs.iter().any(|p| {
                let key = p.child_by_field_name("key").map(|k| key_text(src, k));
                let value = p.child_by_field_name("value").map(|v| key_text(src, v));
                key.as_deref() == Some("format") && value.as_deref() == Some("sql")
            });
            for pair in pairs {
                let Some(key) = pair.child_by_field_name("key") else {
                    continue;
                };
                let name = key_text(src, key);
                if let Some(scalar) = scalar_in(key) {
                    out.push(Scalar {
                        path: path.clone(),
                        key: true,
                        kind: kind_of(scalar).unwrap_or("plain_scalar"),
                        start: scalar.start_byte(),
                        end: scalar.end_byte(),
                        format_sql: false,
                    });
                }
                if let Some(value) = pair.child_by_field_name("value") {
                    path.push(Seg::Key(name));
                    walk(src, value, path, says_sql, out);
                    path.pop();
                }
            }
        }
        "block_sequence" | "flow_sequence" => {
            for item in node.named_children(&mut cursor) {
                if item.kind() == "comment" {
                    continue;
                }
                path.push(Seg::Item);
                walk(src, item, path, false, out);
                path.pop();
            }
        }
        _ => {
            for child in node.named_children(&mut cursor) {
                walk(src, child, path, format_sql, out);
            }
        }
    }
}

/// `path` is exactly `pattern` (`*` any one step).
fn is(path: &[Seg], pattern: &[&str]) -> bool {
    path.len() == pattern.len() && path.iter().zip(pattern).all(|(s, p)| s.is(p))
}

/// `path` ends with `pattern`.
fn ends(path: &[Seg], pattern: &[&str]) -> bool {
    path.len() >= pattern.len() && is(&path[path.len() - pattern.len()..], pattern)
}

fn first(path: &[Seg]) -> Option<&str> {
    path.first().and_then(Seg::key)
}

fn last(path: &[Seg]) -> Option<&str> {
    path.last().and_then(Seg::key)
}

/// Is the scalar SQL, and is it a statement or an expression (§4)?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqlKind {
    Statement,
    Expression,
}

fn sql_kind(s: &Scalar) -> Option<SqlKind> {
    let p = &s.path;
    let under_tests = p
        .iter()
        .any(|seg| seg.key().is_some_and(|k| k == "tests" || k == "data_tests"));
    if last(p) == Some("expression")
        && (under_tests || p.iter().any(|seg| seg.is("expression_is_true")))
    {
        return Some(SqlKind::Expression);
    }
    if ends(p, &["config", "where"]) || ends(p, &["config", "where", "*"]) {
        return Some(SqlKind::Expression);
    }
    if ends(p, &["config", "sql_header"]) {
        return Some(SqlKind::Statement);
    }
    if s.format_sql && first(p) == Some("unit_tests") && last(p) == Some("rows") {
        return Some(SqlKind::Statement);
    }
    match first(p) {
        Some("semantic_models") if last(p) == Some("expr") => Some(SqlKind::Expression),
        Some("metrics") if last(p) == Some("filter") || ends(p, &["filter", "*"]) => {
            Some(SqlKind::Expression)
        }
        Some("saved_queries") if last(p) == Some("where") || ends(p, &["where", "*"]) => {
            Some(SqlKind::Expression)
        }
        _ => None,
    }
}

const TEST_LISTS: [&str; 2] = ["tests", "data_tests"];

/// The dbt-aware capture of a scalar, by where it is (§5).
fn dbt_capture(
    s: &Scalar,
    text: &str,
    file: YamlFile,
    config_key: &dyn Fn(&str) -> bool,
) -> Option<String> {
    let p = &s.path;
    let c = |name: &str| Some(name.to_string());
    match file {
        YamlFile::Properties => {
            if s.key {
                // a test given as a mapping: `- relationships: {...}`
                if TEST_LISTS.iter().any(|l| ends(p, &[l, "*"])) {
                    return c("function.dbt.test");
                }
                // a test's arguments: `- relationships: {to:, field:}`, and
                // the newer `arguments:` block
                let in_test = TEST_LISTS
                    .iter()
                    .any(|l| ends(p, &[l, "*", "*"]) || ends(p, &[l, "*", "*", "arguments"]));
                if in_test && text != "config" && text != "arguments" {
                    return c("variable.parameter.dbt");
                }
                if last(p) == Some("config") {
                    return c("keyword.dbt.config");
                }
                let entry = p.len() == 2
                    && matches!(p[1], Seg::Item)
                    && matches!(
                        first(p),
                        Some(
                            "models"
                                | "seeds"
                                | "snapshots"
                                | "sources"
                                | "analyses"
                                | "exposures"
                                | "semantic_models"
                        )
                    );
                if entry && config_key(text) {
                    return c("keyword.dbt.config");
                }
                return None;
            }
            for (list, capture) in [
                ("models", "type.dbt.model"),
                ("snapshots", "type.dbt.snapshot"),
                ("seeds", "type.dbt.seed"),
                ("analyses", "type.dbt.analysis"),
                ("exposures", "type.dbt.exposure"),
                ("metrics", "type.dbt.metric"),
                ("semantic_models", "type.dbt.semantic_model"),
                ("saved_queries", "type.dbt.saved_query"),
                ("macros", "function.dbt.macro"),
                ("sources", "module.dbt.source"),
            ] {
                if is(p, &[list, "*", "name"]) {
                    return c(capture);
                }
            }
            if is(p, &["sources", "*", "tables", "*", "name"]) {
                return c("type.dbt.source_table");
            }
            if ends(p, &["columns", "*", "name"]) {
                return c("property.dbt.column");
            }
            if is(p, &["macros", "*", "arguments", "*", "name"]) {
                return c("variable.parameter.dbt");
            }
            if TEST_LISTS.iter().any(|l| ends(p, &[l, "*"])) {
                return c("function.dbt.test");
            }
            None
        }
        YamlFile::Project => {
            if s.key {
                if p.is_empty() || text.starts_with('+') {
                    return c("keyword.dbt.config");
                }
                if config_key(text) {
                    return c("keyword.dbt.config");
                }
                return None;
            }
            is(p, &["profile"]).then(|| "type.dbt.profile".to_string())
        }
        YamlFile::Profiles => {
            if s.key {
                if (p.is_empty() && text != "config") || is(p, &["*", "outputs"]) {
                    return c("type.dbt.profile");
                }
                return None;
            }
            is(p, &["*", "target"]).then(|| "type.dbt.profile".to_string())
        }
        YamlFile::Selectors => (!s.key && is(p, &["selectors", "*", "name"]))
            .then(|| "function.dbt.selector".to_string()),
        YamlFile::Packages => None,
    }
}

/// A `ref(…)`, `source(…)` or `doc(…)` call in a text: its byte range and
/// its string arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Call {
    name: &'static str,
    start: usize,
    end: usize,
    /// The string arguments with their content ranges (inside the quotes).
    args: Vec<(String, usize, usize)>,
}

fn calls(text: &str) -> Vec<Call> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    for name in ["ref", "source", "doc"] {
        let mut from = 0;
        while let Some(i) = text[from..].find(name).map(|i| i + from) {
            from = i + name.len();
            let before_ok = i == 0 || {
                let b = bytes[i - 1];
                !(b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
            };
            let mut at = i + name.len();
            while at < bytes.len() && bytes[at] == b' ' {
                at += 1;
            }
            if !before_ok || at >= bytes.len() || bytes[at] != b'(' {
                continue;
            }
            at += 1;
            let mut args = Vec::new();
            let mut closed = None;
            while at < bytes.len() {
                match bytes[at] {
                    b')' => {
                        closed = Some(at + 1);
                        break;
                    }
                    q @ (b'\'' | b'"') => {
                        let open = at + 1;
                        let Some(len) = text[open..].find(q as char) else {
                            break;
                        };
                        args.push((text[open..open + len].to_string(), open, open + len));
                        at = open + len + 1;
                    }
                    b'\n' => break,
                    _ => at += 1,
                }
            }
            if let Some(end) = closed {
                if !args.is_empty() {
                    out.push(Call {
                        name,
                        start: i,
                        end,
                        args,
                    });
                }
            }
        }
    }
    out.sort_by_key(|c| c.start);
    out
}

/// Every span of a YAML document: the base layer, the Jinja and SQL inside
/// values, the dbt-aware captures, the references.
pub(super) fn spans(src: &str, ctx: &YamlContext) -> Option<Vec<ByteSpan>> {
    let mut out = grammar_spans(src, HighlightLanguage::Yaml)?;
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_yaml::LANGUAGE.into())
        .ok()?;
    let tree = parser.parse(src, None)?;
    let mut scalars = Vec::new();
    walk(src, tree.root_node(), &mut Vec::new(), false, &mut scalars);

    let properties = ctx.file == YamlFile::Properties;
    for s in &scalars {
        let (text, pieces) = content(src, s.kind, s.start, s.end);
        let trimmed = text.trim();

        // the dbt-aware capture of the whole scalar (§5)
        if let Some(capture) = dbt_capture(s, trimmed, ctx.file, ctx.config_key) {
            out.push(ByteSpan {
                start: s.start,
                end: s.end,
                capture,
                layer: Layer::Dbt,
            });
        }
        if s.key {
            continue;
        }

        // the SQL of §4 and the Jinja of §3.2
        let sql = if properties { sql_kind(s) } else { None };
        let jinja = text.contains("{{") || text.contains("{%") || text.contains("{#");
        if sql.is_some() || jinja {
            let prefix = if sql == Some(SqlKind::Expression) {
                "select "
            } else {
                ""
            };
            let full = format!("{prefix}{text}");
            if let Some(found) = grammar_spans(&full, HighlightLanguage::Dbt) {
                for span in found {
                    if span.end <= prefix.len() || (span.layer == Layer::Sql && sql.is_none()) {
                        continue;
                    }
                    let start = span.start.max(prefix.len()) - prefix.len();
                    let end = span.end - prefix.len();
                    for (ds, de) in to_doc(&pieces, start, end) {
                        out.push(ByteSpan {
                            start: ds,
                            end: de,
                            capture: span.capture.clone(),
                            layer: span.layer,
                        });
                    }
                }
            }
        }

        // references and doc blocks (§5)
        for call in calls(&text) {
            let (reference, at): (Reference, Vec<(usize, usize)>) = match call.name {
                "ref" => {
                    let model = call.args.last().map(|a| a.0.clone()).unwrap_or_default();
                    (
                        Reference::Model(model),
                        to_doc(&pieces, call.start, call.end),
                    )
                }
                "source" if call.args.len() >= 2 => (
                    Reference::Source(call.args[0].0.clone(), call.args[1].0.clone()),
                    to_doc(&pieces, call.start, call.end),
                ),
                "doc" => {
                    let (name, a, b) = call.args[0].clone();
                    (Reference::Doc(name), to_doc(&pieces, a, b))
                }
                _ => continue,
            };
            let base = if call.name == "doc" {
                "string.special.dbt.doc"
            } else {
                "type.dbt.reference"
            };
            let capture = if (ctx.resolves)(&reference) {
                base.to_string()
            } else {
                format!("{base}.unresolved")
            };
            for (start, end) in at {
                out.push(ByteSpan {
                    start,
                    end,
                    capture: capture.clone(),
                    layer: Layer::Dbt,
                });
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn a_block_scalar_maps_back_through_its_indentation() {
        let src = "k: |\n    select 1\n      from t\n";
        let start = src.find('|').unwrap();
        let (text, pieces) = content(src, "block_scalar", start, src.len());
        assert_eq!(text, "select 1\n  from t\n");
        let from = text.find("from").unwrap();
        assert_eq!(
            to_doc(&pieces, from, from + 4),
            vec![(src.find("from").unwrap(), src.find("from").unwrap() + 4)]
        );
    }

    #[test]
    fn quoted_content_drops_the_quotes() {
        let src = "k: \"{{ var('x') }}\"";
        let start = src.find('"').unwrap();
        let (text, pieces) = content(src, "double_quote_scalar", start, src.len());
        assert_eq!(text, "{{ var('x') }}");
        assert_eq!(to_doc(&pieces, 0, 2), vec![(start + 1, start + 3)]);
    }

    #[test]
    fn calls_are_found_with_their_arguments() {
        let found = calls(
            "to: ref('customers') and {{ source('raw', 'orders') }} {{ doc(\"d\") }} myref('x')",
        );
        let names: Vec<_> = found
            .iter()
            .map(|c| {
                (
                    c.name,
                    c.args.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(),
                )
            })
            .collect();
        assert_eq!(
            names,
            vec![
                ("ref", vec!["customers"]),
                ("source", vec!["raw", "orders"]),
                ("doc", vec!["d"])
            ]
        );
    }

    #[test]
    fn yaml_files_are_known_by_name() {
        assert_eq!(YamlFile::of_path("/p/dbt_project.yml"), YamlFile::Project);
        assert_eq!(
            YamlFile::of_path("/p/models/schema.yml"),
            YamlFile::Properties
        );
        assert_eq!(
            YamlFile::of_path("C:\\p\\selectors.yml"),
            YamlFile::Selectors
        );
        assert_eq!(YamlFile::of_path("profiles.yml"), YamlFile::Profiles);
        assert_eq!(YamlFile::of_path("dependencies.yml"), YamlFile::Packages);
    }
}

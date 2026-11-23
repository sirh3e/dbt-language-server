//! dbt's node-selection syntax, as `--select` and `--exclude` read it
//! (dbt-core's graph/selector_spec.py): whitespace separates a union of
//! groups, `,` intersects the criteria of a group, and each criterion is
//!
//!   [@][n+][method[.arg…]:]value[+n]
//!
//! `@` takes the node, its descendants and their ancestors; `n+` before
//! the value its ancestors (n levels, or all), `+n` after it its
//! descendants. Without a method, dbt guesses one from the value: a path
//! when it holds a `/`, a file when it ends in .sql, .py or .csv, else an
//! fqn. Errors point at the byte range of what is wrong.

use serde::Serialize;

/// How far a graph operator reaches: every level, or at most `n`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    All,
    Levels(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criterion {
    pub method: String,
    /// `config.materialized` → `["materialized"]`.
    pub args: Vec<String>,
    pub value: String,
    pub parents: Option<Depth>,
    pub children: Option<Depth>,
    pub childrens_parents: bool,
    /// Byte range of the criterion in the text it came from.
    pub start: usize,
    pub end: usize,
}

/// A union of intersections: `a,b c` is (a ∩ b) ∪ c.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selection {
    pub groups: Vec<Vec<Criterion>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelectionError {
    pub message: String,
    pub start: usize,
    pub end: usize,
}

/// The methods dbt has. The evaluator answers most; `REFUSED` need a
/// previous run's artifacts and are refused rather than answered wrong.
pub const METHODS: &[&str] = &[
    "tag",
    "path",
    "file",
    "fqn",
    "package",
    "resource_type",
    "config",
    "access",
    "group",
    "source",
    "exposure",
    "metric",
    "semantic_model",
    "test_type",
    "test_name",
    "selector",
    "unit_test",
    "version",
];

pub const REFUSED: &[&str] = &["state", "result", "source_status", "saved_query"];

/// Parse a selection (`--select`'s or `--exclude`'s text).
pub fn parse(text: &str) -> Result<Selection, SelectionError> {
    let mut groups = Vec::new();
    for (start, word) in words(text) {
        let mut group = Vec::new();
        let mut at = start;
        for part in word.split(',') {
            if part.is_empty() {
                return Err(error(
                    "an empty criterion: `,` joins two criteria",
                    at,
                    at + 1,
                ));
            }
            group.push(criterion(part, at)?);
            at += part.len() + 1;
        }
        groups.push(group);
    }
    Ok(Selection { groups })
}

/// The selection and the `--exclude` written inline after it
/// (`tag:a --exclude resource_type:test`), as the finder takes them.
pub fn split_exclude(text: &str) -> (&str, Option<&str>) {
    match text.find("--exclude") {
        Some(i) => (
            text[..i].trim_end(),
            Some(text[i + "--exclude".len()..].trim()),
        ),
        None => (text, None),
    }
}

fn words(text: &str) -> impl Iterator<Item = (usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (c.is_whitespace(), start) {
            (true, Some(s)) => {
                out.push((s, &text[s..i]));
                start = None;
            }
            (false, None) => start = Some(i),
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, &text[s..]));
    }
    out.into_iter()
}

fn error(message: impl Into<String>, start: usize, end: usize) -> SelectionError {
    SelectionError {
        message: message.into(),
        start,
        end,
    }
}

fn criterion(raw: &str, offset: usize) -> Result<Criterion, SelectionError> {
    let end = offset + raw.len();
    let mut rest = raw;
    let childrens_parents = rest.starts_with('@');
    if childrens_parents {
        rest = &rest[1..];
    }
    // n+ before the value: digits then `+`
    let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
    let mut parents = None;
    if rest[digits..].starts_with('+') {
        parents = Some(depth(&rest[..digits]));
        rest = &rest[digits + 1..];
    }
    if childrens_parents && parents.is_some() {
        return Err(error(
            "`@` and `+` before a value do not go together: `@` already takes its ancestors",
            offset,
            end,
        ));
    }
    // +n after the value
    let mut children = None;
    if let Some(plus) = rest.rfind('+') {
        let tail = &rest[plus + 1..];
        if tail.chars().all(|c| c.is_ascii_digit()) {
            children = Some(depth(tail));
            rest = &rest[..plus];
        }
    }
    let (method, args, value) = match rest.split_once(':') {
        Some((head, value)) if is_method_name(head) => {
            let mut parts = head.split('.');
            let method = parts.next().unwrap_or_default().to_string();
            (
                method,
                parts.map(str::to_string).collect(),
                value.to_string(),
            )
        }
        _ => (
            default_method(rest).to_string(),
            Vec::new(),
            rest.to_string(),
        ),
    };
    if value.is_empty() {
        return Err(error(
            format!(
                "`{method}:` needs a value, e.g. `{method}:{}`",
                example(&method)
            ),
            offset,
            end,
        ));
    }
    if REFUSED.contains(&method.as_str()) {
        return Err(error(
            format!("`{method}:` compares against a previous dbt run's artifacts, which the editor does not keep"),
            offset,
            end,
        ));
    }
    if !METHODS.contains(&method.as_str()) {
        return Err(error(
            format!("dbt has no selection method `{method}:`"),
            offset,
            offset + method.len() + 1,
        ));
    }
    if method == "config" && args.is_empty() {
        return Err(error(
            "`config:` names a config key: `config.materialized:incremental`",
            offset,
            end,
        ));
    }
    Ok(Criterion {
        method,
        args,
        value,
        parents,
        children,
        childrens_parents,
        start: offset,
        end,
    })
}

fn depth(digits: &str) -> Depth {
    digits.parse().map_or(Depth::All, Depth::Levels)
}

fn is_method_name(head: &str) -> bool {
    !head.is_empty()
        && head
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

/// dbt's `SelectionCriteria.default_method`.
fn default_method(value: &str) -> &'static str {
    if value.contains('/') || value.contains('\\') {
        "path"
    } else if [".sql", ".py", ".csv"]
        .iter()
        .any(|ext| value.to_lowercase().ends_with(ext))
    {
        "file"
    } else {
        "fqn"
    }
}

fn example(method: &str) -> &'static str {
    match method {
        "tag" => "nightly",
        "path" => "models/staging",
        "resource_type" => "model",
        "config" => "incremental",
        "test_type" => "generic",
        "source" => "raw.orders",
        _ => "name",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    fn one(text: &str) -> Criterion {
        let sel = parse(text).expect("parses");
        assert_eq!(sel.groups.len(), 1);
        assert_eq!(sel.groups[0].len(), 1);
        sel.groups[0][0].clone()
    }

    #[rstest]
    #[case("dim_items", "fqn", "dim_items", None, None, false)]
    #[case("+dim_items", "fqn", "dim_items", Some(Depth::All), None, false)]
    #[case("dim_items+", "fqn", "dim_items", None, Some(Depth::All), false)]
    #[case(
        "2+dim_items+1",
        "fqn",
        "dim_items",
        Some(Depth::Levels(2)),
        Some(Depth::Levels(1)),
        false
    )]
    #[case("@dim_items", "fqn", "dim_items", None, None, true)]
    #[case("tag:nightly", "tag", "nightly", None, None, false)]
    #[case("tag:nightly+", "tag", "nightly", None, Some(Depth::All), false)]
    #[case("models/dim", "path", "models/dim", None, None, false)]
    #[case("dim_items.sql", "file", "dim_items.sql", None, None, false)]
    #[case("stg_*", "fqn", "stg_*", None, None, false)]
    #[case(
        "source:landing.items+",
        "source",
        "landing.items",
        None,
        Some(Depth::All),
        false
    )]
    fn criteria(
        #[case] text: &str,
        #[case] method: &str,
        #[case] value: &str,
        #[case] parents: Option<Depth>,
        #[case] children: Option<Depth>,
        #[case] at: bool,
    ) {
        let c = one(text);
        assert_eq!(
            (
                c.method.as_str(),
                c.value.as_str(),
                c.parents,
                c.children,
                c.childrens_parents
            ),
            (method, value, parents, children, at)
        );
    }

    #[test]
    fn config_takes_its_key_as_an_argument() {
        let c = one("config.materialized:incremental");
        assert_eq!(
            (c.method.as_str(), c.args.clone(), c.value.as_str()),
            ("config", vec!["materialized".to_string()], "incremental")
        );
    }

    #[test]
    fn spaces_are_a_union_and_commas_an_intersection() {
        let sel = parse("tag:a,resource_type:model  +b").unwrap();
        let shape: Vec<Vec<&str>> = sel
            .groups
            .iter()
            .map(|g| g.iter().map(|c| c.value.as_str()).collect())
            .collect();
        assert_eq!(shape, vec![vec!["a", "model"], vec!["b"]]);
        let b = &sel.groups[1][0];
        assert_eq!((b.start, b.end), (27, 29));
    }

    #[rstest]
    #[case("tag:", 0, 4, "needs a value")]
    #[case("a,,b", 2, 3, "empty criterion")]
    #[case("stat:x", 0, 5, "no selection method")]
    #[case("state:modified", 0, 14, "previous dbt run")]
    #[case("@+x", 0, 3, "do not go together")]
    #[case("config:table", 0, 12, "names a config key")]
    #[case("ok bogus:x", 3, 9, "no selection method")]
    fn errors_point_at_what_is_wrong(
        #[case] text: &str,
        #[case] start: usize,
        #[case] end: usize,
        #[case] says: &str,
    ) {
        let err = parse(text).unwrap_err();
        assert_eq!((err.start, err.end), (start, end), "{}", err.message);
        assert!(err.message.contains(says), "{}", err.message);
    }

    #[test]
    fn an_inline_exclude_is_split_off() {
        assert_eq!(
            split_exclude("tag:a +b --exclude resource_type:test"),
            ("tag:a +b", Some("resource_type:test"))
        );
        assert_eq!(split_exclude("tag:a"), ("tag:a", None));
    }

    #[test]
    fn nothing_selects_nothing() {
        assert_eq!(parse("   ").unwrap(), Selection::default());
    }
}

//! dbt's selector string syntax (`--select`, and the string form of a
//! selectors.yml definition): words separated by spaces are a union, a
//! word's comma-separated parts an intersection, and each part one
//! criterion `[@][N+][method:]value[+N]` — dbt-core's
//! `RAW_SELECTOR_PATTERN` (graph/selector_spec.py), read part by part.

use super::method::Method;

/// How far a graph operator walks: `None` is all the way.
pub type Depth = Option<u32>;

/// One `[@][N+][method:]value[+N]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criterion {
    pub method: Method,
    /// Whether the text named the method (else it is dbt's default).
    pub method_written: bool,
    pub value: String,
    pub parents: Option<Depth>,
    pub children: Option<Depth>,
    /// `@`: the node, its descendants and all of their ancestors.
    pub childrens_parents: bool,
    /// Byte span of the whole criterion in the text it was read from.
    pub span: (usize, usize),
}

/// A union of intersections, as dbt reads one `--select` string.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Selection {
    pub union: Vec<Vec<Criterion>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxError {
    pub span: (usize, usize),
    pub message: String,
}

/// Parse a selector string. An empty string selects nothing.
pub fn parse(text: &str) -> Result<Selection, SyntaxError> {
    let mut union = Vec::new();
    for (start, word) in words(text, |c| c.is_whitespace()) {
        // `a,,b`, `,a`, `a,`: dbt reads an empty criterion there
        let bytes = word.as_bytes();
        if let Some(at) = (0..bytes.len())
            .find(|&i| bytes[i] == b',' && (i == 0 || bytes[i - 1] == b',' || i + 1 == bytes.len()))
        {
            return Err(SyntaxError {
                span: (start + at, start + at + 1),
                message: format!("`{word}`: an empty criterion at a `,`"),
            });
        }
        let mut intersection = Vec::new();
        for (offset, part) in words(word, |c| c == ',') {
            intersection.push(criterion(part, start + offset)?);
        }
        if !intersection.is_empty() {
            union.push(intersection);
        }
    }
    Ok(Selection { union })
}

/// The selector that picks exactly these names (dbt's space union) —
/// what the run lenses send.
pub fn union_of<S: AsRef<str>>(names: &[S]) -> String {
    names
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Non-empty pieces of `text` split at `sep`, with their byte offsets.
fn words(text: &str, sep: impl Fn(char) -> bool) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        match (sep(c), start) {
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
    out
}

fn criterion(part: &str, at: usize) -> Result<Criterion, SyntaxError> {
    let span = (at, at + part.len());
    let fail = |message: String| SyntaxError { span, message };
    let mut rest = part;
    let childrens_parents = rest.starts_with('@');
    if childrens_parents {
        rest = &rest[1..];
    }
    let mut parents = None;
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    if rest[digits..].starts_with('+') {
        parents = Some(depth(&rest[..digits]));
        rest = &rest[digits + 1..];
    }
    let mut children = None;
    let trailing = rest.chars().rev().take_while(char::is_ascii_digit).count();
    let body = &rest[..rest.len() - trailing];
    if let Some(body) = body.strip_suffix('+') {
        children = Some(depth(&rest[rest.len() - trailing..]));
        rest = body;
    }
    if childrens_parents && parents.is_some() {
        return Err(fail(format!(
            "`{part}`: `@` and a parents `+` cannot be combined"
        )));
    }
    let (method, method_written, value) = match rest.split_once(':') {
        Some((name, value))
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '.') =>
        {
            (Method::parse(name), true, value)
        }
        _ => (Method::default_for(rest), false, rest),
    };
    if value.is_empty() {
        return Err(fail(format!("`{part}` selects no value")));
    }
    Ok(Criterion {
        method,
        method_written,
        value: value.to_string(),
        parents,
        children,
        childrens_parents,
        span,
    })
}

fn depth(digits: &str) -> Depth {
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> Criterion {
        parse(text).unwrap().union.remove(0).remove(0)
    }

    #[test]
    fn criteria_cases() {
        // (text, method, value, parents, children, @)
        type Case = (
            &'static str,
            Method,
            &'static str,
            Option<Depth>,
            Option<Depth>,
            bool,
        );
        let cases: &[Case] = &[
            ("customers", Method::Fqn, "customers", None, None, false),
            (
                "+customers",
                Method::Fqn,
                "customers",
                Some(None),
                None,
                false,
            ),
            (
                "customers+",
                Method::Fqn,
                "customers",
                None,
                Some(None),
                false,
            ),
            (
                "2+customers+1",
                Method::Fqn,
                "customers",
                Some(Some(2)),
                Some(Some(1)),
                false,
            ),
            ("@customers", Method::Fqn, "customers", None, None, true),
            (
                "@tag:nightly+",
                Method::Tag,
                "nightly",
                None,
                Some(None),
                true,
            ),
            ("tag:nightly", Method::Tag, "nightly", None, None, false),
            (
                "config.materialized:table",
                Method::Config("materialized".into()),
                "table",
                None,
                None,
                false,
            ),
            (
                "source:raw.orders+",
                Method::Source,
                "raw.orders",
                None,
                Some(None),
                false,
            ),
            (
                "models/staging",
                Method::Path,
                "models/staging",
                None,
                None,
                false,
            ),
            (
                "path:models/staging+",
                Method::Path,
                "models/staging",
                None,
                Some(None),
                false,
            ),
            (
                "customers.sql",
                Method::File,
                "customers.sql",
                None,
                None,
                false,
            ),
            (
                "jaffle_shop.staging.*",
                Method::Fqn,
                "jaffle_shop.staging.*",
                None,
                None,
                false,
            ),
            (
                "state:modified",
                Method::NeedsState("state"),
                "modified",
                None,
                None,
                false,
            ),
            (
                "tagz:x",
                Method::Unknown("tagz".into()),
                "x",
                None,
                None,
                false,
            ),
        ];
        for (text, method, value, parents, children, at) in cases {
            let c = one(text);
            assert_eq!(
                (
                    &c.method,
                    c.value.as_str(),
                    c.parents,
                    c.children,
                    c.childrens_parents
                ),
                (method, *value, *parents, *children, *at),
                "{text}"
            );
        }
    }

    #[test]
    fn spaces_union_commas_intersect() {
        let s = parse("  a b,tag:x  +c ").unwrap();
        let shape: Vec<Vec<&str>> = s
            .union
            .iter()
            .map(|i| i.iter().map(|c| c.value.as_str()).collect())
            .collect();
        assert_eq!(shape, vec![vec!["a"], vec!["b", "x"], vec!["c"]]);
        assert_eq!(s.union[1][1].span, (6, 11));
        assert!(parse("").unwrap().union.is_empty());
    }

    #[test]
    fn errors_point_at_the_criterion() {
        let cases = [("a @+b", (2, 5)), ("tag:", (0, 4)), ("a,+", (2, 3))];
        for (text, span) in cases {
            assert_eq!(parse(text).unwrap_err().span, span, "{text}");
        }
    }

    #[test]
    fn union_of_joins_names() {
        assert_eq!(union_of(&["a", "b"]), "a b");
    }
}

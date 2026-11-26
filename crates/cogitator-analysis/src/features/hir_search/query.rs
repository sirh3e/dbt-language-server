//! The structural query language of `dbt.search` (docs/search.md):
//! clauses over a model's SQL structure, combined with `and`, `or`, `not`
//! and parentheses (their grammar is in [`super::expr`]). It is a small
//! language of our own, read word by word — not SQL or Jinja, which only
//! the front end reads.
//!
//! ```text
//! clause   := "joins" rel ["on" column]
//!           | "select-star" ["from" rel]
//!           | "reads" rel
//!           | "uses" "column" rel "." column
//! rel      := "model" | "source"              any model / any source
//!           | "model:" name | name            one model
//!           | "source:" source ["." table]    one source (or its tables)
//! ```
//!
//! Names compare case-insensitively.

use cogitator_error::guidance::Guidance;

use super::expr::{words, Expr, Reader};

/// What a relation pattern accepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rel {
    AnyModel,
    AnySource,
    Model(String),
    Source {
        source: String,
        table: Option<String>,
    },
}

impl Rel {
    pub fn accepts_model(&self, name: &str) -> bool {
        match self {
            Rel::AnyModel => true,
            Rel::Model(m) => m.eq_ignore_ascii_case(name),
            _ => false,
        }
    }

    pub fn accepts_source(&self, source: &str, table: &str) -> bool {
        match self {
            Rel::AnySource => true,
            Rel::Source {
                source: s,
                table: t,
            } => {
                s.eq_ignore_ascii_case(source)
                    && t.as_deref().is_none_or(|t| t.eq_ignore_ascii_case(table))
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Clause {
    Joins { rel: Rel, on: Option<String> },
    SelectStar { from: Option<Rel> },
    Reads(Rel),
    UsesColumn { rel: Rel, column: String },
}

impl Clause {
    /// The clause as the query wrote it, for a match's `env`.
    pub fn describe(&self) -> String {
        match self {
            Clause::Joins { rel, on: None } => format!("joins {}", rel_text(rel)),
            Clause::Joins { rel, on: Some(c) } => format!("joins {} on {c}", rel_text(rel)),
            Clause::SelectStar { from: None } => "select-star".to_string(),
            Clause::SelectStar { from: Some(r) } => format!("select-star from {}", rel_text(r)),
            Clause::Reads(r) => format!("reads {}", rel_text(r)),
            Clause::UsesColumn { rel, column } => format!("uses column {}.{column}", rel_text(rel)),
        }
    }
}

fn rel_text(rel: &Rel) -> String {
    match rel {
        Rel::AnyModel => "model".into(),
        Rel::AnySource => "source".into(),
        Rel::Model(m) => m.clone(),
        Rel::Source {
            source,
            table: None,
        } => format!("source:{source}"),
        Rel::Source {
            source,
            table: Some(t),
        } => format!("source:{source}.{t}"),
    }
}

/// A whole query: its clauses, each evaluated once per model, and how
/// they combine ([`Expr`]'s leaves index `clauses`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub clauses: Vec<Clause>,
    pub expr: Expr,
}

impl Query {
    /// The query written back canonically.
    pub fn describe(&self) -> String {
        self.expr.describe(&self.clauses)
    }
}

const HELP: &str =
    "write clauses combined with `and`, `or`, `not` and parentheses: `joins <rel> [on <column>]`, `select-star [from <rel>]`, \
     `reads <rel>`, `uses column <rel>.<column>`; a <rel> is `model`, `source`, a model name, \
     `model:<name>` or `source:<source>[.<table>]` (docs/search.md).";

pub(super) fn refuse(what: String) -> Guidance {
    Guidance::new("dbt.search cannot read the query.")
        .because(what)
        .fix(HELP)
}

fn is_name(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
}

pub fn parse_rel(word: &str) -> Result<Rel, Guidance> {
    let bad = || refuse(format!("'{word}' is no relation"));
    match word.to_ascii_lowercase().as_str() {
        "model" => return Ok(Rel::AnyModel),
        "source" => return Ok(Rel::AnySource),
        _ => {}
    }
    if let Some(rest) = word.strip_prefix("source:") {
        let (source, table) = match rest.split_once('.') {
            Some((s, t)) => (s, Some(t)),
            None => (rest, None),
        };
        if !is_name(source) || table.is_some_and(|t| !is_name(t)) {
            return Err(bad());
        }
        return Ok(Rel::Source {
            source: source.to_string(),
            table: table.map(str::to_string),
        });
    }
    let name = word.strip_prefix("model:").unwrap_or(word);
    match is_name(name) {
        true => Ok(Rel::Model(name.to_string())),
        false => Err(bad()),
    }
}

pub fn parse(text: &str) -> Result<Query, Guidance> {
    let words = words(text);
    if words.is_empty() {
        return Err(refuse("the query is empty".into()));
    }
    let mut reader = Reader::new(&words);
    let expr = reader.query()?;
    Ok(Query {
        clauses: reader.clauses,
        expr,
    })
}

pub(super) fn clause(words: &[&str]) -> Result<Clause, Guidance> {
    let lower: Vec<String> = words.iter().map(|w| w.to_ascii_lowercase()).collect();
    let lower: Vec<&str> = lower.iter().map(String::as_str).collect();
    match lower.as_slice() {
        [] => Err(refuse(
            "a clause is empty (two operators in a row, or one at an end?)".into(),
        )),
        ["joins", _] => Ok(Clause::Joins {
            rel: parse_rel(words[1])?,
            on: None,
        }),
        ["joins", _, "on", c] if is_name(c) => Ok(Clause::Joins {
            rel: parse_rel(words[1])?,
            on: Some(words[3].to_string()),
        }),
        ["select-star"] => Ok(Clause::SelectStar { from: None }),
        ["select-star", "from", _] => Ok(Clause::SelectStar {
            from: Some(parse_rel(words[2])?),
        }),
        ["reads", _] => Ok(Clause::Reads(parse_rel(words[1])?)),
        ["uses", "column", target] => {
            let (rel, _) = target
                .rsplit_once('.')
                .filter(|(_, c)| is_name(c))
                .ok_or_else(|| refuse(format!("'{}' is no <rel>.<column>", words[2])))?;
            let at = rel.len();
            Ok(Clause::UsesColumn {
                rel: parse_rel(&words[2][..at])?,
                column: words[2][at + 1..].to_string(),
            })
        }
        _ => Err(refuse(format!("'{}' is no clause", words.join(" ")))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clauses() {
        let model = |m: &str| Rel::Model(m.into());
        for (text, want) in [
            (
                "joins stg_orders on customer_id",
                vec![Clause::Joins {
                    rel: model("stg_orders"),
                    on: Some("customer_id".into()),
                }],
            ),
            (
                "select-star from source and reads model:customers",
                vec![
                    Clause::SelectStar {
                        from: Some(Rel::AnySource),
                    },
                    Clause::Reads(model("customers")),
                ],
            ),
            (
                "READS source:shop.orders",
                vec![Clause::Reads(Rel::Source {
                    source: "shop".into(),
                    table: Some("orders".into()),
                })],
            ),
            (
                "uses column orders.amount",
                vec![Clause::UsesColumn {
                    rel: model("orders"),
                    column: "amount".into(),
                }],
            ),
            (
                "uses column source:shop.orders.amount",
                vec![Clause::UsesColumn {
                    rel: Rel::Source {
                        source: "shop".into(),
                        table: Some("orders".into()),
                    },
                    column: "amount".into(),
                }],
            ),
        ] {
            assert_eq!(parse(text).ok().map(|q| q.clauses), Some(want), "{text}");
        }
    }

    #[test]
    fn refusals() {
        for (text, why) in [
            ("", "empty"),
            ("joins", "no clause"),
            ("reads a and and reads b", "empty"),
            ("joins x on", "no clause"),
            ("uses column amount", "no <rel>.<column>"),
            ("reads source:a.b.c", "no relation"),
            ("selects stuff", "no clause"),
        ] {
            let err = parse(text).unwrap_err().to_message();
            assert!(err.contains(why), "{text}: {err}");
        }
    }

    #[test]
    fn describes_round_trip() {
        let q = parse("joins stg_orders on id and select-star from source:a").unwrap();
        let back: Vec<String> = q.clauses.iter().map(Clause::describe).collect();
        assert_eq!(
            back,
            ["joins stg_orders on id", "select-star from source:a"]
        );
    }
}

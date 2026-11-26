//! The boolean structure of a `dbt.search` query: clauses combined with
//! `and`, `or`, `not` and parentheses (docs/search.md).
//!
//! ```text
//! query  := or
//! or     := and ("or" and)*
//! and    := unary ("and" unary)*
//! unary  := "not" unary | "(" or ")" | clause
//! ```
//!
//! `not` binds tightest, then `and`, then `or`. A clause reads its words
//! up to the next `and`, `or` or `)`. The leaves are indices into
//! [`super::query::Query::clauses`], so each clause is evaluated once per
//! model however often the tree names it.

use cogitator_error::guidance::Guidance;

use super::query::{clause, refuse, Clause};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// The clause at this index of the query's clauses.
    Leaf(usize),
    Not(Box<Expr>),
    And(Vec<Expr>),
    Or(Vec<Expr>),
}

impl Expr {
    /// Whether the expression holds, given whether each clause found a site.
    pub fn holds(&self, found: &[bool]) -> bool {
        match self {
            Expr::Leaf(i) => found[*i],
            Expr::Not(e) => !e.holds(found),
            Expr::And(es) => es.iter().all(|e| e.holds(found)),
            Expr::Or(es) => es.iter().any(|e| e.holds(found)),
        }
    }

    /// The clauses whose sites show why the expression holds: every
    /// clause of an `and`, the `or` branches that held, nothing under a
    /// `not` (a site of a negated clause is where it does *not* hold).
    pub fn witnesses(&self, found: &[bool]) -> Vec<usize> {
        match self {
            Expr::Leaf(i) => vec![*i],
            Expr::Not(_) => Vec::new(),
            Expr::And(es) => es.iter().flat_map(|e| e.witnesses(found)).collect(),
            Expr::Or(es) => es
                .iter()
                .filter(|e| e.holds(found))
                .flat_map(|e| e.witnesses(found))
                .collect(),
        }
    }

    /// The expression written back, with parentheses only where needed.
    pub fn describe(&self, clauses: &[Clause]) -> String {
        let part = |e: &Expr, parent: u8| match e.precedence() < parent {
            true => format!("({})", e.describe(clauses)),
            false => e.describe(clauses),
        };
        match self {
            Expr::Leaf(i) => clauses[*i].describe(),
            Expr::Not(e) => format!("not {}", part(e, 3)),
            Expr::And(es) => es
                .iter()
                .map(|e| part(e, 2))
                .collect::<Vec<_>>()
                .join(" and "),
            Expr::Or(es) => es
                .iter()
                .map(|e| part(e, 1))
                .collect::<Vec<_>>()
                .join(" or "),
        }
    }

    fn precedence(&self) -> u8 {
        match self {
            Expr::Or(_) => 1,
            Expr::And(_) => 2,
            Expr::Not(_) => 3,
            Expr::Leaf(_) => 4,
        }
    }
}

/// `(` and `)` as words of their own, wherever they touch a word.
pub(super) fn words(text: &str) -> Vec<&str> {
    text.split_whitespace()
        .flat_map(|word| {
            let mut parts = Vec::new();
            let mut rest = word;
            while let Some(r) = rest.strip_prefix('(') {
                parts.push("(");
                rest = r;
            }
            let closing = rest.len() - rest.trim_end_matches(')').len();
            let core = &rest[..rest.len() - closing];
            if !core.is_empty() {
                parts.push(core);
            }
            parts.extend(std::iter::repeat_n(")", closing));
            parts
        })
        .collect()
}

fn is(word: Option<&&str>, keyword: &str) -> bool {
    word.is_some_and(|w| w.eq_ignore_ascii_case(keyword))
}

/// A recursive-descent reader over the query's words, collecting the
/// clauses it meets in order.
pub(super) struct Reader<'a> {
    words: &'a [&'a str],
    at: usize,
    pub(super) clauses: Vec<Clause>,
}

impl<'a> Reader<'a> {
    pub(super) fn new(words: &'a [&'a str]) -> Self {
        Reader {
            words,
            at: 0,
            clauses: Vec::new(),
        }
    }

    /// The whole query; words left over are a `)` that closes nothing.
    pub(super) fn query(&mut self) -> Result<Expr, Guidance> {
        let expr = self.or()?;
        match self.words.get(self.at) {
            None => Ok(expr),
            Some(w) => Err(refuse(format!("'{w}' closes nothing (a `)` too many?)"))),
        }
    }

    fn or(&mut self) -> Result<Expr, Guidance> {
        self.joined("or", Self::and, Expr::Or)
    }

    fn and(&mut self) -> Result<Expr, Guidance> {
        self.joined("and", Self::unary, Expr::And)
    }

    fn joined(
        &mut self,
        keyword: &str,
        part: fn(&mut Self) -> Result<Expr, Guidance>,
        make: fn(Vec<Expr>) -> Expr,
    ) -> Result<Expr, Guidance> {
        let mut parts = vec![part(self)?];
        while is(self.words.get(self.at), keyword) {
            self.at += 1;
            parts.push(part(self)?);
        }
        Ok(match parts.len() {
            1 => parts.pop().expect("one part"),
            _ => make(parts),
        })
    }

    fn unary(&mut self) -> Result<Expr, Guidance> {
        let word = self.words.get(self.at);
        if is(word, "not") {
            self.at += 1;
            if self.at == self.words.len() {
                return Err(refuse("`not` has nothing after it".into()));
            }
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        if word == Some(&"(") {
            self.at += 1;
            let inner = self.or()?;
            if self.words.get(self.at) != Some(&")") {
                return Err(refuse("a `(` is not closed".into()));
            }
            self.at += 1;
            return Ok(inner);
        }
        let start = self.at;
        while let Some(w) = self.words.get(self.at) {
            if *w == ")" || *w == "(" || ["and", "or"].iter().any(|k| w.eq_ignore_ascii_case(k)) {
                break;
            }
            self.at += 1;
        }
        let parsed = clause(&self.words[start..self.at])?;
        self.clauses.push(parsed);
        Ok(Expr::Leaf(self.clauses.len() - 1))
    }
}

#[cfg(test)]
#[path = "expr_tests.rs"]
mod tests;

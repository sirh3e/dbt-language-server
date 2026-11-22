//! S-expressions: the plan, and what analyses say of it, as nested lists
//! a reader can scan and a test can snapshot (specs/ir-pipeline/10 §6).
//!
//! `Sexp` is a value; `pretty(width)` prints it flat when it fits and
//! otherwise one child a line, indented under its head. Ids never appear:
//! columns and relations go by name, so equal plans print equally.

use std::fmt;

/// An S-expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sexp {
    /// A bare symbol: a name, a keyword, a number.
    Atom(String),
    /// A string, printed quoted.
    Str(String),
    List(Vec<Sexp>),
}

/// A symbol.
pub fn atom(text: impl Into<String>) -> Sexp {
    Sexp::Atom(text.into())
}

/// A quoted string.
pub fn string(text: impl Into<String>) -> Sexp {
    Sexp::Str(text.into())
}

/// `(head items...)`.
pub fn list(head: &str, items: impl IntoIterator<Item = Sexp>) -> Sexp {
    let mut all = vec![atom(head)];
    all.extend(items);
    Sexp::List(all)
}

impl Sexp {
    /// The one-line form.
    pub fn flat(&self) -> String {
        let mut out = String::new();
        self.write_flat(&mut out);
        out
    }

    fn write_flat(&self, out: &mut String) {
        match self {
            Sexp::Atom(a) => out.push_str(a),
            Sexp::Str(s) => {
                out.push('"');
                for c in s.chars() {
                    if c == '"' || c == '\\' {
                        out.push('\\');
                    }
                    out.push(c);
                }
                out.push('"');
            }
            Sexp::List(items) => {
                out.push('(');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    item.write_flat(out);
                }
                out.push(')');
            }
        }
    }

    /// Flat where it fits in `width` columns, otherwise the head and its
    /// atoms on the first line and every list child on a line of its own,
    /// two columns in.
    pub fn pretty(&self, width: usize) -> String {
        let mut out = String::new();
        self.write_pretty(0, width, &mut out);
        out
    }

    fn write_pretty(&self, indent: usize, width: usize, out: &mut String) {
        let flat = self.flat();
        let Sexp::List(items) = self else {
            out.push_str(&flat);
            return;
        };
        if indent + flat.len() <= width {
            out.push_str(&flat);
            return;
        }
        out.push('(');
        // leading atoms stay on the head's line: `(aggregate`, `(scan events`
        let lead = items
            .iter()
            .take_while(|i| !matches!(i, Sexp::List(_)))
            .count()
            .max(1);
        for (i, item) in items[..lead].iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            item.write_flat(out);
        }
        for item in &items[lead..] {
            out.push('\n');
            out.push_str(&" ".repeat(indent + 2));
            item.write_pretty(indent + 2, width, out);
        }
        out.push(')');
    }
}

impl fmt::Display for Sexp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.pretty(100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_lists_print_flat_and_strings_are_quoted() {
        let s = list(
            "filter",
            [list("=", [atom("status"), string("o\"k")]), atom("x")],
        );
        assert_eq!(s.flat(), "(filter (= status \"o\\\"k\") x)");
        assert_eq!(s.pretty(80), s.flat());
    }

    #[test]
    fn long_lists_break_one_child_a_line_under_their_head() {
        let scan = list(
            "scan",
            [
                atom("events"),
                list("grows", [atom("ts"), atom("declared")]),
            ],
        );
        let s = list("filter", [list("=", [atom("status"), string("ok")]), scan]);
        assert_eq!(
            s.pretty(36),
            "(filter\n  (= status \"ok\")\n  (scan events (grows ts declared)))"
        );
    }
}

//! The layout as a document, and its printer: Wadler's "prettier printer"
//! as Prettier runs it. A [`Doc::Group`] is laid out flat when it fits the
//! rest of the line, else broken; its separators follow it.
//!
//! - a `Fixed` separator never breaks (`a + b`);
//! - a `Soft` one breaks with its group (`a,` then the next target);
//! - a `Hard` one always breaks, and breaks every group around it.
//!
//! [`Doc::Shift`] moves the indentation of every later line, which is how a
//! Jinja block (`{% if %}` … `{% endif %}`) indents the lines inside it
//! without being a part of SQL's tree.

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Break {
    Fixed,
    Soft,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sep {
    /// The text when it does not break: `" "` or `""`.
    pub flat: &'static str,
    pub kind: Break,
    /// Blank lines after the break.
    pub blanks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Doc {
    Text(String),
    Sep(Sep),
    /// One level deeper, for the lines that break inside it.
    Nest(Vec<Doc>),
    Group {
        docs: Vec<Doc>,
        /// Must break: a Hard separator, text over lines, or a rule.
        breaks: bool,
    },
    Shift(isize),
}

/// Whether a run of docs must break wherever it stands.
pub fn must_break(docs: &[Doc]) -> bool {
    docs.iter().any(|doc| match doc {
        Doc::Text(text) => text.contains('\n'),
        Doc::Sep(sep) => sep.kind == Break::Hard,
        Doc::Nest(docs) => must_break(docs),
        Doc::Group { breaks, .. } => *breaks,
        Doc::Shift(_) => false,
    })
}

pub fn group(docs: Vec<Doc>, forced: bool) -> Doc {
    let breaks = forced || must_break(&docs);
    Doc::Group { docs, breaks }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Broken,
}

/// The text of a document under the rules' width and indentation.
pub fn print(docs: &[Doc], config: &Config) -> String {
    let mut printer = Printer {
        config,
        out: String::new(),
        column: 0,
        shift: 0,
        line_level: 0,
    };
    // a stack of what is left to print, the next on top
    let mut stack: Vec<(usize, Mode, &Doc)> =
        docs.iter().rev().map(|d| (0, Mode::Broken, d)).collect();
    while let Some((level, mode, doc)) = stack.pop() {
        match doc {
            Doc::Text(text) => printer.text(text),
            Doc::Sep(sep) => match (mode, sep.kind) {
                (_, Break::Hard) | (Mode::Broken, Break::Soft) => {
                    printer.newline(level, sep.blanks)
                }
                _ => printer.text(sep.flat),
            },
            Doc::Shift(by) => printer.shift += by,
            Doc::Nest(docs) => stack.extend(docs.iter().rev().map(|d| (level + 1, mode, d))),
            Doc::Group { docs, breaks } => {
                let flat = mode == Mode::Flat || (!breaks && fits(&printer, docs, &stack));
                let inner = if flat { Mode::Flat } else { Mode::Broken };
                // a group's lines are indented from the line it starts on,
                // not from every group it is in (sqlfluff: a bracket closes
                // at the indent of the line that opened it)
                let base = printer.line_level.min(level);
                stack.extend(docs.iter().rev().map(|d| (base, inner, d)));
            }
        }
    }
    printer.out
}

struct Printer<'c> {
    config: &'c Config,
    out: String,
    column: usize,
    shift: isize,
    /// The level of the line being printed, before its shift.
    line_level: usize,
}

impl Printer<'_> {
    fn text(&mut self, text: &str) {
        self.out.push_str(text);
        self.column = match text.rfind('\n') {
            Some(at) => width(&text[at + 1..]),
            None => self.column + width(text),
        };
    }

    fn newline(&mut self, level: usize, blanks: usize) {
        let kept = self.out.trim_end_matches([' ', '\t']).len();
        self.out.truncate(kept);
        self.out.push_str(&"\n".repeat(1 + blanks));
        self.line_level = level;
        let levels = (level as isize + self.shift).max(0) as usize;
        let indent = self.config.indent(levels);
        self.column = levels * self.config.indent_width();
        self.out.push_str(&indent);
    }
}

fn width(text: &str) -> usize {
    text.chars().count()
}

/// Whether `docs`, flat, and what follows them up to the next line break
/// fit in what is left of the line.
fn fits(printer: &Printer<'_>, docs: &[Doc], rest: &[(usize, Mode, &Doc)]) -> bool {
    let limit = printer.config.max_line_length;
    let mut left = limit as isize - printer.column as isize;
    let mut work: Vec<(Mode, &Doc)> = docs.iter().rev().map(|d| (Mode::Flat, d)).collect();
    let mut rest = rest.iter().rev();
    loop {
        if left < 0 {
            return false;
        }
        let Some((mode, doc)) = work.pop().or_else(|| rest.next().map(|(_, m, d)| (*m, *d))) else {
            return true;
        };
        match doc {
            Doc::Text(text) => match text.find('\n') {
                Some(at) => return left >= width(&text[..at]) as isize,
                None => left -= width(text) as isize,
            },
            Doc::Sep(sep) => match (mode, sep.kind) {
                (_, Break::Hard) | (Mode::Broken, Break::Soft) => return true,
                _ => left -= sep.flat.len() as isize,
            },
            Doc::Shift(_) => {}
            Doc::Nest(docs) => work.extend(docs.iter().rev().map(|d| (mode, d))),
            Doc::Group { docs, breaks } => {
                let inner = if mode == Mode::Flat && !breaks {
                    Mode::Flat
                } else {
                    Mode::Broken
                };
                work.extend(docs.iter().rev().map(|d| (inner, d)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Doc {
        Doc::Text(s.into())
    }
    fn sep(flat: &'static str, kind: Break) -> Doc {
        Doc::Sep(Sep {
            flat,
            kind,
            blanks: 0,
        })
    }
    /// `select` and its targets, as the builder makes them.
    fn select(targets: &[&str], width: usize) -> String {
        let mut nest = vec![sep(" ", Break::Soft)];
        for (i, t) in targets.iter().enumerate() {
            if i > 0 {
                nest.extend([text(","), sep(" ", Break::Soft)]);
            }
            nest.push(text(t));
        }
        let doc = group(vec![text("select"), Doc::Nest(nest)], false);
        let config = Config {
            max_line_length: width,
            ..Config::default()
        };
        print(&[doc], &config)
    }

    #[test]
    fn a_group_that_fits_stays_flat() {
        assert_eq!(select(&["a", "b"], 80), "select a, b");
    }

    #[test]
    fn a_group_that_does_not_fit_breaks_every_separator() {
        assert_eq!(
            select(&["alpha", "beta"], 12),
            "select\n    alpha,\n    beta"
        );
    }

    #[test]
    fn a_hard_separator_breaks_its_groups_and_keeps_blank_lines() {
        let docs = vec![
            text("select 1;"),
            Doc::Sep(Sep {
                flat: " ",
                kind: Break::Hard,
                blanks: 1,
            }),
            text("select 2;"),
        ];
        assert_eq!(print(&docs, &Config::default()), "select 1;\n\nselect 2;");
        assert!(must_break(&docs));
    }

    #[test]
    fn a_shift_indents_the_lines_after_it() {
        let hard = || sep("", Break::Hard);
        let docs = vec![
            text("{% if x %}"),
            Doc::Shift(1),
            hard(),
            text("and b"),
            Doc::Shift(-1),
            hard(),
            text("{% endif %}"),
        ];
        assert_eq!(
            print(&docs, &Config::default()),
            "{% if x %}\n    and b\n{% endif %}"
        );
    }

    #[test]
    fn trailing_spaces_never_end_a_line() {
        let docs = vec![
            text("a"),
            sep(" ", Break::Fixed),
            sep(" ", Break::Hard),
            text("b"),
        ];
        assert_eq!(print(&docs, &Config::default()), "a\nb");
    }
}

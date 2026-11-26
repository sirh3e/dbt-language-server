//! What goes between two tokens: a space or none, and whether the line may,
//! must or must not break there.

use super::doc::{Break, Sep};
use super::space::{self, Leaf};
use crate::config::{Commas, Config};

/// What happened between the last token and the next.
#[derive(Debug, Default)]
pub struct Pending {
    /// An Indent: an explicit one (`Some(false)`) breaks with its group.
    pub indent: Option<bool>,
    pub dedent: bool,
    /// A segment that may start a line: a join's condition, a `when`.
    pub soft: bool,
    /// A segment that starts a line of its own: a clause.
    pub hard: bool,
    /// The indentation a segment moves (an indented join).
    pub shift: isize,
    /// A CTE just ended: a blank line before the next one (LT08).
    pub after_cte: bool,
}

/// Whether a token closes what a Dedent just ended: a bracket, or `case`'s
/// `end`.
pub fn closes(leaf: &Leaf<'_>) -> bool {
    leaf.is_close()
        || (leaf.is_keyword && leaf.parent == "case_expression" && leaf.raw.eq_ignore_ascii_case("end"))
}

/// The separator before `next`; `inline_tag`: a Jinja tag written inside a
/// line is on one side, and the line does not break there.
pub fn separator(
    prev: &Leaf<'_>,
    next: &Leaf<'_>,
    pending: &Pending,
    config: &Config,
    inline_tag: bool,
) -> Sep {
    let trailing = config.layout.commas == Commas::Trailing;
    let hard = pending.hard
        || prev.ty == "inline_comment"
        || prev.ty == "statement_terminator"
        || (next.keeps_its_line() && next.newlines_before > 0)
        || (prev.keeps_its_line() && next.newlines_before > 0);
    // before an operator's first token, never inside `||`
    let operator = (next.ty == "binary_operator" || next.parent == "binary_operator") && next.spaced;
    let soft = pending.indent == Some(false)
        || (pending.dedent && closes(next))
        || (trailing && prev.ty == "comma")
        || (!trailing && next.ty == "comma")
        || pending.soft
        || (pending.after_cte && next.ty != "comma")
        || operator;
    let kind = match (hard, soft) {
        (true, _) => Break::Hard,
        _ if inline_tag => Break::Fixed,
        (_, true) => Break::Soft,
        _ => Break::Fixed,
    };
    let from_source = if hard {
        next.newlines_before.saturating_sub(1)
    } else {
        0
    };
    // a blank line between statements and after a CTE whatever the source;
    // the source's own blank lines up to max_blank_lines
    let required = usize::from(
        prev.ty == "statement_terminator"
            || (pending.after_cte && next.ty != "comma" && config.layout.blank_line_after_cte),
    );
    let flat = if !trailing && prev.ty == "comma" {
        " "
    } else {
        space::space(prev, next)
    };
    Sep {
        flat,
        kind,
        blanks: from_source.min(config.max_blank_lines).max(required),
    }
}

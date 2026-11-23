//! Keyword arguments at a call site: in `m(1, b=2)` the `b` is the
//! parameter's name, part of the macro's API, so renaming the parameter
//! renames it. minijinja's AST keeps no span for a keyword, so the
//! argument list right after the callee is read with minijinja's own
//! tokenizer, depth-aware (`m(x=n(b=1), b=2)` touches only the outer `b`).

use cogitator_ir::Span;
use minijinja::machinery::{tokenize, Token};
use minijinja::syntax::SyntaxConfig;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct KeywordArgs {
    /// Where the call passes `name=`.
    pub spans: Vec<Span>,
    /// The call passes the new name already: the rename would make it
    /// pass one keyword twice.
    pub has_new: bool,
}

/// The keyword `name=` of the call whose argument list starts at byte
/// `from` of `text` (after the callee; white space may precede the `(`).
pub(crate) fn keyword_args(text: &str, from: usize, name: &str, new: &str) -> KeywordArgs {
    let mut out = KeywordArgs::default();
    let Some(rest) = text.get(from..) else {
        return out;
    };
    let mut depth = 0u32;
    let mut prev_opens_arg = false;
    let mut pending: Option<(&str, Span)> = None;
    for item in tokenize(rest, true, SyntaxConfig::default()) {
        let Ok((token, span)) = item else {
            break;
        };
        if let Some((ident, at)) = pending.take() {
            if matches!(token, Token::Assign) && depth == 1 {
                if ident == name {
                    out.spans.push(at);
                }
                out.has_new |= ident == new;
            }
        }
        let opens_arg = match token {
            Token::ParenOpen | Token::BracketOpen | Token::BraceOpen => {
                depth += 1;
                true
            }
            Token::ParenClose | Token::BracketClose | Token::BraceClose => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
                false
            }
            Token::Comma => true,
            Token::Ident(ident) if prev_opens_arg && depth == 1 => {
                let at = Span::new(
                    from as u32 + span.start_offset,
                    from as u32 + span.end_offset,
                );
                pending = Some((ident, at));
                false
            }
            _ if depth == 0 => break, // not a call's argument list
            _ => false,
        };
        prev_opens_arg = opens_arg;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
        let from = text.find('(').unwrap();
        keyword_args(text, from, name, "zz")
            .spans
            .iter()
            .map(|s| &text[s.start as usize..s.end as usize])
            .collect()
    }

    #[test]
    fn only_the_outer_keyword_counts() {
        for (text, want) in [
            ("m(b=2)", 1),
            ("m(1, b=2)", 1),
            ("m(b)", 0),
            ("m(x=n(b=1))", 0),
            ("m(x=n(b=1), b=2)", 1),
            ("m(x={'b': 1}, b=[b])", 1),
            ("m(1, c=2)", 0),
            ("m(b == 2)", 0),
        ] {
            assert_eq!(found(text, "b").len(), want, "{text}");
        }
    }

    #[test]
    fn the_new_name_already_passed_is_seen() {
        assert!(keyword_args("m(b=1, zz=2)", 1, "b", "zz").has_new);
        assert!(!keyword_args("m(b=1, n(zz=2))", 1, "b", "zz").has_new);
    }
}

//! Jinja control flow flattened so the SQL binds as one query, without
//! moving a byte: `for` and `endfor` tags are blanked (the body read
//! once), `if` keeps its first branch and blanks every `elif` / `else`
//! branch through its `endif`. What a blanked branch held is not bound,
//! which [`super::missed`] turns into a refusal. Read with minijinja's
//! tokenizer.

use minijinja::machinery::{tokenize, Token};
use minijinja::syntax::SyntaxConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tag {
    If,
    ElseLike,
    EndIf,
    Loop,
}

fn tag_of(word: &str) -> Option<Tag> {
    match word {
        "if" => Some(Tag::If),
        "elif" | "else" => Some(Tag::ElseLike),
        "endif" => Some(Tag::EndIf),
        "for" | "endfor" => Some(Tag::Loop),
        _ => None,
    }
}

/// The control tags in order: (kind, tag start, tag end).
fn control_tags(text: &str) -> Option<Vec<(Tag, usize, usize)>> {
    let mut out = Vec::new();
    // the open tag's start, and its first word once read
    let mut open: Option<(usize, Option<Option<Tag>>)> = None;
    for item in tokenize(text, false, SyntaxConfig::default()) {
        let (token, span) = item.ok()?;
        match (&token, open) {
            (Token::BlockStart, _) => open = Some((span.start_offset as usize, None)),
            (Token::BlockEnd, Some((at, tag))) => {
                if let Some(Some(tag)) = tag {
                    out.push((tag, at, span.end_offset as usize));
                }
                open = None;
            }
            (token, Some((at, None))) => {
                let tag = match token {
                    Token::Ident(word) => tag_of(word),
                    _ => None,
                };
                open = Some((at, Some(tag)));
            }
            _ => {}
        }
    }
    Some(out)
}

fn blank(bytes: &mut [u8]) {
    for b in bytes {
        if !matches!(b, b'\n' | b'\r') {
            *b = b' ';
        }
    }
}

/// The text with its control flow flattened, same length.
pub(super) fn without_control_tags(text: &str) -> Option<String> {
    let mut out = text.as_bytes().to_vec();
    // per open `if`: where its blanked tail starts, once an else is seen
    let mut ifs: Vec<Option<usize>> = Vec::new();
    for (tag, start, end) in control_tags(text)? {
        let blanking = ifs.iter().any(Option::is_some);
        match tag {
            Tag::If => ifs.push(None),
            Tag::ElseLike => {
                if let Some(top @ None) = ifs.last_mut() {
                    *top = Some(start);
                }
            }
            Tag::EndIf => match ifs.pop() {
                Some(Some(from)) => blank(&mut out[from..end]),
                Some(None) => {}
                None => return None, // an endif without its if
            },
            Tag::Loop => {}
        }
        if !blanking && matches!(tag, Tag::If | Tag::Loop) || tag == Tag::EndIf {
            blank(&mut out[start..end]);
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `text` with each listed piece blanked.
    fn blanked(text: &str, pieces: &[&str]) -> String {
        let mut out = text.to_string();
        for piece in pieces {
            out = out.replacen(piece, &" ".repeat(piece.len()), 1);
        }
        out
    }

    #[test]
    fn control_flow_flattens_in_place() {
        for (text, pieces) in [
            ("select 1", &[][..]),
            (
                "a {% for x in y %}b{% endfor %} c",
                &["{% for x in y %}", "{% endfor %}"],
            ),
            ("a {% if x %}b{% endif %}", &["{% if x %}", "{% endif %}"]),
            (
                "a {% if x %}b{% else %}c{% endif %} d",
                &["{% if x %}", "{% else %}c{% endif %}"],
            ),
            (
                "{% if x %}b{% elif y %}c{% else %}d{% endif %}",
                &["{% if x %}", "{% elif y %}c{% else %}d{% endif %}"],
            ),
            (
                "{% if x %}{% if y %}b{% else %}c{% endif %}{% else %}d{% endif %}",
                &[
                    "{% if x %}{% if y %}",
                    "{% else %}c{% endif %}{% else %}d{% endif %}",
                ],
            ),
            ("{{ x }} {% set y = 1 %}", &[]),
        ] {
            let got = without_control_tags(text).unwrap();
            assert_eq!(got, blanked(text, pieces), "{text}");
        }
    }
}

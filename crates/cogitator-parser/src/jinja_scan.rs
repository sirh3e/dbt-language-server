//! Scan-side Jinja extraction with minijinja (specs/rust-port/06 §2):
//! macro signatures, `{% docs %}` blocks, and `{{ doc("…") }}` substitution.
//! Regexes remain as per-file fallback (06 §2.4).

use std::sync::LazyLock;

use minijinja::machinery::{tokenize as jinja_tokenize, Token as JToken};
use minijinja::syntax::SyntaxConfig;
use regex::Regex;

use crate::jinja_spans::{blanked, NOT_CODE};
use crate::LineTable;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RawMacro {
    pub name: String,
    /// Full `name(args…)` signature text (Go kept it as the description).
    pub signature: String,
    /// Structured parameters (spec 06 §2.2 enrichment): names + default
    /// expressions, extracted at token level so dbt-tagged files work too.
    pub params: Vec<MacroParam>,
    /// 0-based (line, column) of the signature start/end.
    pub start: (u32, u32),
    pub end: (u32, u32),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MacroParam {
    pub name: String,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RawDocsBlock {
    pub name: String,
    pub content: String,
    /// 0-based (line, column) of the block's name identifier. Added
    /// after cache format 1: `default` keeps old payloads decodable,
    /// though the format bump already cold-starts them.
    #[serde(default)]
    pub start: (u32, u32),
}

type JinjaTokens<'a> = Vec<(JToken<'a>, minijinja::machinery::Span)>;

fn tokenize_all(text: &str) -> Result<JinjaTokens<'_>, String> {
    jinja_tokenize(text, false, SyntaxConfig::default())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn ident_is<'a>(tok: &'a JToken<'a>, want: &str) -> bool {
    matches!(tok, JToken::Ident(s) if *s == want)
}

/// One `{% snapshot name %}` block (03 §6 addendum): snapshots are
/// ref-able like models; only the name and where it sits matter.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RawSnapshot {
    pub name: String,
    pub start: (u32, u32),
}

/// Extract `{% snapshot name %}` definitions. Tokenize failures fall
/// back to an empty list — a broken snapshot file degrades locally.
pub fn extract_snapshots(text: &str) -> Vec<RawSnapshot> {
    let Ok(tokens) = tokenize_all(text) else {
        return Vec::new();
    };
    let table = LineTable::new(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if matches!(tokens[i].0, JToken::BlockStart)
            && tokens
                .get(i + 1)
                .is_some_and(|(t, _)| ident_is(t, "snapshot"))
        {
            if let Some((JToken::Ident(name), span)) = tokens.get(i + 2).map(|(t, s)| (t, s)) {
                out.push(RawSnapshot {
                    name: (*name).to_string(),
                    start: table.line_col(span.start_offset),
                });
                i += 2;
            }
        }
        i += 1;
    }
    out
}

/// Extract `{% macro name(args…) %}` definitions (06 §2.2).
pub fn extract_macros(text: &str) -> Vec<RawMacro> {
    match tokenize_all(text) {
        Ok(tokens) => extract_macros_from_tokens(text, &tokens),
        Err(err) => {
            tracing::debug!(%err, "minijinja tokenize failed; regex fallback for macros");
            extract_macros_regex(text)
        }
    }
}

fn extract_macros_from_tokens(text: &str, tokens: &JinjaTokens<'_>) -> Vec<RawMacro> {
    let table = LineTable::new(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        // BlockStart, Ident("macro"), Ident(name), ParenOpen … ParenClose
        if matches!(tokens[i].0, JToken::BlockStart)
            && tokens.get(i + 1).is_some_and(|(t, _)| ident_is(t, "macro"))
            && matches!(tokens.get(i + 2).map(|(t, _)| t), Some(JToken::Ident(_)))
            && matches!(tokens.get(i + 3).map(|(t, _)| t), Some(JToken::ParenOpen))
        {
            let name_span = tokens[i + 2].1;
            let name = match &tokens[i + 2].0 {
                JToken::Ident(s) => (*s).to_string(),
                _ => unreachable!(),
            };
            let mut depth = 0i32;
            let mut j = i + 3;
            let mut close = None;
            while let Some((t, span)) = tokens.get(j) {
                match t {
                    JToken::ParenOpen => depth += 1,
                    JToken::ParenClose => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(*span);
                            break;
                        }
                    }
                    JToken::BlockEnd => break, // unterminated header: skip (06 §2.4)
                    _ => {}
                }
                j += 1;
            }
            if let Some(close) = close {
                let sig_start = name_span.start_offset as usize;
                let sig_end = close.end_offset as usize;
                let signature = text[sig_start..sig_end].to_string();
                out.push(RawMacro {
                    name,
                    params: params_from_signature(&signature),
                    signature,
                    start: table.line_col(sig_start as u32),
                    end: table.line_col(sig_end as u32),
                });
                i = j;
            }
        }
        i += 1;
    }
    out
}

/// Extract `{% docs name %} … {% enddocs %}` blocks (06 §2.3).
pub fn extract_docs_blocks(text: &str) -> Vec<RawDocsBlock> {
    match tokenize_all(text) {
        Ok(tokens) => extract_docs_from_tokens(text, &tokens),
        Err(err) => {
            tracing::debug!(%err, "minijinja tokenize failed; regex fallback for docs blocks");
            extract_docs_regex(text)
        }
    }
}

fn extract_docs_from_tokens(text: &str, tokens: &JinjaTokens<'_>) -> Vec<RawDocsBlock> {
    let table = LineTable::new(text);
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if matches!(tokens[i].0, JToken::BlockStart)
            && tokens.get(i + 1).is_some_and(|(t, _)| ident_is(t, "docs"))
        {
            let (name, name_start) = match tokens.get(i + 2).map(|(t, s)| (t, s)) {
                Some((JToken::Ident(s), span)) => {
                    ((*s).to_string(), table.line_col(span.start_offset))
                }
                _ => {
                    i += 1;
                    continue;
                }
            };
            // content runs from the end of this block's BlockEnd to the start
            // of the `{% enddocs %}` BlockStart.
            let mut j = i + 3;
            let mut content_start = None;
            while let Some((t, span)) = tokens.get(j) {
                if matches!(t, JToken::BlockEnd) {
                    content_start = Some(span.end_offset as usize);
                    break;
                }
                j += 1;
            }
            if let Some(content_start) = content_start {
                let mut k = j + 1;
                while let Some((t, span)) = tokens.get(k) {
                    if matches!(t, JToken::BlockStart)
                        && tokens
                            .get(k + 1)
                            .is_some_and(|(t, _)| ident_is(t, "enddocs"))
                    {
                        let content_end = span.start_offset as usize;
                        out.push(RawDocsBlock {
                            name,
                            content: text[content_start..content_end].trim().to_string(),
                            start: name_start,
                        });
                        i = k;
                        break;
                    }
                    k += 1;
                }
            }
        }
        i += 1;
    }
    out
}

/// `{{ doc("name") }}` substitution inside descriptions (06 §2.3).
/// Splices right-to-left so earlier offsets stay valid.
pub fn replace_doc_refs(description: &str, lookup: impl Fn(&str) -> Option<String>) -> String {
    let Ok(tokens) = tokenize_all(description) else {
        return description.to_string();
    };
    let mut replacements: Vec<(usize, usize, String)> = Vec::new();
    for w in tokens.windows(6) {
        if matches!(w[0].0, JToken::VariableStart)
            && ident_is(&w[1].0, "doc")
            && matches!(w[2].0, JToken::ParenOpen)
            && matches!(w[4].0, JToken::ParenClose)
            && matches!(w[5].0, JToken::VariableEnd)
        {
            let name = match &w[3].0 {
                JToken::Str(s) => (*s).to_string(),
                JToken::String(s) => s.to_string(),
                _ => continue,
            };
            if let Some(content) = lookup(&name) {
                replacements.push((
                    w[0].1.start_offset as usize,
                    w[5].1.end_offset as usize,
                    content,
                ));
            }
        }
    }
    let mut result = description.to_string();
    for (start, end, content) in replacements.into_iter().rev() {
        result.replace_range(start..end, &content);
    }
    result
}

// ---------------------------------------------------------------------------
// Regex fallbacks — the Go behavioral reference (03 §5 / §6.4), per-file only.
// ---------------------------------------------------------------------------

static MACRO_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\{%-?\s*macro\s+(\w+\(.*?\))\s*-?%\}").unwrap());
static DOCS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\{%-?\s*docs\s+([a-zA-Z0-9_]+)\s*-?%\}(.*?)\{%-?\s*enddocs\s*%\}").unwrap()
});

fn extract_macros_regex(text: &str) -> Vec<RawMacro> {
    let table = LineTable::new(text);
    let live = blanked(text, NOT_CODE);
    MACRO_RE
        .captures_iter(&live)
        .filter_map(|caps| {
            let m = caps.get(1)?;
            let signature = m.as_str();
            let name = signature.split('(').next()?.to_string();
            Some(RawMacro {
                name,
                params: params_from_signature(signature),
                signature: signature.to_string(),
                start: table.line_col(m.start() as u32),
                end: table.line_col(m.end() as u32),
            })
        })
        .collect()
}

/// Structured params from `name(a, b=expr, …)` — top-level commas only, so
/// defaults containing calls/collections stay intact.
pub fn params_from_signature(signature: &str) -> Vec<MacroParam> {
    let Some(inner) = signature
        .split_once('(')
        .map(|(_, rest)| rest.strip_suffix(')').unwrap_or(rest))
    else {
        return Vec::new();
    };
    let mut params = Vec::new();
    let mut depth = 0i32;
    let mut current = String::new();
    let flush = |piece: &str, params: &mut Vec<MacroParam>| {
        let piece = piece.trim();
        if piece.is_empty() {
            return;
        }
        match piece.split_once('=') {
            Some((name, default)) => params.push(MacroParam {
                name: name.trim().to_string(),
                default: Some(default.trim().to_string()),
            }),
            None => params.push(MacroParam {
                name: piece.to_string(),
                default: None,
            }),
        }
    };
    for c in inner.chars() {
        match c {
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => flush(&std::mem::take(&mut current), &mut params),
            _ => current.push(c),
        }
    }
    flush(&current, &mut params);
    params
}

fn extract_docs_regex(text: &str) -> Vec<RawDocsBlock> {
    let table = LineTable::new(text);
    let live = blanked(text, NOT_CODE);
    DOCS_RE
        .captures_iter(&live)
        .map(|caps| RawDocsBlock {
            name: caps[1].to_string(),
            // From the original: a docs body may hold comments of its own.
            content: text[caps.get(2).map_or(0..0, |m| m.range())]
                .trim()
                .to_string(),
            start: table.line_col(caps.get(1).map(|m| m.start() as u32).unwrap_or_default()),
        })
        .collect()
}

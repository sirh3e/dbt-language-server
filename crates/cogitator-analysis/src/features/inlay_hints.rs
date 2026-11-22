//! textDocument/inlayHint (01 §6.3): the resolved warehouse relation
//! after each ref()/source() call, the value each var() resolves to,
//! and parameter names at macro call sites. Pure — the shell resolves
//! the active target first.

use cogitator_lsp::{inlay_hint_kind, InlayHint, Position, Range};
use cogitator_parser::TokenKind;

use super::relation::TargetRelation;
use super::{relation_info, DbtContext, Document};

/// Which hints a client gets (cogitator.yml `inlay:`).
#[derive(Debug, Clone, Copy)]
pub struct InlayOptions {
    /// The value after each `var()` call.
    pub vars: bool,
}

impl Default for InlayOptions {
    fn default() -> Self {
        InlayOptions { vars: true }
    }
}

pub fn inlay_hints(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    range: Range,
    target: Option<&TargetRelation>,
) -> Vec<InlayHint> {
    inlay_hints_with(ctx, doc, uri, range, target, InlayOptions::default())
}

pub fn inlay_hints_with(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    range: Range,
    target: Option<&TargetRelation>,
    options: InlayOptions,
) -> Vec<InlayHint> {
    let whole_doc = range.is_zero();
    let within = |line: u32| whole_doc || (range.start.line <= line && line <= range.end.line);
    let mut out = Vec::new();
    for (i, token) in doc.parsed.tokens.iter().enumerate() {
        if !within(token.line) {
            continue;
        }
        match token.kind {
            // the relation the call resolves to, after the closing quote
            TokenKind::Ref | TokenKind::SourceTable => {
                let pos = Position {
                    line: token.line,
                    character: token.column,
                };
                let Some(info) = relation_info(ctx, Some(doc), uri, Some(pos), target) else {
                    continue;
                };
                out.push(InlayHint {
                    position: Position {
                        line: token.line,
                        character: token.column + token.len() + 1,
                    },
                    label: format!("→ {}", info.relation),
                    kind: Some(inlay_hint_kind::TYPE),
                    padding_left: true,
                    padding_right: false,
                    tooltip: None,
                });
            }
            TokenKind::Var if options.vars => out.push(var_hint(ctx, doc, uri, i, token)),
            // parameter names for positional macro arguments
            TokenKind::Macro => out.extend(macro_param_hints(ctx, doc, i, token)),
            _ => {}
        }
    }
    out
}

fn macro_param_hints(
    ctx: &DbtContext,
    doc: &Document,
    i: usize,
    token: &cogitator_parser::Token,
) -> Vec<InlayHint> {
    let name = doc.parsed.literal(token);
    let Some(params) = ctx
        .macro_detail_map
        .values()
        .find_map(|package| package.get(name))
        .map(|details| &details.params)
    else {
        return Vec::new();
    };
    let tokens = &doc.parsed.tokens;
    // arguments start after `(`; one hint per positional arg start
    let mut j = i + 1;
    if !matches!(tokens.get(j).map(|t| t.kind), Some(TokenKind::Symbol('('))) {
        return Vec::new();
    }
    j += 1;
    let mut depth = 0usize;
    let mut arg = 0usize;
    let mut at_arg_start = true;
    let mut arg_named = false;
    let mut out = Vec::new();
    while let Some(t) = tokens.get(j) {
        match t.kind {
            TokenKind::Symbol('(') => depth += 1,
            TokenKind::Symbol(')') if depth == 0 => break,
            TokenKind::Symbol(')') => depth -= 1,
            TokenKind::Symbol(',') if depth == 0 => {
                arg += 1;
                at_arg_start = true;
                arg_named = false;
            }
            TokenKind::JinjaClose(_) => break,
            _ if at_arg_start => {
                at_arg_start = false;
                // named arguments (`last_name='b'`) carry their own name:
                // `=` never tokenizes, so read the raw text to the next
                // argument boundary instead
                let boundary = tokens[j..]
                    .iter()
                    .find(|n| {
                        matches!(n.kind, TokenKind::Symbol(',') | TokenKind::Symbol(')'))
                            || matches!(n.kind, TokenKind::JinjaClose(_))
                    })
                    .map(|n| n.start as usize)
                    .unwrap_or(doc.parsed.text.len());
                if doc.parsed.text[t.start as usize..boundary].contains('=') {
                    arg_named = true;
                }
                if !arg_named {
                    if let Some(param) = params.get(arg) {
                        out.push(InlayHint {
                            position: Position {
                                line: t.line,
                                character: t.column.saturating_sub(
                                    if t.kind == TokenKind::StringLit { 1 } else { 0 },
                                ),
                            },
                            label: format!("{}:", param.name),
                            kind: Some(inlay_hint_kind::PARAMETER),
                            padding_left: false,
                            padding_right: true,
                            tooltip: None,
                        });
                    }
                }
            }
            _ => {}
        }
        j += 1;
    }
    out
}

/// `= 'jaffle'` after a `var('…')` call: the value it resolves to, the
/// default when only that applies, or that it is undefined.
fn var_hint(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    i: usize,
    token: &cogitator_parser::Token,
) -> InlayHint {
    let parsed = &doc.parsed;
    let name = parsed.literal(token);
    let call = parsed.var_call(i);
    let default = call
        .default
        .map(|(s, e)| &parsed.text[s as usize..e as usize]);
    let var = ctx.var_for(uri, name);
    // after the closing parenthesis when a default is written, else
    // right after the name's closing quote
    let position = match (default, call.close) {
        (Some(_), Some(close)) => {
            let t = &parsed.tokens[close];
            Position {
                line: t.line,
                character: t.column + t.len(),
            }
        }
        _ => Position {
            line: token.line,
            character: token.column + token.len() + 1,
        },
    };
    let (label, tooltip) = var_hint_label(var, default, &ctx.project.name);
    InlayHint {
        position,
        label,
        kind: Some(inlay_hint_kind::TYPE),
        padding_left: true,
        padding_right: false,
        tooltip: Some(tooltip),
    }
}

/// Longest value text a hint shows before `…`.
const VAR_HINT_CHARS: usize = 40;

/// A var hint's label and tooltip. Pure, for the case table.
pub(crate) fn var_hint_label(
    var: Option<&crate::context::VariableDetails>,
    default: Option<&str>,
    root_project: &str,
) -> (String, String) {
    use crate::context::VarSource;
    let Some(var) = var else {
        return match default {
            Some(d) => (
                format!("= {} (default)", shorten(d)),
                format!("undefined; the default {d} applies"),
            ),
            None => (
                "= ? undefined".to_string(),
                "not defined in dbt_project.yml, a package or an override".to_string(),
            ),
        };
    };
    let shown = match &var.value {
        serde_json::Value::String(s) => format!("'{s}'"),
        other => other.to_string(),
    };
    let from = match &var.source {
        VarSource::Override => " (override)".to_string(),
        VarSource::Package(p) => format!(" ({p})"),
        VarSource::Scoped(scope) if scope != root_project => format!(" ({scope})"),
        _ => String::new(),
    };
    let tooltip = format!(
        "{}\n\n{}",
        serde_json::to_string_pretty(&var.value).unwrap_or(shown.clone()),
        super::var_source_words(var)
    );
    (format!("= {}{from}", shorten(&shown)), tooltip)
}

fn shorten(text: &str) -> String {
    if text.chars().count() <= VAR_HINT_CHARS {
        return text.to_string();
    }
    let cut: String = text.chars().take(VAR_HINT_CHARS - 1).collect();
    format!("{cut}…")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{VarSource, VariableDetails};
    use serde_json::json;

    fn var(value: serde_json::Value, source: VarSource) -> VariableDetails {
        VariableDetails {
            name: "v".into(),
            value,
            uri: "/p/dbt_project.yml".into(),
            source,
            ..Default::default()
        }
    }

    #[test]
    fn var_labels() {
        let long = json!("x".repeat(60));
        let cases: Vec<(Option<VariableDetails>, Option<&str>, &str)> = vec![
            (
                Some(var(json!("jaffle"), VarSource::Project)),
                None,
                "= 'jaffle'",
            ),
            (
                Some(var(json!(1), VarSource::Scoped("shop".into()))),
                None,
                "= 1",
            ),
            (
                Some(var(json!(1), VarSource::Scoped("utils".into()))),
                None,
                "= 1 (utils)",
            ),
            (
                Some(var(json!(1), VarSource::Package("utils".into()))),
                None,
                "= 1 (utils)",
            ),
            (
                Some(var(json!([1, 2]), VarSource::Override)),
                Some("0"),
                "= [1,2] (override)",
            ),
            (None, Some("7"), "= 7 (default)"),
            (None, None, "= ? undefined"),
        ];
        for (v, default, want) in cases {
            assert_eq!(var_hint_label(v.as_ref(), default, "shop").0, want);
        }
        let (label, _) = var_hint_label(Some(&var(long, VarSource::Project)), None, "shop");
        assert_eq!(label.chars().count(), 2 + VAR_HINT_CHARS);
        assert!(label.ends_with('…'), "{label}");
        let (_, tooltip) = var_hint_label(Some(&var(json!(1), VarSource::Project)), None, "shop");
        assert_eq!(tooltip, "1\n\ndbt_project.yml:1");
    }
}

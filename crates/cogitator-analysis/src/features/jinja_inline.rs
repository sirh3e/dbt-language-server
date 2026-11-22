//! What one `{{ … }}` renders to, by dbt.expandJinja's renderer: a hover
//! section and (opt-in, cogitator.yml `inlay.jinja`) inlay hints after
//! short single-line expressions. Bounded: every render runs on a fuel
//! budget and its output is cut at a size, so a runaway loop or a huge
//! expansion degrades to "too large to preview" instead of blocking the
//! request; anything the stubs cannot render says why.

use std::collections::BTreeSet;

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{inlay_hint_kind, InlayHint, Position, Range};
use cogitator_parser::{JinjaKind, TokenKind};
use minijinja::Environment;

use super::jinja_preview::{environment, render_error, PreviewInputs};
use super::Document;

/// Render budget (minijinja fuel: instructions) per expression.
pub const JINJA_FUEL: u64 = 100_000;
/// Longest rendered text a hover shows.
pub const HOVER_CHARS: usize = 2_000;
/// Longest rendered text an inlay hint shows.
pub const HINT_CHARS: usize = 40;
/// At most this many expressions get a hint per request.
pub const MAX_HINTS: usize = 64;

/// One `{{ … }}` of a document.
#[derive(Debug, Clone, PartialEq)]
pub struct JinjaExpr {
    /// The whole expression, braces included.
    pub text: String,
    pub range: Range,
    /// Unqualified macro calls in it (`{{ cents(x) }}`).
    pub macros: Vec<String>,
}

/// Every `{{ … }}` of a document, in order.
pub fn jinja_expressions(doc: &Document) -> Vec<JinjaExpr> {
    let parsed = &doc.parsed;
    let mut out = Vec::new();
    let mut open: Option<usize> = None;
    for (i, t) in parsed.tokens.iter().enumerate() {
        match t.kind {
            TokenKind::JinjaOpen(JinjaKind::Expr) => open = Some(i),
            TokenKind::JinjaClose(JinjaKind::Expr) => {
                let Some(o) = open.take() else { continue };
                let first = &parsed.tokens[o];
                let macros = parsed.tokens[o..i]
                    .iter()
                    .enumerate()
                    .filter(|(k, m)| {
                        m.kind == TokenKind::Macro
                            && !matches!(
                                parsed.tokens.get(o + k - 1).map(|p| p.kind),
                                Some(TokenKind::Symbol('.'))
                            )
                    })
                    .map(|(_, m)| parsed.literal(m).to_string())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect();
                out.push(JinjaExpr {
                    text: parsed.text[first.start as usize..t.end as usize].to_string(),
                    range: Range {
                        start: Position {
                            line: first.line,
                            character: first.column,
                        },
                        end: Position {
                            line: t.line,
                            character: t.column + t.len(),
                        },
                    },
                    macros,
                });
            }
            _ => {}
        }
    }
    out
}

/// The `{{ … }}` the cursor is inside.
pub fn jinja_expression_at(doc: &Document, pos: Position) -> Option<JinjaExpr> {
    jinja_expressions(doc).into_iter().find(|e| {
        let after_start =
            (pos.line, pos.character) >= (e.range.start.line, e.range.start.character);
        let before_end = (pos.line, pos.character) <= (e.range.end.line, e.range.end.character);
        after_start && before_end
    })
}

/// A renderer for many expressions: the environment and the macro
/// sources are set up once.
pub struct InlineRenderer {
    env: Environment<'static>,
    defined: Vec<String>,
}

/// What one render came to.
#[derive(Debug, Clone, PartialEq)]
pub enum Rendered {
    Text(String),
    /// Over the size cap; the first `chars` characters.
    TooLarge(String),
    Failed(String),
}

impl InlineRenderer {
    pub fn new(inputs: &PreviewInputs) -> Self {
        let mut env = environment(inputs);
        env.set_fuel(Some(JINJA_FUEL));
        let macros = inputs.macro_sources.join("\n");
        let ok = env.add_template_owned("__macros", macros.clone()).is_ok();
        let defined = if ok { macro_names(&macros) } else { Vec::new() };
        InlineRenderer { env, defined }
    }

    /// Render one expression, cut at `cap` characters.
    pub fn render(&self, expr: &JinjaExpr, cap: usize) -> Rendered {
        let imports: Vec<&str> = expr
            .macros
            .iter()
            .filter(|m| self.defined.contains(m))
            .map(String::as_str)
            .collect();
        let source = if imports.is_empty() {
            expr.text.clone()
        } else {
            format!(
                "{{% from \"__macros\" import {} %}}{}",
                imports.join(", "),
                expr.text
            )
        };
        match self.env.render_str(&source, minijinja::context! {}) {
            Ok(text) => {
                if text.chars().count() > cap {
                    Rendered::TooLarge(text.chars().take(cap).collect())
                } else {
                    Rendered::Text(text)
                }
            }
            Err(err) => {
                let out_of_fuel = err.kind() == minijinja::ErrorKind::OutOfFuel;
                let guidance: Guidance = if out_of_fuel {
                    Guidance::new("Jinja preview stopped: the expression ran over its budget.")
                } else {
                    render_error(err)
                };
                Rendered::Failed(guidance.to_message())
            }
        }
    }
}

/// `{% macro name(` names of a macro source.
fn macro_names(source: &str) -> Vec<String> {
    cogitator_parser::jinja_scan::extract_macros(source)
        .into_iter()
        .map(|m| m.name)
        .collect()
}

/// The hover section for the `{{ … }}` under the cursor.
pub fn jinja_hover(doc: &Document, pos: Position, renderer: &InlineRenderer) -> Option<String> {
    let expr = jinja_expression_at(doc, pos)?;
    Some(match renderer.render(&expr, HOVER_CHARS) {
        Rendered::Text(t) => format!("Renders to (preview):\n{t}"),
        Rendered::TooLarge(t) => {
            format!("Renders to (preview, first {HOVER_CHARS} characters):\n{t}…")
        }
        Rendered::Failed(why) => format!("Jinja preview: {why}"),
    })
}

/// `⇒ rendered` after each short single-line `{{ … }}` in `range`
/// (whole document when zero). Failures and long renderings get none.
pub fn jinja_inlay_hints(
    doc: &Document,
    range: Range,
    renderer: &InlineRenderer,
) -> Vec<InlayHint> {
    let whole = range.is_zero();
    jinja_expressions(doc)
        .into_iter()
        .filter(|e| e.range.start.line == e.range.end.line)
        .filter(|e| {
            whole
                || (range.start.line <= e.range.start.line && e.range.start.line <= range.end.line)
        })
        .take(MAX_HINTS)
        .filter_map(|e| match renderer.render(&e, HINT_CHARS) {
            Rendered::Text(t) if !t.contains('\n') && !t.trim().is_empty() => Some(InlayHint {
                position: e.range.end,
                label: format!("⇒ {}", t.trim()),
                kind: Some(inlay_hint_kind::TYPE),
                padding_left: true,
                padding_right: false,
                tooltip: Some("Jinja preview (dbt.expandJinja's stubs)".to_string()),
            }),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn inputs() -> PreviewInputs {
        PreviewInputs {
            macro_sources: vec!["{% macro double(x) %}{{ x * 2 }}{% endmacro %}".to_string()],
            model_relations: HashMap::from([("customers".into(), "db.main.customers".into())]),
            source_relations: HashMap::new(),
            vars: HashMap::from([("n".into(), serde_json::json!(3))]),
            target_name: "dev".into(),
            is_incremental: false,
            this: None,
            env: HashMap::from([
                ("SCHEMA".into(), "analytics".into()),
                ("DB_PASSWORD".into(), "hunter2".into()),
            ]),
        }
    }

    const URI: &str = "file:///p/models/m.sql";

    fn docs(text: &str) -> crate::Documents {
        let mut docs = crate::Documents::default();
        docs.open(URI.into(), text, &Default::default());
        docs
    }

    #[test]
    fn expressions_render() {
        let r = InlineRenderer::new(&inputs());
        let text =
            "select {{ ref('customers') }}, {{ double(var('n')) }}, {{ env_var('SCHEMA') }}, \
                    {{ env_var('DB_PASSWORD') }}, {{ nope() }}, {{ range(10**9) | list | length }}";
        let all = docs(text);
        let d = all.get(URI).unwrap();
        let got: Vec<Rendered> = jinja_expressions(d)
            .iter()
            .map(|e| r.render(e, HINT_CHARS))
            .collect();
        assert_eq!(got[0], Rendered::Text("db.main.customers".into()));
        assert_eq!(got[1], Rendered::Text("6".into()));
        assert_eq!(got[2], Rendered::Text("analytics".into()));
        assert_eq!(got[3], Rendered::Text("••••••".into()));
        assert!(matches!(got[4], Rendered::Failed(_)), "{:?}", got[4]);
        assert!(matches!(got[5], Rendered::Failed(_)), "{:?}", got[5]);
    }

    #[test]
    fn hover_and_hints() {
        let r = InlineRenderer::new(&inputs());
        let all = docs("select {{ double(4) }} as x");
        let d = all.get(URI).unwrap();
        let hover = jinja_hover(
            d,
            Position {
                line: 0,
                character: 10,
            },
            &r,
        )
        .unwrap();
        assert_eq!(hover, "Renders to (preview):\n8");
        assert!(jinja_hover(
            d,
            Position {
                line: 0,
                character: 2
            },
            &r
        )
        .is_none());
        let hints = jinja_inlay_hints(d, Range::default(), &r);
        assert_eq!(hints.len(), 1);
        assert_eq!(hints[0].label, "⇒ 8");
        assert_eq!(hints[0].position.character, 22);
    }
}

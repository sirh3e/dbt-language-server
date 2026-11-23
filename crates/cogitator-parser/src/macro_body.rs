//! What a macro's body and a macro call look like to the call checks
//! (FEATURE_REQUESTS: macro call checks, dispatch implementations):
//! whether a body reads `varargs` / `kwargs`, the `adapter.dispatch(…)`
//! it makes, and a call's argument shape. Pure over minijinja's
//! tokenizer and expression parser — no hand-written lexing.

use minijinja::machinery::{ast, parse_expr, tokenize, Span, Token as JToken};
use minijinja::syntax::SyntaxConfig;

/// Facts of one macro body the call checks need.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MacroBody {
    /// The body reads `varargs`: extra positional arguments are taken.
    pub reads_varargs: bool,
    /// The body reads `kwargs`: unknown keyword arguments are taken.
    pub reads_kwargs: bool,
    /// The first `adapter.dispatch('name'[, 'namespace'])` of the body.
    pub dispatch: Option<DispatchCall>,
}

/// `adapter.dispatch('name', 'namespace')` with literal arguments.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DispatchCall {
    pub name: String,
    pub namespace: Option<String>,
}

/// One `adapter.dispatch(…)` site of a document, with byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchSite {
    pub call: DispatchCall,
    /// The `adapter` token's start to the closing `)`'s end.
    pub start: u32,
    pub end: u32,
}

pub(crate) type JTokens<'a> = [(JToken<'a>, Span)];

fn str_of<'a>(t: &'a JToken<'_>) -> Option<&'a str> {
    match t {
        JToken::Str(s) => Some(s),
        JToken::String(s) => Some(s),
        _ => None,
    }
}

/// The dispatch call starting at token `i` (`adapter`), if any.
pub(crate) fn dispatch_at(tokens: &JTokens<'_>, i: usize) -> Option<(DispatchCall, u32, u32)> {
    let at = |j: usize| tokens.get(j).map(|(t, _)| t);
    if !matches!(at(i), Some(JToken::Ident("adapter")))
        || !matches!(at(i + 1), Some(JToken::Dot))
        || !matches!(at(i + 2), Some(JToken::Ident("dispatch")))
        || !matches!(at(i + 3), Some(JToken::ParenOpen))
    {
        return None;
    }
    let name = at(i + 4).and_then(str_of)?.to_string();
    let (namespace, close) = match at(i + 5) {
        Some(JToken::ParenClose) => (None, i + 5),
        Some(JToken::Comma) => match (at(i + 6), at(i + 7)) {
            (Some(ns), Some(JToken::ParenClose)) => (str_of(ns).map(str::to_string), i + 7),
            // `macro_namespace='x'` keyword form
            (Some(JToken::Ident("macro_namespace")), Some(JToken::Assign)) => {
                let ns = at(i + 8).and_then(str_of).map(str::to_string);
                (ns, i + 9)
            }
            _ => return None,
        },
        _ => return None,
    };
    if !matches!(at(close), Some(JToken::ParenClose)) {
        return None;
    }
    let start = tokens[i].1.start_offset;
    let end = tokens[close].1.end_offset;
    Some((DispatchCall { name, namespace }, start, end))
}

/// Body facts from the token after a macro header's `BlockEnd` up to its
/// own `{% endmacro %}` (nested macro blocks are skipped over).
pub(crate) fn body_facts(tokens: &JTokens<'_>, from: usize) -> MacroBody {
    let mut out = MacroBody::default();
    let mut depth = 0i32;
    let mut i = from;
    while i < tokens.len() {
        let block = i > 0 && matches!(tokens[i - 1].0, JToken::BlockStart);
        match &tokens[i].0 {
            JToken::Ident("macro") if block => depth += 1,
            JToken::Ident("endmacro") if block => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            JToken::Ident("varargs") if depth == 0 => out.reads_varargs = true,
            JToken::Ident("kwargs") if depth == 0 => out.reads_kwargs = true,
            JToken::Ident("adapter") if depth == 0 && out.dispatch.is_none() => {
                out.dispatch = dispatch_at(tokens, i).map(|(call, _, _)| call);
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// Every `adapter.dispatch(…)` of a document; empty when it does not
/// tokenize.
pub fn dispatch_sites(text: &str) -> Vec<DispatchSite> {
    let Ok(tokens) = tokenize(text, false, SyntaxConfig::default()).collect::<Result<Vec<_>, _>>()
    else {
        return Vec::new();
    };
    (0..tokens.len())
        .filter_map(|i| dispatch_at(&tokens, i))
        .map(|(call, start, end)| DispatchSite { call, start, end })
        .collect()
}

/// A call's arguments as minijinja's parser reads them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallShape {
    pub positional: usize,
    /// Keyword names with their byte offset into the parsed source.
    pub kwargs: Vec<(String, u32)>,
    /// `*args` / `**kwargs` splats: the count is unknowable.
    pub splat: bool,
}

/// Parse `source` (`name(…)` or `pkg.name(…)`, nothing after it) as one
/// call expression; `None` when it is not one or does not parse.
pub fn call_shape(source: &str) -> Option<CallShape> {
    let expr = parse_expr(source).ok()?;
    let ast::Expr::Call(call) = expr else {
        return None;
    };
    let mut shape = CallShape::default();
    for arg in &call.args {
        match arg {
            ast::CallArg::Pos(_) => shape.positional += 1,
            ast::CallArg::Kwarg(name, _) => {
                // the name borrows the source: its offset is exact
                let offset = (name.as_ptr() as usize).checked_sub(source.as_ptr() as usize)?;
                shape.kwargs.push(((*name).to_string(), offset as u32));
            }
            ast::CallArg::PosSplat(_) | ast::CallArg::KwargSplat(_) => shape.splat = true,
        }
    }
    Some(shape)
}

#[cfg(test)]
#[path = "macro_body_tests.rs"]
mod tests;

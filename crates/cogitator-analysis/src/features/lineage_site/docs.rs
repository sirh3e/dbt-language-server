//! `{{ doc('…') }}` in descriptions, resolved against the docs blocks.

use crate::DbtContext;

/// `{{ doc('name') }}` replaced by the docs block's text.
pub(super) fn resolve_docs(ctx: &DbtContext, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open..].find("}}") else {
            break;
        };
        let inner = rest[open + 2..open + close].trim();
        out.push_str(&rest[..open]);
        match doc_name(inner).and_then(|n| ctx.doc_detail_map.get(n)) {
            Some(doc) => out.push_str(doc.content.trim()),
            None => out.push_str(&rest[open..open + close + 2]),
        }
        rest = &rest[open + close + 2..];
    }
    out.push_str(rest);
    out
}

fn doc_name(call: &str) -> Option<&str> {
    let args = call.strip_prefix("doc")?.trim_start().strip_prefix('(')?;
    let args = args.trim_end().strip_suffix(')')?.trim();
    let quote = args.chars().next().filter(|q| *q == '\'' || *q == '"')?;
    let name = args.strip_prefix(quote)?.strip_suffix(quote)?;
    (!name.contains(quote)).then_some(name)
}

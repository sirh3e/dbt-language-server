//! Fixes for `private-ref`, at a `ref()` into another group's private
//! model: either widen the target ("Make 'x' protected", in x's
//! properties YAML) or move the referencing model into the target's
//! group ("Join group 'g'", in its own entry). Both change who may
//! reference what, so neither is preferred: which is right is a
//! decision about the project, not a fix of a typo.

use std::path::Path;

use cogitator_lsp::{
    CodeAction, DocumentChange, Position, Range, TextDocumentEdit, TextEdit, WorkspaceEdit,
};
use cogitator_parser::TokenKind;

use super::super::{model_name_from_uri, DbtContext, Document};
use super::yaml_text::{entry_at, insert_after_line, plain_name};
use crate::context::ModelDetails;

pub fn private_ref_actions(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    range: Range,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Vec<CodeAction> {
    let parsed = &doc.parsed;
    let Some(idx) = parsed.find_token_at_cursor(range.start.line, range.start.character) else {
        return Vec::new();
    };
    let token = &parsed.tokens[idx];
    if token.kind != TokenKind::Ref {
        return Vec::new();
    }
    let name = parsed.literal(token);
    let Some(target) = ctx.model_detail_map.get(name) else {
        return Vec::new();
    };
    let Some(group) = target.group.as_deref() else {
        return Vec::new();
    };
    let own = model_name_from_uri(uri).and_then(|m| ctx.model_detail_map.get(&m).map(|d| (m, d)));
    if target.access.as_deref() != Some("private")
        || own.as_ref().and_then(|(_, d)| d.group.as_deref()) == Some(group)
    {
        return Vec::new();
    }
    let mut out = Vec::new();
    out.extend(protect_action(name, target, read));
    if let Some((model, details)) = own {
        out.extend(join_group_action(&model, details, group, read));
    }
    out
}

/// `access: private` → `access: protected` in the target's entry.
fn protect_action(
    name: &str,
    target: &ModelDetails,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<CodeAction> {
    let schema = target.schema_uri.as_deref()?;
    let text = read(schema)?;
    let range = value_range(
        &text,
        target.schema_range.start.line as usize,
        "access",
        "private",
    )?;
    Some(action(
        format!("Make '{name}' protected"),
        schema,
        TextEdit {
            range,
            new_text: "protected".to_string(),
        },
    ))
}

/// `group: <group>` on the referencing model's entry: its group replaced
/// where it has one (an entry key or under `config:`), added otherwise.
fn join_group_action(
    model: &str,
    details: &ModelDetails,
    group: &str,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<CodeAction> {
    let group = plain_name(group)?;
    let schema = details.schema_uri.as_deref()?;
    let text = read(schema)?;
    let line = details.schema_range.start.line as usize;
    let edit = match value_range(&text, line, "group", "") {
        Some(range) => TextEdit {
            range,
            new_text: group.to_string(),
        },
        None => {
            let lines: Vec<&str> = text.lines().collect();
            let entry = entry_at(&lines, line)?;
            let pad = " ".repeat(entry.key_indent);
            insert_after_line(&text, line, format!("{pad}group: {group}\n"))
        }
    };
    Some(action(
        format!("Move '{model}' into group '{group}'"),
        schema,
        edit,
    ))
}

/// The value of `key:` anywhere in the entry holding `line` (its own
/// keys or `config:`'s), when it reads `expected` (any value for "").
fn value_range(text: &str, line: usize, key: &str, expected: &str) -> Option<Range> {
    let lines: Vec<&str> = text.lines().collect();
    let entry = entry_at(&lines, line)?;
    let prefix = format!("{key}:");
    (entry.dash_line..=entry.end).find_map(|i| {
        let raw = lines[i];
        let trimmed = raw.trim_start().trim_start_matches("- ");
        let rest = trimmed.strip_prefix(&prefix)?;
        let value = rest.split('#').next().unwrap_or("").trim();
        let value = value.trim_matches(|c| c == '"' || c == '\'');
        if value.is_empty() || (!expected.is_empty() && value != expected) {
            return None;
        }
        let after_key = raw.find(&prefix)? + prefix.len();
        let at = after_key + raw[after_key..].find(value)?;
        let start: u32 = raw[..at].encode_utf16().count() as u32;
        let len = value.encode_utf16().count() as u32;
        Some(Range {
            start: Position {
                line: i as u32,
                character: start,
            },
            end: Position {
                line: i as u32,
                character: start + len,
            },
        })
    })
}

fn action(title: String, schema: &Path, edit: TextEdit) -> CodeAction {
    CodeAction {
        title,
        kind: Some("quickfix"),
        diagnostics: None,
        edit: Some(WorkspaceEdit {
            document_changes: vec![DocumentChange::Edit(TextDocumentEdit::new(
                cogitator_lsp::fs_path_to_uri(schema),
                vec![edit],
            ))],
        }),
        command: None,
        is_preferred: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "models:\n  - name: a\n    access: private\n    group: finance\n  - name: b\n    config:\n      group: \"ops\"\n  - name: c\n";

    #[test]
    fn finds_a_value_among_the_entrys_own_and_config_keys() {
        let r = value_range(YAML, 1, "access", "private").unwrap();
        assert_eq!(
            (r.start.line, r.start.character, r.end.character),
            (2, 12, 19)
        );
        assert!(value_range(YAML, 1, "access", "protected").is_none());
        let r = value_range(YAML, 4, "group", "").unwrap();
        assert_eq!(
            (r.start.line, r.start.character, r.end.character),
            (6, 14, 17)
        );
        assert!(value_range(YAML, 7, "group", "").is_none(), "c has none");
    }
}

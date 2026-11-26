//! What a client's completion supports (`textDocument.completion.
//! completionItem` capabilities) and the one place items are fitted to
//! it: snippets become plain text, `labelDetails` fold into `detail`,
//! Markdown documentation is sent as plain text.

use serde::Serialize;

use super::CompletionItem;

/// `insertTextFormat` values.
pub mod insert_text_format {
    pub const PLAIN: u8 = 1;
    pub const SNIPPET: u8 = 2;
}

/// LSP 3.17 `CompletionItemLabelDetails`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct LabelDetails {
    /// Right after the label, no space added (` bigint`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Further right, less prominent (the relation).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// The client's completion capabilities the server reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CompletionSupport {
    pub snippets: bool,
    pub label_details: bool,
    pub markdown: bool,
}

impl CompletionSupport {
    /// Everything on: for tests and callers that adapt later.
    pub const ALL: CompletionSupport = CompletionSupport {
        snippets: true,
        label_details: true,
        markdown: true,
    };

    /// Read from `initialize`'s `capabilities` value.
    pub fn from_capabilities(capabilities: Option<&serde_json::Value>) -> Self {
        let item = capabilities.and_then(|c| c.pointer("/textDocument/completion/completionItem"));
        let flag = |key: &str| {
            item.and_then(|i| i.get(key))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        };
        let markdown = item
            .and_then(|i| i.get("documentationFormat"))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|formats| formats.iter().any(|f| f == "markdown"));
        CompletionSupport {
            snippets: flag("snippetSupport"),
            label_details: flag("labelDetailsSupport"),
            markdown,
        }
    }
}

/// Fits `item` to what the client supports.
pub fn adapt(mut item: CompletionItem, support: CompletionSupport) -> CompletionItem {
    if item.insert_text_format == Some(insert_text_format::SNIPPET) && !support.snippets {
        item.insert_text = plain_snippet(&item.insert_text);
        item.insert_text_format = None;
    }
    if !support.label_details {
        // the type and where it is from, in `detail`; a bare description
        // (no type) leaves the item's own detail
        if let Some(details) = item.label_details.take() {
            if let Some(ty) = details
                .detail
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty())
            {
                item.detail = match details.description {
                    Some(description) => format!("{ty} · {description}"),
                    None => ty,
                };
            }
        }
    }
    if !support.markdown {
        item.markdown = false;
    }
    item
}

/// A snippet as the text it inserts with every placeholder at its
/// default: `${1:id}` → `id`, `$1` / `$0` → nothing, `\$` → `$`.
pub fn plain_snippet(snippet: &str) -> String {
    let mut out = String::with_capacity(snippet.len());
    let mut chars = snippet.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            }
            '$' if chars.peek() == Some(&'{') => {
                chars.next();
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    chars.next();
                }
                if chars.peek() == Some(&':') {
                    chars.next();
                }
                let mut depth = 1;
                for inner in chars.by_ref() {
                    match inner {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    if depth > 0 {
                        out.push(inner);
                    }
                }
            }
            '$' if chars.peek().is_some_and(char::is_ascii_digit) => {
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    chars.next();
                }
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_fall_back_to_their_defaults() {
        assert_eq!(
            plain_snippet("where ${1:updated_at} > (select max(${1:updated_at}) from x)$0"),
            "where updated_at > (select max(updated_at) from x)"
        );
        assert_eq!(plain_snippet("a \\$ b $2"), "a $ b ");
    }

    #[test]
    fn capabilities_are_read() {
        let caps = serde_json::json!({"textDocument": {"completion": {"completionItem": {
            "snippetSupport": true, "labelDetailsSupport": true,
            "documentationFormat": ["markdown", "plaintext"]}}}});
        assert_eq!(
            CompletionSupport::from_capabilities(Some(&caps)),
            CompletionSupport::ALL
        );
        assert_eq!(
            CompletionSupport::from_capabilities(None),
            CompletionSupport::default()
        );
    }

    #[test]
    fn an_item_degrades_to_plain_text() {
        let item = CompletionItem {
            label: "id".into(),
            insert_text: "${1:id}".into(),
            insert_text_format: Some(insert_text_format::SNIPPET),
            label_details: Some(LabelDetails {
                detail: Some(" bigint".into()),
                description: Some("stg_orders".into()),
            }),
            markdown: true,
            ..Default::default()
        };
        let plain = adapt(item.clone(), CompletionSupport::default());
        assert_eq!(plain.insert_text, "id");
        assert_eq!(plain.insert_text_format, None);
        assert_eq!(plain.detail, "bigint · stg_orders");
        let json = serde_json::to_value(&plain).unwrap();
        assert!(json["documentation"].is_string(), "{json}");
        assert!(json.get("labelDetails").is_none(), "{json}");
        let rich = serde_json::to_value(adapt(item, CompletionSupport::ALL)).unwrap();
        assert_eq!(rich["insertTextFormat"], 2);
        assert_eq!(rich["labelDetails"]["detail"], " bigint");
        assert_eq!(rich["documentation"]["kind"], "markdown");
    }
}

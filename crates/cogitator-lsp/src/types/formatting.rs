//! textDocument/formatting wire types (01 §6.3).

use serde::Deserialize;

use super::params::TextDocumentIdentifier;

#[derive(Debug, Deserialize)]
pub struct DocumentFormattingParams<'a> {
    #[serde(borrow, rename = "textDocument")]
    pub text_document: TextDocumentIdentifier<'a>,
    #[serde(default)]
    pub options: FormattingOptions,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormattingOptions {
    pub tab_size: u8,
    pub insert_spaces: bool,
}

impl Default for FormattingOptions {
    fn default() -> Self {
        FormattingOptions {
            tab_size: 4,
            insert_spaces: true,
        }
    }
}

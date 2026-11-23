//! textDocument/implementation: the implementations of a dispatched
//! macro (`features::dispatch`). Pure over the context snapshot and the
//! open document, so it answers inline like signatureHelp.

use cogitator_analysis::features;

use super::Dispatcher;

impl Dispatcher {
    pub(super) fn implementation(&mut self, content: &[u8]) {
        self.respond_position(
            content,
            "textDocument/implementation",
            |ctx, doc, _, pos| {
                doc.map(|d| features::implementation(ctx, d, pos))
                    .unwrap_or_default()
            },
        );
    }
}

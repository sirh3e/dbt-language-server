//! Hover and definition on compiled SQL files, offloaded (they read the
//! source and the compiled file): `server/compiled_source.rs`.

use cogitator_analysis::features;
use cogitator_lsp::{Location, Position, Range};

use super::Dispatcher;
use crate::server::compiled_source;
use crate::server::reply::Reply;
use crate::server::snapshot::Snapshot;

impl Dispatcher {
    /// textDocument/hover on a compiled file; `false` when the document
    /// is not one (the caller answers as usual).
    pub(super) fn compiled_hover(&mut self, content: &[u8]) -> bool {
        let Some((id, uri, pos)) = Self::position_request(content) else {
            return false;
        };
        if compiled_source::source_of(&self.ctx_for(&uri), &uri).is_none() {
            return false;
        }
        let snap = self.snapshot_for(&uri);
        self.offload_blocking(id, "textDocument/hover", move || {
            compiled_source::hover(&snap, &uri, pos)
        });
        true
    }

    /// textDocument/definition on a compiled file: a real SQL target (a
    /// CTE of the file) wins, else the source line the compiled one came
    /// from.
    pub(super) fn compiled_definition(
        &mut self,
        id: i64,
        uri: String,
        pos: Position,
        snap: Snapshot,
    ) {
        self.offload_blocking(id, "textDocument/definition", move || {
            let fallback = Location {
                uri: uri.clone(),
                range: Range::zero_width(pos),
            };
            let sql = snap
                .doc
                .as_deref()
                .map(|doc| features::definition(&snap.ctx, doc, &uri, pos))
                .filter(|l| *l != fallback);
            let location = sql
                .or_else(|| compiled_source::definition(&snap, &uri, pos))
                .unwrap_or(fallback);
            Reply::result(&location)
        });
    }
}

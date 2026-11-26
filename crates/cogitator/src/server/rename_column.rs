//! `dbt.renameColumn {uri, position, newName, direction?, apply?}`: the
//! column under the cursor renamed downstream (`"down"`, the default, what
//! textDocument/rename does) or up- and downstream (`"both"`: from the
//! model it is defined in). Answers the rename — origin, chain, renamed
//! models and the WorkspaceEdit — for the client to preview; with
//! `apply: true` the server also sends the edit as `workspace/applyEdit`.
//! The analysis is `features::rename_column_directed`; this shell reads
//! files and builds the reply.

use cogitator_analysis::{features, Document};
use cogitator_lsp::{error_codes, ApplyWorkspaceEditParams, Position};
use serde_json::{Map, Value};

use crate::server::reply::{Effect, Reply};
use crate::server::snapshot::Snapshot;

fn refuse(what: impl Into<String>, fix: &str) -> Reply {
    Reply::error(
        error_codes::INVALID_PARAMS,
        cogitator_error::guidance::Guidance::new(what.into())
            .fix(fix)
            .to_message(),
    )
}

pub fn reply(
    snap: &Snapshot,
    uri: &str,
    position: Option<Position>,
    arg: &Map<String, Value>,
) -> Reply {
    let Some(position) = position else {
        return refuse(
            "dbt.renameColumn needs a position.",
            "pass {\"uri\": …, \"position\": {\"line\": 0, \"character\": 0}, \"newName\": …}.",
        );
    };
    let Some(new_name) = arg.get("newName").and_then(Value::as_str).map(str::trim) else {
        return refuse(
            "dbt.renameColumn needs a newName.",
            "pass the column's new name as \"newName\".",
        );
    };
    let word = arg
        .get("direction")
        .and_then(Value::as_str)
        .unwrap_or("down");
    let Some(direction) = features::ColumnRenameDirection::parse(word) else {
        return refuse(
            format!("dbt.renameColumn does not know the direction '{word}'."),
            "pass \"down\" (the column and its consumers) or \"both\" (from where it is defined).",
        );
    };
    let disk = snap
        .doc
        .is_none()
        .then(|| Document::from_disk(uri, &snap.ctx.dialect))
        .flatten();
    let doc = snap.doc.as_deref().or(disk.as_ref());
    let read = |p: &std::path::Path| std::fs::read_to_string(p).ok();
    let answer =
        features::rename_column_directed(&snap.ctx, doc, uri, position, new_name, direction, &read);
    match answer {
        None => Reply::null(),
        Some(Err(reason)) => Reply::error(error_codes::REQUEST_FAILED, reason),
        Some(Ok(rename)) => {
            let reply = Reply::result(&rename);
            if arg.get("apply").and_then(Value::as_bool) != Some(true) {
                return reply;
            }
            let label = format!(
                "Rename column {}.{} → {} ({})",
                rename.origin.model,
                rename.origin.column,
                rename.new_name,
                match direction {
                    features::ColumnRenameDirection::Down => "downstream",
                    features::ColumnRenameDirection::Both => "up- and downstream",
                },
            );
            reply.with(Effect::ApplyEdit(ApplyWorkspaceEditParams {
                label,
                edit: rename.edit,
            }))
        }
    }
}

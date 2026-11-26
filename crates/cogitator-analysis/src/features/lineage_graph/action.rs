//! The code action that opens a column's lineage graph as a file, for
//! clients that run code actions but cannot wrap a command (Zed): on a
//! model's output column, "Show column lineage of `m.col`" runs
//! `dbt.columnLineageGraph {uri, column, show: true}`, which writes the
//! walk as text ([`super::text`]) and opens it through showDocument.

use cogitator_lsp::{CodeAction, Command, Position};

use super::super::rename_column::column_at;
use super::super::{file_uri, DbtContext, Document};

/// The command and argument the action sends (pinned by the e2e tests).
pub const SHOW_COMMAND: &str = "dbt.columnLineageGraph";

/// "Show column lineage of `model.column`" when the cursor names a
/// model's output column (in its SQL or its YAML entry); `None` elsewhere.
pub fn show_lineage_action(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    pos: Position,
) -> Option<CodeAction> {
    let (model, column, _) = column_at(ctx, doc, uri, pos)?;
    let details = ctx.model_detail_map.get(&model)?;
    if details.uri.as_os_str().is_empty() {
        return None;
    }
    let title = format!("Show column lineage of {model}.{column}");
    let argument = serde_json::json!({
        "uri": file_uri(&details.uri),
        "column": column,
        "show": true,
    });
    Some(CodeAction::command_only(
        title.clone(),
        Command {
            title,
            command: SHOW_COMMAND.to_string(),
            arguments: vec![argument],
        },
    ))
}

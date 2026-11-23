//! dbt.previewCte: the rows of one CTE (or the final select) of a model,
//! the model run only up to that CTE. Pure halves only — the shell picks
//! the SQL source (`source`), runs `dbt show --inline` and hands the
//! stdout back:
//!
//! - `query`: the compiled (or rendered) text cut at a CTE with
//!   sqlparser — never by slicing text — and held to one SELECT;
//! - `show`: `dbt show --output json` stdout → columns and rows;
//! - `render`: the markdown table editors without a command UI open;
//! - `locate`: the "Preview rows" lenses and the CTE under a cursor.

mod locate;
mod query;
mod render;
mod show;
mod source;

pub use locate::{cte_at, cte_preview_lenses};
pub use query::preview_query;
pub use render::{preview_file_name, render_markdown};
pub use show::{parse_show, ShowTable};
pub use source::{choose_source, PreviewSource};

use serde::Serialize;

/// The dbt.previewCte answer.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CtePreview {
    pub model: String,
    /// The CTE previewed; `None` is the model's final select.
    pub cte: Option<String>,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    /// More rows existed than `limit`.
    pub truncated: bool,
    pub limit: usize,
    /// The query dbt ran.
    pub sql: String,
    /// `compiled` (target/compiled, fresh) or `rendered` (cogitator's own
    /// Jinja rendering, dbt.expandJinja's renderer).
    pub source: PreviewSource,
    /// The markdown table the server wrote and opened, when it could.
    pub uri: Option<String>,
    /// The same table as markdown, for editors that show text.
    pub table: String,
}

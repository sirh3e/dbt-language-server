//! dbt.compiledSql: the compiled-SQL coordinates for a model — the pure
//! half (name + expected path resolution); the shell reads the file and
//! enforces the wire cap.

use std::path::PathBuf;

use serde::Serialize;

use super::model_info::compiled_sql_path;
use super::tests_nav::resolve_model_name;
use super::{model_name_from_uri, DbtContext};

/// The dbt.compiledSql result. `uri` and `content` are explicit `null`
/// when dbt has not written a compiled file (or it is over the wire cap).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompiledSql {
    pub model: String,
    pub uri: Option<String>,
    pub content: Option<String>,
}

/// The model (named, or the file's own — `model_arg` wins, like
/// dbt.modelInfo) and the path dbt compiles it to.
pub fn compiled_target(
    ctx: &DbtContext,
    uri: &str,
    model_arg: Option<&str>,
) -> Option<(String, PathBuf)> {
    let requested = match model_arg {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => model_name_from_uri(uri)?,
    };
    let name = if ctx.model_detail_map.contains_key(&requested) {
        requested
    } else {
        resolve_model_name(ctx, &requested)?
    };
    let details = ctx.model_detail_map.get(&name)?;
    let root = ctx.project_root.as_deref()?;
    let path = compiled_sql_path(root, &ctx.project.name, &details.uri);
    Some((name, path))
}

//! Which models a sync covers, and where a model without an entry gets
//! one.

use std::path::PathBuf;

use super::{NewEntries, SyncOptions};
use crate::context::ModelKind;
use crate::DbtContext;

/// The models a sync covers: the root project's SQL models, by name.
pub(super) fn selected<'c>(
    ctx: &'c DbtContext,
    options: &SyncOptions,
) -> Vec<(String, Option<&'c crate::ModelDetails>)> {
    let own = |d: &crate::ModelDetails| {
        d.kind == ModelKind::Model
            && d.project_name == ctx.project.name
            && d.uri.extension().is_some_and(|e| e == "sql")
    };
    if options.models.is_empty() {
        let mut names: Vec<(String, Option<&crate::ModelDetails>)> = ctx
            .model_detail_map
            .iter()
            .filter(|(_, d)| own(d))
            .map(|(n, d)| (n.clone(), Some(d)))
            .collect();
        names.sort_by(|a, b| a.0.cmp(&b.0));
        return names;
    }
    options
        .models
        .iter()
        .map(|n| (n.clone(), ctx.model_detail_map.get(n).filter(|d| own(d))))
        .collect()
}

/// Where a model without an entry gets one.
pub(super) fn new_entry_path(
    details: &crate::ModelDetails,
    model: &str,
    layout: NewEntries,
) -> PathBuf {
    match layout {
        NewEntries::PerModel => details.uri.with_file_name(format!("{model}.yml")),
        NewEntries::PerFolder => {
            let folder = details
                .uri
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "models".to_string());
            details.uri.with_file_name(format!("_{folder}__models.yml"))
        }
    }
}

/// `<base>_v<N>` without an entry of its own: a version of `base`.
pub(super) fn version_of<'c>(
    ctx: &'c DbtContext,
    model: &'c str,
    details: &crate::ModelDetails,
) -> Option<&'c str> {
    if details.schema_uri.is_some() {
        return None;
    }
    let (base, v) = model.rsplit_once("_v")?;
    let versioned = !v.is_empty()
        && v.chars().all(|c| c.is_ascii_digit())
        && ctx
            .model_detail_map
            .get(base)
            .is_some_and(|d| d.schema_uri.is_some());
    versioned.then_some(base)
}

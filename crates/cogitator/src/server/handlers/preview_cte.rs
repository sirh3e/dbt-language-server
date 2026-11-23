//! dbt.previewCte's preparation, on a blocking thread: which model and
//! CTE, which SQL (dbt's compiled file while it is current, else the
//! server's own Jinja rendering) and the read-only query cut from it.
//! The run itself is `server::preview_cte`.

use std::path::PathBuf;
use std::time::SystemTime;

use cogitator_analysis::features::{self, PreviewSource};
use cogitator_error::guidance::{closest_matches, Guidance};
use cogitator_lsp::Position;

use super::super::derived_cache::DerivedCache;
use super::super::snapshot::Snapshot;

/// What the request asked for.
pub struct PreviewAsk {
    pub uri: String,
    pub cte: Option<String>,
    pub position: Option<Position>,
    /// Rows to show; the query asks for one more to know of a cut.
    pub limit: usize,
}

/// Ready to run: the model, the CTE and the query.
#[derive(Debug)]
pub struct Prepared {
    pub model: String,
    pub cte: Option<String>,
    pub sql: String,
    pub source: PreviewSource,
    pub project_root: PathBuf,
}

#[tracing::instrument(skip_all, fields(cte = ask.cte.as_deref().unwrap_or("")))]
pub fn prepare(
    snap: &Snapshot,
    derived: &DerivedCache,
    ask: &PreviewAsk,
) -> Result<Prepared, Guidance> {
    let ctx = &snap.ctx;
    let model = features::model_name_from_uri(&ask.uri)
        .filter(|m| ctx.model_detail_map.contains_key(m))
        .ok_or_else(|| {
            Guidance::new("dbt.previewCte needs a model's .sql file.")
                .at(ask.uri.clone())
                .suggest(closest_matches(
                    &features::model_name_from_uri(&ask.uri).unwrap_or_default(),
                    ctx.model_detail_map.keys().map(String::as_str),
                ))
                .fix("pass the uri of a model the project knows.")
        })?;
    let project_root = ctx.project_root.clone().ok_or_else(|| {
        Guidance::new("Cannot preview: no dbt project found.")
            .fix("open the directory holding dbt_project.yml.")
    })?;
    let cte = ask.cte.clone().filter(|c| !c.is_empty()).or_else(|| {
        let doc = snap.doc.as_deref()?;
        features::cte_at(&doc.parsed, ask.position?)
    });

    let model_path = ctx.model_detail_map[&model].uri.clone();
    let disk = std::fs::read_to_string(&model_path).ok();
    let buffer_matches_disk = snap
        .doc
        .as_deref()
        .is_none_or(|doc| disk.as_deref() == Some(&*doc.text));
    let compiled = features::compiled_target(ctx, &ask.uri, None).map(|(_, path)| path);
    let source = features::choose_source(
        compiled.as_deref().and_then(mtime),
        mtime(&model_path),
        buffer_matches_disk,
    );
    let text = match (&source, &compiled) {
        (PreviewSource::Compiled, Some(path)) => read_compiled(path, snap.compiled_cap_bytes)?,
        _ => render(snap, derived, &model, disk)?,
    };
    let sql = features::preview_query(&text, &ctx.dialect, cte.as_deref(), ask.limit + 1)?;
    Ok(Prepared {
        model,
        cte,
        sql,
        source,
        project_root,
    })
}

fn mtime(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn read_compiled(path: &std::path::Path, cap: u64) -> Result<String, Guidance> {
    let too_big = std::fs::metadata(path).is_ok_and(|m| m.len() > cap);
    if too_big {
        return Err(Guidance::new("The compiled SQL is too large to preview.")
            .at(path.display().to_string())
            .fix("raise limits.compiled_sql_mib in cogitator.yml."));
    }
    std::fs::read_to_string(path).map_err(|err| {
        Guidance::new("The compiled SQL could not be read.")
            .at(path.display().to_string())
            .because(format!("{err}."))
            .fix("run dbt compile for the model again.")
    })
}

/// The open buffer (or the file) rendered by dbt.expandJinja's renderer.
fn render(
    snap: &Snapshot,
    derived: &DerivedCache,
    model: &str,
    disk: Option<String>,
) -> Result<String, Guidance> {
    let text = match snap.doc.as_deref() {
        Some(doc) => doc.text.to_string(),
        None => disk.ok_or_else(|| {
            Guidance::new(format!("Could not read model '{model}'."))
                .fix("open the model in the editor, or check the uri.")
        })?,
    };
    let mut inputs = super::commands::preview_inputs(snap, derived);
    inputs.this = inputs.model_relations.get(model).cloned();
    Ok(features::expand_jinja(&text, &inputs)?.sql)
}

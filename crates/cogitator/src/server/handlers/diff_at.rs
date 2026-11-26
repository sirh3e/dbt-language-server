//! `dbt.diffAt {uri, rev?, what?, show?}`: the model now against itself
//! at a git revision (default `HEAD`) or as of the last dbt run
//! (`rev: "last-run"`: manifest.json's raw_code, and for `compiled` the
//! file under target/compiled/ or the code dbt recorded). `show: true`
//! writes the diff under `target/.cogitator/diff/` and opens it.

use std::path::{Path, PathBuf};

use cogitator_analysis::artifacts::manifest_code::{model_code, ModelCode};
use cogitator_analysis::artifacts::ArtifactPaths;
use cogitator_analysis::features;
use cogitator_analysis::features::diff_at::{self, DiffRequest, DiffWhat, OldSide, LAST_RUN};
use cogitator_error::guidance::Guidance;
use cogitator_lsp::{error_codes, ShowDocumentParams};

use super::super::derived_cache::DerivedCache;
use super::super::git;
use super::super::reply::{Effect, Reply};
use super::super::snapshot::Snapshot;
use super::navigation::short_uri;

pub struct DiffAsk {
    pub uri: String,
    pub rev: Option<String>,
    pub what: Option<String>,
    pub show: bool,
}

#[tracing::instrument(skip_all, fields(uri = %short_uri(&ask.uri), rev = ?ask.rev))]
pub fn diff_at(snap: &Snapshot, derived: &DerivedCache, ask: &DiffAsk, artifact_cap: u64) -> Reply {
    match compute(snap, derived, ask, artifact_cap) {
        Ok((diff, root)) => {
            let reply = Reply::result(&diff);
            match (ask.show, root.and_then(|root| write(&root, &diff))) {
                (true, Some(uri)) => reply.with(Effect::ShowDocument(ShowDocumentParams {
                    uri,
                    take_focus: true,
                    selection: None,
                })),
                _ => reply,
            }
        }
        Err(guidance) => Reply::error(error_codes::INVALID_PARAMS, guidance.to_message()),
    }
}

fn compute(
    snap: &Snapshot,
    derived: &DerivedCache,
    ask: &DiffAsk,
    artifact_cap: u64,
) -> Result<(diff_at::DiffAt, Option<PathBuf>), Guidance> {
    let what = DiffWhat::parse(ask.what.as_deref())?;
    let model = features::model_name_from_uri(&ask.uri).ok_or_else(|| {
        Guidance::new("dbt.diffAt needs a model's .sql file.")
            .at(ask.uri.clone())
            .fix("pass the uri of a dbt model.")
    })?;
    let path =
        PathBuf::from(cogitator_lsp::uri_to_fs_path(&ask.uri).unwrap_or_else(|| ask.uri.clone()));
    let current = match snap.doc.as_deref() {
        Some(doc) => doc.text.to_string(),
        None => std::fs::read_to_string(&path).map_err(|_| {
            Guidance::new(format!("Could not read model '{model}'."))
                .at(path.display().to_string())
                .fix("open the model in the editor, or check the uri.")
        })?,
    };
    let rev = ask.rev.clone().unwrap_or_else(|| "HEAD".to_string());
    let root = snap.ctx.project_root.clone();
    let (old, label) = if rev == LAST_RUN {
        let root = root.as_deref().ok_or_else(|| {
            Guidance::new("no dbt project is loaded.").fix("open a file inside a dbt project.")
        })?;
        (
            last_run(root, &model, what, derived, artifact_cap)?,
            rev.clone(),
        )
    } else {
        let (text, commit) = git::show(&path, &rev)?;
        let short = &commit.sha[..commit.sha.len().min(10)];
        (OldSide::Template(text), format!("{rev} ({short})"))
    };
    let mut inputs = super::commands::preview_inputs(snap, derived);
    inputs.this = inputs.model_relations.get(&model).cloned();
    let diff = diff_at::diff_at(
        DiffRequest {
            model: &model,
            rev: &label,
            what,
            old,
            current: &current,
            dialect: &snap.ctx.dialect,
        },
        &inputs,
    )?;
    Ok((diff, root))
}

/// The old side as of the last dbt run.
fn last_run(
    root: &Path,
    model: &str,
    what: DiffWhat,
    derived: &DerivedCache,
    artifact_cap: u64,
) -> Result<OldSide, Guidance> {
    let manifest_path = ArtifactPaths::under(root).manifest;
    let code = std::fs::read_to_string(&manifest_path)
        .map_err(|_| {
            Guidance::new("there is no last run to compare with.")
                .at(manifest_path.display().to_string())
                .fix("run dbt (parse, compile, run or build) first.")
        })
        .and_then(|text| model_code(&text, &manifest_path, model))?
        .ok_or_else(|| {
            Guidance::new(format!("'{model}' is not in target/manifest.json."))
                .fix("re-run dbt parse: the model may be newer than the manifest.")
        })?;
    if what != DiffWhat::Compiled {
        return code.raw_code.map(OldSide::Template).ok_or_else(|| {
            Guidance::new(format!("the manifest holds no raw_code for '{model}'."))
                .fix("re-run dbt parse with a dbt that records it (1.3 or later).")
        });
    }
    if let Some(text) = compiled_file(root, &code) {
        return Ok(OldSide::Compiled(text));
    }
    let recorded = derived
        .artifacts(root, artifact_cap)
        .run_results
        .and_then(|rr| {
            rr.results
                .iter()
                .find(|r| r.unique_id == code.unique_id)
                .and_then(|r| r.compiled_code.clone())
        })
        .or(code.compiled_code);
    recorded.map(OldSide::Compiled).ok_or_else(|| {
        Guidance::new(format!("dbt has not compiled '{model}' yet.")).fix(format!(
            "run dbt compile --select {model}, then compare again."
        ))
    })
}

/// `target/compiled/<package>/<original_file_path>`, where dbt writes it.
fn compiled_file(root: &Path, code: &ModelCode) -> Option<String> {
    let package = code.package_name.as_deref()?;
    let file = code.original_file_path.as_deref()?;
    std::fs::read_to_string(root.join("target/compiled").join(package).join(file)).ok()
}

fn write(root: &Path, diff: &diff_at::DiffAt) -> Option<String> {
    let dir = root.join("target/.cogitator/diff");
    let rev: String = diff
        .rev
        .split(' ')
        .next()
        .unwrap_or("rev")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let path = dir.join(format!("{}@{rev}.{}.diff", diff.model, diff.what.as_str()));
    if let Err(err) = std::fs::create_dir_all(&dir)
        .and_then(|()| std::fs::write(&path, diff_at::shown_text(diff)))
    {
        tracing::warn!(%err, "could not write the diff; returning it only");
        return None;
    }
    Some(cogitator_lsp::fs_path_to_uri(&path))
}

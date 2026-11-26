//! dbt.profileColumn's run: prepare (blocking: the column under the
//! cursor, the model's relation, the query), then either DuckDB's
//! read-only connection (a duckdb adapter in a `duckdb-parser` build) or
//! `dbt show --inline` as a cancellable job of the registry. The profile
//! lands in the derived cache, which the hover reads; the request is
//! answered with it. Off unless cogitator.yml `warehouse.profile: true`.

use std::path::PathBuf;
use std::sync::Arc;

use cogitator_analysis::features::{self, profile};
use cogitator_error::guidance::Guidance;
use cogitator_lsp::{error_codes, Position};

mod engines;
mod surface;

pub use surface::{action, hover};

use super::client_link::ClientLink;
use super::config_file::AnalyticsSettings;
use super::derived_cache::DerivedCache;
use super::jobs::DbtJobs;
use super::snapshot::Snapshot;
use engines::{duckdb_file, duckdb_rows, show_rows};

pub struct ProfileJob {
    pub id: i64,
    pub uri: String,
    pub position: Option<Position>,
    pub column: Option<String>,
    pub binary: String,
    pub snap: Snapshot,
    pub derived: Arc<DerivedCache>,
    pub settings: Arc<AnalyticsSettings>,
    pub artifact_cap: u64,
}

struct Prepared {
    key: profile::ProfileKey,
    relation: String,
    sql: String,
    stamp: String,
    project_root: PathBuf,
    /// The duckdb file, when the fast path applies.
    duckdb: Option<PathBuf>,
}

/// The `{profile, hover}` answer.
#[derive(serde::Serialize)]
struct Answer<'a> {
    profile: &'a profile::ColumnProfile,
    hover: String,
}

pub fn spawn(link: ClientLink, jobs: Arc<DbtJobs>, job: ProfileJob) {
    let token = jobs.next_token();
    let label = format!(
        "profile: {}",
        features::model_name_from_uri(&job.uri).unwrap_or_default()
    );
    let span = tracing::info_span!("dbt_profile_column", token);
    use tracing::Instrument as _;
    let id = job.id;
    let answer_link = link.clone();
    let handle = tokio::spawn(
        async move {
            let derived = job.derived.clone();
            let result = run(&link, token, job).await;
            match result {
                Ok(profile) => {
                    let hover = profile::hover_markdown(&profile);
                    answer_link.respond(
                        id,
                        &Answer {
                            profile: &profile,
                            hover,
                        },
                    );
                    derived.store_column_profile(
                        profile::ProfileKey::new(&profile.model, &profile.column),
                        profile,
                    );
                }
                Err(guidance) => answer_link.respond_error(
                    id,
                    error_codes::REQUEST_FAILED,
                    guidance.to_message(),
                ),
            }
        }
        .instrument(span),
    );
    jobs.register(token, label, handle.abort_handle());
}

async fn run(
    link: &ClientLink,
    token: u64,
    job: ProfileJob,
) -> Result<profile::ColumnProfile, Guidance> {
    if !job.settings.profile {
        return Err(Guidance::new("column profiling is off.")
            .because("it runs a query against the warehouse.")
            .fix("set warehouse.profile: true in cogitator.yml to allow it."));
    }
    let profiles = job.snap.profiles.clone();
    let binary = job.binary.clone();
    let prepared = tokio::task::spawn_blocking(move || prepare(&job))
        .await
        .map_err(|err| {
            Guidance::new("the profile could not be prepared.").because(format!("{err}."))
        })??;
    let progress = link.progress_begin(format!(
        "profile: {}.{} [job {token}]",
        prepared.key.model, prepared.key.column
    ));
    let (engine, answer) = match prepared.duckdb.clone() {
        Some(db) => {
            let sql = prepared.sql.clone();
            let answer = tokio::task::spawn_blocking(move || duckdb_rows(&db, &sql))
                .await
                .unwrap_or_else(|err| {
                    Err(Guidance::new("the duckdb profile failed.").because(format!("{err}.")))
                });
            ("duckdb", answer)
        }
        None => ("dbt show", show_rows(&binary, &profiles, &prepared).await),
    };
    let (columns, rows) = match answer {
        Ok(answer) => answer,
        Err(guidance) => {
            progress.end("profile failed");
            return Err(guidance);
        }
    };
    progress.end(format!(
        "profiled {}.{}",
        prepared.key.model, prepared.key.column
    ));
    profile::profile_of(
        &prepared.key,
        &prepared.relation,
        engine,
        &prepared.stamp,
        prepared.sql.clone(),
        &columns,
        &rows,
    )
}

fn prepare(job: &ProfileJob) -> Result<Prepared, Guidance> {
    let snap = &job.snap;
    let ctx = &snap.ctx;
    let model = features::model_name_from_uri(&job.uri)
        .filter(|m| ctx.model_detail_map.contains_key(m))
        .ok_or_else(|| {
            Guidance::new("dbt.profileColumn needs a model's .sql file.")
                .at(job.uri.clone())
                .fix("open a model and put the cursor on one of its output columns.")
        })?;
    let project_root = ctx.project_root.clone().ok_or_else(|| {
        Guidance::new("no dbt project is loaded.").fix("open a file inside a dbt project.")
    })?;
    let doc = match snap.doc.clone() {
        Some(doc) => doc,
        None => Arc::new(
            cogitator_analysis::Document::from_disk(&job.uri, &ctx.dialect).ok_or_else(|| {
                Guidance::new(format!("Could not read model '{model}'.")).at(job.uri.clone())
            })?,
        ),
    };
    let lineage = super::handlers::commands::lineage_for(&job.derived, ctx, Some(&doc), &job.uri);
    let key = match (&job.column, job.position) {
        (Some(name), _) => profile::column_named(ctx, &model, lineage.as_ref().as_ref(), name),
        (None, Some(pos)) => {
            profile::profile_target(ctx, &model, &doc, lineage.as_ref().as_ref(), pos)
        }
        (None, None) => None,
    }
    .ok_or_else(|| {
        Guidance::new("no output column of the model is under the cursor.")
            .because("a profile is of a column the model produces or documents.")
            .fix("put the cursor on a selected column name (or pass column: \"…\").")
    })?;
    let artifacts = job.derived.artifacts(&project_root, job.artifact_cap);
    if artifacts
        .manifest
        .as_deref()
        .and_then(|m| m.models.get(&model))
        .and_then(|n| n.config.materialized.as_deref())
        == Some("ephemeral")
    {
        return Err(Guidance::new(format!(
            "'{model}' is ephemeral: it has no table to profile."
        ))
        .fix("profile a column of a model that dbt materializes."));
    }
    let target = super::handlers::profiles::active_target_relation(snap, &job.derived);
    let info = features::relation_info(ctx, Some(&doc), &job.uri, None, target.as_ref())
        .ok_or_else(|| Guidance::new(format!("'{model}' has no relation.")))?;
    let info = match artifacts.manifest.as_deref() {
        Some(manifest) => features::with_manifest_relation(info, manifest),
        None => info,
    };
    let stamp = artifacts
        .run_results
        .as_deref()
        .and_then(|rr| rr.metadata.generated_at.clone())
        .unwrap_or_default();
    let dialect = ctx.dialect.as_str();
    let sql = profile::profile_sql(
        dialect,
        &info.relation,
        &key.column,
        job.settings.profile_sample_rows,
    );
    let duckdb = (dialect == "duckdb" && cfg!(feature = "duckdb-parser"))
        .then(|| duckdb_file(snap, &job.derived, &project_root))
        .flatten();
    Ok(Prepared {
        key,
        relation: info.relation,
        sql,
        stamp,
        project_root,
        duckdb,
    })
}

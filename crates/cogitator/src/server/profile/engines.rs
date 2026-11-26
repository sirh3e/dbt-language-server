//! The two ways a profile query runs: DuckDB's read-only connection,
//! or `dbt show --inline` through the dbt binary.

use std::path::PathBuf;

use cogitator_analysis::features::{self, profile};
use cogitator_error::guidance::Guidance;
use serde_json::Value;

use super::super::derived_cache::DerivedCache;
use super::super::snapshot::Snapshot;
use super::Prepared;

/// The active duckdb target's database file, relative paths from the root.
pub(super) fn duckdb_file(
    snap: &Snapshot,
    derived: &DerivedCache,
    root: &std::path::Path,
) -> Option<PathBuf> {
    use super::super::handlers::profiles::{active_profile_target, find_profiles_file};
    let path = find_profiles_file(&snap.profiles, &snap.root_path)?;
    let inventory =
        derived.profiles_inventory(&path, &snap.ctx.project.profile, &snap.profiles.allow);
    let (profile, target) =
        active_profile_target(&snap.profiles, &snap.ctx.project.profile, &inventory);
    let profile = inventory.profile(&profile)?;
    let target = profile.targets.iter().find(|t| t.name == target)?;
    let file = PathBuf::from(target.warehouse_path.clone()?);
    let file = if file.is_relative() {
        root.join(file)
    } else {
        file
    };
    file.is_file().then_some(file)
}

pub(super) type Rows = (Vec<String>, Vec<Vec<Value>>);

#[cfg(feature = "duckdb-parser")]
pub(super) fn duckdb_rows(db: &std::path::Path, sql: &str) -> Result<Rows, Guidance> {
    let answer = cogitator_duckdb::catalog::query(db, sql)?;
    let rows = answer
        .rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|c| c.map_or(Value::Null, Value::String))
                .collect()
        })
        .collect();
    Ok((answer.columns, rows))
}

#[cfg(not(feature = "duckdb-parser"))]
pub(super) fn duckdb_rows(_db: &std::path::Path, _sql: &str) -> Result<Rows, Guidance> {
    Err(Guidance::new("this build has no duckdb engine.")
        .fix("rebuild with --features duckdb-parser."))
}

pub(super) async fn show_rows(
    binary: &str,
    profiles: &super::super::ProfilesConfig,
    prepared: &Prepared,
) -> Result<Rows, Guidance> {
    let limit = profile::TOP_VALUES + 1;
    let mut extra = vec![
        "--inline".to_string(),
        prepared.sql.clone(),
        "--limit".to_string(),
        (limit + 1).to_string(),
        "--output".to_string(),
        "json".to_string(),
    ];
    if let Some(vars) = profiles.dbt_vars_arg() {
        extra.extend(["--vars".to_string(), vars]);
    }
    let nonempty = |s: &String| (!s.is_empty()).then(|| s.clone());
    let args = super::super::commands::dbt_args(
        "show",
        None,
        nonempty(&profiles.target).as_deref(),
        nonempty(&profiles.dir_override).as_deref(),
        nonempty(&profiles.profile).as_deref(),
        &extra,
    );
    let output = tokio::process::Command::new(binary)
        .args(&args)
        .current_dir(&prepared.project_root)
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|err| {
            Guidance::new(format!("Failed to launch '{binary}'."))
                .because(format!("{err}."))
                .fix("install dbt, or start cogitator with --fusion <path-to-dbt-binary>.")
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout
        } else {
            stderr
        };
        return Err(Guidance::new(format!(
            "dbt show failed profiling '{}'.",
            prepared.key.column
        ))
        .because(super::super::commands::tail(&detail, 400))
        .fix(format!("the query was: {}", prepared.sql)));
    }
    let table = features::parse_show(&String::from_utf8_lossy(&output.stdout), limit)?;
    Ok((table.columns, table.rows))
}

//! profiles.yml resolution: explicit override dir, dbt-style search order,
//! and fall-through when a candidate has no matching target.

use std::path::{Path, PathBuf};

use cogitator_analysis::util::{get_dialect, profiles_candidates};
use cogitator_analysis::ContextBuilder;
use pretty_assertions::assert_eq;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/jaffle_shop_duckdb")
}

/// A throwaway directory holding one profiles.yml.
fn temp_profiles(name: &str, body: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("cogitator-profiles-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("profiles.yml"), body).unwrap();
    dir
}

const SNOWFLAKE_JAFFLE: &str =
    "jaffle_shop:\n  target: dev\n  outputs:\n    dev:\n      type: snowflake\n";

#[test]
fn override_dir_wins_over_workspace_lookup() {
    let dir = temp_profiles("override", SNOWFLAKE_JAFFLE);
    let dialect = get_dialect(
        "jaffle_shop",
        fixture_root().to_str().unwrap(),
        dir.to_str().unwrap(),
    );
    assert_eq!(dialect.as_str(), "snowflake");
}

#[test]
fn workspace_dot_dbt_lookup_kept() {
    // no override: the fixture's <root>/.dbt/profiles.yml resolves duckdb
    let dialect = get_dialect("jaffle_shop", fixture_root().to_str().unwrap(), "");
    assert_eq!(dialect.as_str(), "duckdb");
}

#[test]
fn non_matching_override_falls_through_to_workspace() {
    let dir = temp_profiles(
        "other",
        "other_profile:\n  target: dev\n  outputs:\n    dev:\n      type: postgres\n",
    );
    let dialect = get_dialect(
        "jaffle_shop",
        fixture_root().to_str().unwrap(),
        dir.to_str().unwrap(),
    );
    assert_eq!(dialect.as_str(), "duckdb");
}

#[test]
fn candidate_order() {
    let candidates = profiles_candidates("/override", "/work");
    assert_eq!(candidates[0], Path::new("/override").join("profiles.yml"));
    // DBT_PROFILES_DIR may or may not be set in this environment; the two
    // workspace entries are adjacent and ordered either way
    let work = candidates
        .iter()
        .position(|p| *p == Path::new("/work").join("profiles.yml"))
        .expect("<root>/profiles.yml candidate");
    assert_eq!(
        candidates[work + 1],
        Path::new("/work").join(".dbt").join("profiles.yml")
    );
}

#[test]
fn context_builder_honors_profiles_dir() {
    let dir = temp_profiles("ctx", SNOWFLAKE_JAFFLE);
    let mut builder = ContextBuilder::default();
    builder.set_profiles_dir(dir.to_str().unwrap());
    let ctx = builder.build(fixture_root().to_str().unwrap());
    assert_eq!(ctx.dialect.as_str(), "snowflake");
}

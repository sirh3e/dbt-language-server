//! The on-disk differential corpus: `testdata/oracle/<case>/`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::claims::PlanClaims;

/// The corpus shipped with this repository.
pub fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/oracle")
}

/// The safety label of a fix (specs/ir-pipeline/05).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Safety {
    /// Preserves bag semantics.
    P,
    /// Changes intent; offered, never preferred.
    N,
}

/// What a fixture is there to prove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// P fix: the results must be bag-equal.
    Preserve,
    /// N fix: the results must differ, showing the intent change.
    Witness,
    /// N fix: the results must be equal where the precondition holds.
    Precondition,
}

/// A data shape a fixture exercises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cover {
    Nulls,
    Duplicates,
    Empty,
}

/// `expect.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// The rule id the fix belongs to, such as `join.left_nullified`.
    pub rule: String,
    /// What the fix does, in a few words.
    pub fix: String,
    pub safety: Safety,
    /// Why this label is correct for this SQL.
    pub description: String,
    /// The file holding the post-fix SQL.
    #[serde(default = "default_after")]
    pub after: String,
    pub fixtures: Vec<FixtureSpec>,
    #[serde(default)]
    pub plan: PlanClaims,
    /// An incremental case (specs/ir-pipeline/10): the model built on the
    /// seed, maintained through the delta by `strategy`, must equal it
    /// rebuilt in full on seed and delta. `after` is unused.
    #[serde(default)]
    pub incremental: Option<IncrementalSpec>,
}

/// `[incremental]` of an incremental case.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IncrementalSpec {
    pub strategy: IncrementalStrategy,
    /// The key `delete+insert` replaces rows by.
    #[serde(default)]
    pub unique_key: Vec<String>,
    /// The model's `is_incremental()` branch rendered, `{{ this }}` as
    /// `this_model`.
    #[serde(default = "default_increment")]
    pub increment: String,
}

/// The dbt strategies an incremental case applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum IncrementalStrategy {
    #[serde(rename = "append")]
    Append,
    #[serde(rename = "delete+insert")]
    DeleteInsert,
}

fn default_increment() -> String {
    "increment.sql".to_owned()
}

fn default_after() -> String {
    "after.sql".to_owned()
}

/// One `[[fixtures]]` entry.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureSpec {
    pub name: String,
    /// The seed file, relative to the case directory.
    pub seed: String,
    pub role: Role,
    #[serde(default)]
    pub covers: Vec<Cover>,
    /// Incremental cases: what arrives after the seed, as statements that
    /// replace the input tables with their rows then.
    #[serde(default)]
    pub delta: Option<String>,
}

/// A fixture with its seed SQL loaded.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub name: String,
    pub seed: String,
    pub role: Role,
    /// Incremental cases: the delta's SQL.
    pub delta: Option<String>,
}

/// An incremental case's strategy and increment, loaded.
#[derive(Debug, Clone)]
pub struct Incremental {
    pub strategy: IncrementalStrategy,
    pub unique_key: Vec<String>,
    pub increment: String,
}

/// One loaded and validated case.
#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub rule: String,
    pub safety: Safety,
    pub schema: String,
    pub model: String,
    pub after: String,
    pub fixtures: Vec<Fixture>,
    pub plan: PlanClaims,
    pub incremental: Option<Incremental>,
}

impl Case {
    /// The SQL that creates and seeds this fixture's tables.
    pub fn setup(&self, fixture: &Fixture) -> String {
        format!("{}\n{}", self.schema, fixture.seed)
    }
}

/// Load every case under `root`, in name order. Non-directories are ignored.
pub fn discover(root: &Path) -> Result<Vec<Case>, String> {
    let entries = fs::read_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
    let mut dirs = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|e| format!("{}: {e}", root.display()))?
            .path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    dirs.iter().map(|dir| load_case(dir)).collect()
}

/// Load and validate the case in `dir`.
pub fn load_case(dir: &Path) -> Result<Case, String> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| format!("{}: not a case directory", dir.display()))?;
    let context = |e: String| format!("case {name}: {e}");

    let text = read(&dir.join("expect.toml")).map_err(context)?;
    let expect: Expect = toml::from_str(&text).map_err(|e| context(e.to_string()))?;
    validate(&expect).map_err(context)?;

    let mut fixtures = Vec::with_capacity(expect.fixtures.len());
    for spec in &expect.fixtures {
        fixtures.push(Fixture {
            name: spec.name.clone(),
            seed: read(&dir.join(&spec.seed)).map_err(context)?,
            role: spec.role,
            delta: match &spec.delta {
                Some(file) => Some(read(&dir.join(file)).map_err(context)?),
                None => None,
            },
        });
    }
    let incremental = match &expect.incremental {
        Some(spec) => Some(Incremental {
            strategy: spec.strategy,
            unique_key: spec.unique_key.clone(),
            increment: read(&dir.join(&spec.increment)).map_err(context)?,
        }),
        None => None,
    };
    Ok(Case {
        rule: expect.rule,
        safety: expect.safety,
        schema: read(&dir.join("schema.sql")).map_err(context)?,
        model: read(&dir.join("model.sql")).map_err(context)?,
        after: match incremental {
            Some(_) => String::new(),
            None => read(&dir.join(&expect.after)).map_err(context)?,
        },
        fixtures,
        plan: expect.plan,
        incremental,
        name,
    })
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The structural rules that keep a case's label meaningful.
pub fn validate(expect: &Expect) -> Result<(), String> {
    if expect.fixtures.is_empty() {
        return Err("no fixtures".to_owned());
    }
    let has = |role| expect.fixtures.iter().any(|f| f.role == role);
    match expect.safety {
        Safety::P => {
            if has(Role::Witness) || has(Role::Precondition) {
                return Err("a P fix takes only preserve fixtures".to_owned());
            }
        }
        Safety::N => {
            if !has(Role::Witness) || !has(Role::Precondition) {
                return Err("an N fix needs a witness and a precondition fixture".to_owned());
            }
            if has(Role::Preserve) {
                return Err("an N fix takes no preserve fixtures".to_owned());
            }
        }
    }
    if let Some(spec) = &expect.incremental {
        if expect.fixtures.iter().any(|f| f.delta.is_none()) {
            return Err("an incremental case needs a delta on every fixture".to_owned());
        }
        if spec.strategy == IncrementalStrategy::DeleteInsert && spec.unique_key.is_empty() {
            return Err("delete+insert needs a unique_key".to_owned());
        }
    }
    let covered: BTreeSet<Cover> = expect
        .fixtures
        .iter()
        .flat_map(|f| f.covers.iter().copied())
        .collect();
    for needed in [Cover::Nulls, Cover::Duplicates, Cover::Empty] {
        if !covered.contains(&needed) {
            return Err(format!("no fixture covers {needed:?}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect(safety: Safety, roles: &[Role]) -> Expect {
        Expect {
            rule: "r".to_owned(),
            fix: "f".to_owned(),
            safety,
            description: "d".to_owned(),
            after: default_after(),
            fixtures: roles
                .iter()
                .map(|role| FixtureSpec {
                    name: "x".to_owned(),
                    seed: "seed.sql".to_owned(),
                    role: *role,
                    covers: vec![Cover::Nulls, Cover::Duplicates, Cover::Empty],
                    delta: None,
                })
                .collect(),
            plan: PlanClaims::default(),
            incremental: None,
        }
    }

    #[test]
    fn p_needs_only_preserve_fixtures() {
        assert!(validate(&expect(Safety::P, &[Role::Preserve])).is_ok());
        assert!(validate(&expect(Safety::P, &[Role::Witness])).is_err());
        assert!(validate(&expect(Safety::P, &[])).is_err());
    }

    #[test]
    fn n_needs_witness_and_precondition() {
        assert!(validate(&expect(Safety::N, &[Role::Witness, Role::Precondition])).is_ok());
        assert!(validate(&expect(Safety::N, &[Role::Witness])).is_err());
        assert!(validate(&expect(Safety::N, &[Role::Precondition])).is_err());
        assert!(validate(&expect(
            Safety::N,
            &[Role::Witness, Role::Precondition, Role::Preserve]
        ))
        .is_err());
    }

    #[test]
    fn every_data_shape_must_be_covered() {
        let mut e = expect(Safety::P, &[Role::Preserve]);
        e.fixtures[0].covers = vec![Cover::Nulls, Cover::Duplicates];
        assert!(validate(&e).unwrap_err().contains("Empty"));
    }

    #[test]
    fn an_incremental_case_needs_deltas_and_a_key_to_replace_by() {
        let mut e = expect(Safety::P, &[Role::Preserve]);
        e.incremental = Some(IncrementalSpec {
            strategy: IncrementalStrategy::DeleteInsert,
            unique_key: Vec::new(),
            increment: default_increment(),
        });
        assert!(validate(&e).unwrap_err().contains("delta"));
        e.fixtures[0].delta = Some("delta.sql".into());
        assert!(validate(&e).unwrap_err().contains("unique_key"));
        if let Some(spec) = e.incremental.as_mut() {
            spec.unique_key = vec!["day".into()];
        }
        assert!(validate(&e).is_ok());
    }

    #[test]
    fn a_missing_file_names_the_case() {
        let dir = tempfile::tempdir().expect("tempdir");
        let err = load_case(dir.path()).unwrap_err();
        assert!(err.contains("expect.toml"), "{err}");
    }
}

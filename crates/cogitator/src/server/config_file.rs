//! The hot-loadable server config: `<root>/cogitator.yml` (or
//! `.cogitator.yml`), every key optional. The pure core — parse, warn
//! about unknown keys with did-you-mean, DIFF two versions and APPLY
//! only the changed keys (so a file edit never clobbers runtime
//! switches it did not touch) — lives here; the watcher task is the
//! shell. `startup:` keys are honored only from `--config` at launch
//! (the tracing subscriber and parser backend are install-once).

use std::path::Path;

use cogitator_error::guidance::{closest_matches, Guidance};
use serde::Deserialize;

use super::{ProfilesConfig, SharedLimits, SharedToggles};

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct FileConfig {
    #[serde(default)]
    pub profiles: ProfilesSection,
    #[serde(default)]
    pub limits: LimitsSection,
    #[serde(default)]
    pub refresh: RefreshSection,
    #[serde(default)]
    pub startup: StartupSection,
    #[serde(default)]
    pub lint: LintSection,
    #[serde(default)]
    pub warehouse: WarehouseSection,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ProfilesSection {
    pub dir: Option<String>,
    pub profile: Option<String>,
    pub target: Option<String>,
    pub allow: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct LimitsSection {
    /// dbt.compiledSql wire cap in MiB.
    pub compiled_sql_mib: Option<u64>,
    /// target/ artifact files past this size are skipped.
    pub artifact_mib: Option<u64>,
}

/// Opt-in style checks (03 §10); absent keys fall back to the launch
/// baseline, so the changed-keys-only semantics hold per flag.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct LintSection {
    pub select_star: Option<bool>,
    pub unaliased_columns: Option<bool>,
    pub trailing_comma: Option<bool>,
    pub undocumented_columns: Option<bool>,
    pub dead_models: Option<bool>,
}

/// Bit per lint check inside the shared AtomicU32.
pub mod lint_flags {
    pub const SELECT_STAR: u32 = 1;
    pub const UNALIASED_COLUMNS: u32 = 1 << 1;
    pub const TRAILING_COMMA: u32 = 1 << 2;
    pub const UNDOCUMENTED_COLUMNS: u32 = 1 << 3;
    pub const DEAD_MODELS: u32 = 1 << 4;
}

impl LintSection {
    /// Resolve to bitflags; a key absent from the file keeps the bit
    /// it has in `base` (the launch value).
    pub fn flags(&self, base: u32) -> u32 {
        let mut flags = base;
        let mut bit = |value: Option<bool>, mask: u32| match value {
            Some(true) => flags |= mask,
            Some(false) => flags &= !mask,
            None => {}
        };
        bit(self.select_star, lint_flags::SELECT_STAR);
        bit(self.unaliased_columns, lint_flags::UNALIASED_COLUMNS);
        bit(self.trailing_comma, lint_flags::TRAILING_COMMA);
        bit(self.undocumented_columns, lint_flags::UNDOCUMENTED_COLUMNS);
        bit(self.dead_models, lint_flags::DEAD_MODELS);
        flags
    }
}

/// Warehouse introspection switches (duckdb builds only; harmless and
/// inert elsewhere).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct WarehouseSection {
    pub columns: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct RefreshSection {
    /// Context-refresh coalescing window.
    pub debounce_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct StartupSection {
    pub debug: Option<bool>,
    pub otlp: Option<String>,
    pub tree_sitter: Option<bool>,
    pub fusion: Option<String>,
}

impl StartupSection {
    pub fn is_empty(&self) -> bool {
        self == &StartupSection::default()
    }
}

/// Parse `text` (the content of `path`). Errors come back as Guidance
/// carrying the syntax position where saphyr knows one.
pub fn parse(text: &str, path: &Path) -> Result<FileConfig, Guidance> {
    if text.trim().is_empty() {
        return Ok(FileConfig::default()); // an empty file means defaults
    }
    if let Some((line, character, message)) = cogitator_schemas::yaml::syntax_error(text) {
        return Err(Guidance::new("cogitator.yml does not parse.")
            .at(format!("{}:{}:{}", path.display(), line + 1, character + 1))
            .because(message)
            .fix("fix the YAML; the previous configuration stays active."));
    }
    let marked = cogitator_schemas::yaml::load_marked(text, path).map_err(|err| {
        Guidance::new("cogitator.yml does not parse.")
            .at(path.display().to_string())
            .because(err.to_string())
            .fix("fix the YAML; the previous configuration stays active.")
    })?;
    let value = cogitator_schemas::yaml::to_json(&marked);
    serde_json::from_value(value).map_err(|err| {
        Guidance::new("cogitator.yml has a value of the wrong type.")
            .at(path.display().to_string())
            .because(err.to_string())
            .fix("compare against the documented schema in the README.")
    })
}

// kept in lockstep with cogitator_analysis::config_schema (test-pinned)
const ROOT_KEYS: &[&str] = &[
    "profiles",
    "limits",
    "refresh",
    "startup",
    "lint",
    "warehouse",
];
const PROFILES_KEYS: &[&str] = &["dir", "profile", "target", "allow"];
const LIMITS_KEYS: &[&str] = &["compiled_sql_mib", "artifact_mib"];
const REFRESH_KEYS: &[&str] = &["debounce_ms"];
const STARTUP_KEYS: &[&str] = &["debug", "otlp", "tree_sitter", "fusion"];
const LINT_KEYS: &[&str] = &[
    "select_star",
    "unaliased_columns",
    "trailing_comma",
    "undocumented_columns",
    "dead_models",
];
const WAREHOUSE_KEYS: &[&str] = &["columns"];

/// Unknown keys warn with did-you-mean — never a hard error, so a
/// typo'd key cannot take the rest of the file down.
pub fn unknown_key_warnings(text: &str, path: &Path) -> Vec<Guidance> {
    use cogitator_schemas::yaml::{as_str, entries, get, position_of};
    use saphyr::{LoadableYamlNode, MarkedYaml};

    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return Vec::new();
    };
    let Some(root) = docs.first() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut flag = |key: &MarkedYaml<'_>, known: &[&str], section: &str| {
        let Some(name) = as_str(key) else { return };
        if known.contains(&name) {
            return;
        }
        let position = position_of(key);
        out.push(
            Guidance::new(format!("'{name}' is not a cogitator.yml key{section}."))
                .at(format!("{}:{}", path.display(), position.line + 1))
                .suggest(closest_matches(name, known.iter().copied()))
                .fix("the full schema is documented in the README."),
        );
    };
    for (key, _) in entries(root) {
        flag(key, ROOT_KEYS, "");
    }
    for (section, keys) in [
        ("profiles", PROFILES_KEYS),
        ("limits", LIMITS_KEYS),
        ("refresh", REFRESH_KEYS),
        ("startup", STARTUP_KEYS),
        ("lint", LINT_KEYS),
        ("warehouse", WAREHOUSE_KEYS),
    ] {
        if let Some(node) = get(root, section) {
            for (key, _) in entries(node) {
                flag(key, keys, &format!(" under {section}:"));
            }
        }
    }
    out
}

/// One applied difference between two file versions. `None` values in
/// the NEW file revert the key to the launch-time `baseline`.
#[derive(Debug, PartialEq)]
pub enum Change {
    ProfilesDir(String),
    Profile(String),
    Target(String),
    Allow(Vec<String>),
    CompiledCapMib(u64),
    DebounceMs(u64),
    ArtifactCapMib(u64),
    LintFlags(u32),
    WarehouseColumns(bool),
}

/// The changed-keys-only diff: a key participates only when its file
/// value CHANGED between `old` and `new`; a key that disappeared
/// reverts to `baseline`'s launch value.
pub fn diff(old: &FileConfig, new: &FileConfig, baseline: &Baseline) -> Vec<Change> {
    let mut out = Vec::new();
    let mut field = |old_v: &Option<String>,
                     new_v: &Option<String>,
                     fallback: &str,
                     make: fn(String) -> Change| {
        if old_v != new_v {
            out.push(make(new_v.clone().unwrap_or_else(|| fallback.to_string())));
        }
    };
    field(
        &old.profiles.dir,
        &new.profiles.dir,
        &baseline.profiles.dir_override,
        Change::ProfilesDir,
    );
    field(
        &old.profiles.profile,
        &new.profiles.profile,
        &baseline.profiles.profile,
        Change::Profile,
    );
    field(
        &old.profiles.target,
        &new.profiles.target,
        &baseline.profiles.target,
        Change::Target,
    );
    if old.profiles.allow != new.profiles.allow {
        out.push(Change::Allow(
            new.profiles
                .allow
                .clone()
                .unwrap_or_else(|| baseline.profiles.allow.clone()),
        ));
    }
    if old.limits.compiled_sql_mib != new.limits.compiled_sql_mib {
        out.push(Change::CompiledCapMib(
            new.limits
                .compiled_sql_mib
                .unwrap_or(baseline.compiled_cap_mib),
        ));
    }
    if old.refresh.debounce_ms != new.refresh.debounce_ms {
        out.push(Change::DebounceMs(
            new.refresh.debounce_ms.unwrap_or(baseline.debounce_ms),
        ));
    }
    if old.limits.artifact_mib != new.limits.artifact_mib {
        out.push(Change::ArtifactCapMib(
            new.limits.artifact_mib.unwrap_or(baseline.artifact_mib),
        ));
    }
    if old.lint != new.lint {
        out.push(Change::LintFlags(new.lint.flags(baseline.lint_flags)));
    }
    if old.warehouse != new.warehouse {
        out.push(Change::WarehouseColumns(
            new.warehouse.columns.unwrap_or(baseline.warehouse_columns),
        ));
    }
    out
}

/// Launch precedence (defaults < file < CLI < initializationOptions):
/// on the FIRST load of the root file, a profiles key that a higher
/// layer already set (non-empty in the baseline) stays with that layer.
/// Later edits apply unconditionally — last writer wins.
pub fn launch_filter(changes: Vec<Change>, baseline: &Baseline) -> Vec<Change> {
    changes
        .into_iter()
        .filter(|change| match change {
            Change::ProfilesDir(_) => baseline.profiles.dir_override.is_empty(),
            Change::Profile(_) => baseline.profiles.profile.is_empty(),
            Change::Target(_) => baseline.profiles.target.is_empty(),
            Change::Allow(_) => baseline.profiles.allow.is_empty(),
            Change::CompiledCapMib(_)
            | Change::DebounceMs(_)
            | Change::ArtifactCapMib(_)
            | Change::LintFlags(_)
            | Change::WarehouseColumns(_) => true,
        })
        .collect()
}

/// The launch-time values keys revert to when removed from the file.
#[derive(Debug, Clone)]
pub struct Baseline {
    pub profiles: ProfilesConfig,
    pub compiled_cap_mib: u64,
    pub debounce_ms: u64,
    pub artifact_mib: u64,
    pub lint_flags: u32,
    pub warehouse_columns: bool,
}

/// Apply changes to the live state; returns the key names for the one
/// INFO message. State writes stay in this shell helper (05 §6.2).
pub fn apply(
    changes: &[Change],
    profiles: &std::sync::Mutex<ProfilesConfig>,
    limits: &SharedLimits,
    toggles: &SharedToggles,
) -> Vec<&'static str> {
    use std::sync::atomic::Ordering;

    let mut applied = Vec::new();
    for change in changes {
        match change {
            Change::ProfilesDir(dir) => {
                profiles.lock().expect("profiles config lock").dir_override = dir.clone();
                applied.push("profiles.dir");
            }
            Change::Profile(profile) => {
                profiles.lock().expect("profiles config lock").profile = profile.clone();
                applied.push("profiles.profile");
            }
            Change::Target(target) => {
                profiles.lock().expect("profiles config lock").target = target.clone();
                applied.push("profiles.target");
            }
            Change::Allow(allow) => {
                profiles.lock().expect("profiles config lock").allow = allow.clone();
                applied.push("profiles.allow");
            }
            Change::CompiledCapMib(mib) => {
                limits
                    .compiled_cap_bytes
                    .store(mib * 1024 * 1024, Ordering::Relaxed);
                applied.push("limits.compiled_sql_mib");
            }
            Change::DebounceMs(ms) => {
                limits.refresh_debounce_ms.store(*ms, Ordering::Relaxed);
                applied.push("refresh.debounce_ms");
            }
            Change::ArtifactCapMib(mib) => {
                limits
                    .artifact_cap_bytes
                    .store(mib * 1024 * 1024, Ordering::Relaxed);
                applied.push("limits.artifact_mib");
            }
            Change::LintFlags(flags) => {
                toggles.lints.store(*flags, Ordering::Relaxed);
                applied.push("lint");
            }
            Change::WarehouseColumns(on) => {
                toggles.warehouse_columns.store(*on, Ordering::Relaxed);
                applied.push("warehouse.columns");
            }
        }
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline() -> Baseline {
        Baseline {
            profiles: ProfilesConfig::default(),
            compiled_cap_mib: 2,
            debounce_ms: 200,
            artifact_mib: 64,
            lint_flags: 0,
            warehouse_columns: false,
        }
    }

    #[test]
    fn key_lists_match_the_shared_schema() {
        use cogitator_analysis::config_schema::{section, COGITATOR_SCHEMA};
        let names: Vec<&str> = COGITATOR_SCHEMA.iter().map(|s| s.name).collect();
        assert_eq!(ROOT_KEYS, names.as_slice());
        for (keys, name) in [
            (PROFILES_KEYS, "profiles"),
            (LIMITS_KEYS, "limits"),
            (REFRESH_KEYS, "refresh"),
            (STARTUP_KEYS, "startup"),
            (LINT_KEYS, "lint"),
            (WAREHOUSE_KEYS, "warehouse"),
        ] {
            let schema: Vec<&str> = section(name)
                .expect(name)
                .keys
                .iter()
                .map(|k| k.key)
                .collect();
            assert_eq!(keys, schema.as_slice(), "{name}");
        }
    }

    #[test]
    fn lint_section_diffs_into_flags() {
        let old = parse("lint:\n  select_star: true\n", Path::new("x")).unwrap();
        let new = parse(
            "lint:\n  select_star: true\n  dead_models: true\n",
            Path::new("x"),
        )
        .unwrap();
        let changes = diff(&old, &new, &baseline());
        assert_eq!(
            changes,
            vec![Change::LintFlags(
                lint_flags::SELECT_STAR | lint_flags::DEAD_MODELS
            )]
        );
        // removing the whole section reverts to the baseline (all off)
        let gone = parse("", Path::new("x")).unwrap();
        let reverted = diff(&new, &gone, &baseline());
        assert_eq!(reverted, vec![Change::LintFlags(0)]);
    }

    #[test]
    fn parses_partial_and_empty_files() {
        let path = Path::new("cogitator.yml");
        let full = parse(
            "profiles:\n  dir: /etc/dbt\n  target: prod\nlimits:\n  compiled_sql_mib: 4\n",
            path,
        )
        .unwrap();
        assert_eq!(full.profiles.dir.as_deref(), Some("/etc/dbt"));
        assert_eq!(full.limits.compiled_sql_mib, Some(4));
        assert!(full.startup.is_empty());

        let empty = parse("", path).unwrap();
        assert_eq!(empty, FileConfig::default());
    }

    #[test]
    fn syntax_errors_carry_a_position() {
        let err = parse("profiles:\n  dir: [unclosed\n", Path::new("cogitator.yml"))
            .expect_err("must fail");
        let message = err.to_message();
        assert!(message.contains("cogitator.yml:"), "{message}");
        assert!(
            message.contains("previous configuration stays active"),
            "{message}"
        );
    }

    #[test]
    fn unknown_keys_suggest() {
        let warnings = unknown_key_warnings(
            "profils:\n  dir: /x\nprofiles:\n  targett: dev\n",
            Path::new("cogitator.yml"),
        );
        let texts: Vec<String> = warnings.iter().map(|w| w.to_message()).collect();
        assert!(
            texts
                .iter()
                .any(|t| t.contains("'profils'") && t.contains("profiles")),
            "{texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|t| t.contains("'targett'") && t.contains("target")),
            "{texts:?}"
        );
    }

    #[test]
    fn diff_applies_changed_keys_only_and_reverts_removals() {
        let old = parse("profiles:\n  target: dev\n  dir: /a\n", Path::new("x")).unwrap();
        let new = parse("profiles:\n  target: prod\n", Path::new("x")).unwrap();
        let changes = diff(&old, &new, &baseline());
        // target changed; dir was REMOVED → reverts to the baseline (empty)
        assert!(
            changes.contains(&Change::Target("prod".into())),
            "{changes:?}"
        );
        assert!(
            changes.contains(&Change::ProfilesDir(String::new())),
            "{changes:?}"
        );
        assert_eq!(changes.len(), 2, "untouched keys stay out: {changes:?}");
    }

    #[test]
    fn launch_filter_keeps_higher_layers() {
        let mut base = baseline();
        base.profiles.target = "prod".into(); // set via CLI/initOptions
        let kept = launch_filter(
            vec![
                Change::Target("dev".into()),
                Change::ProfilesDir("/x".into()),
                Change::CompiledCapMib(4),
            ],
            &base,
        );
        // the CLI-set target survives the first file load; the rest applies
        assert_eq!(
            kept,
            vec![Change::ProfilesDir("/x".into()), Change::CompiledCapMib(4)]
        );
    }

    #[test]
    fn apply_writes_the_live_state() {
        use std::sync::atomic::Ordering;
        let profiles = std::sync::Mutex::new(ProfilesConfig::default());
        let limits = SharedLimits::default();
        let toggles = SharedToggles::default();
        let applied = apply(
            &[
                Change::Target("prod".into()),
                Change::CompiledCapMib(4),
                Change::DebounceMs(50),
                Change::LintFlags(lint_flags::SELECT_STAR),
            ],
            &profiles,
            &limits,
            &toggles,
        );
        assert_eq!(
            applied,
            vec![
                "profiles.target",
                "limits.compiled_sql_mib",
                "refresh.debounce_ms",
                "lint"
            ]
        );
        assert_eq!(profiles.lock().unwrap().target, "prod");
        assert_eq!(
            limits.compiled_cap_bytes.load(Ordering::Relaxed),
            4 * 1024 * 1024
        );
        assert_eq!(limits.refresh_debounce_ms.load(Ordering::Relaxed), 50);
        assert_eq!(
            toggles.lints.load(Ordering::Relaxed),
            lint_flags::SELECT_STAR
        );
    }
}

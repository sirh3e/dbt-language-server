//! The cogitator.yml schema as data (03 §10): one table that the
//! server's config parser, the YAML diagnostics, completion and hover
//! all consume, so the file's documentation can never drift from what
//! the server actually reads.

/// One documented key inside a section.
#[derive(Debug, Clone, Copy)]
pub struct KeyDoc {
    pub key: &'static str,
    /// One-sentence hover/completion documentation.
    pub doc: &'static str,
    /// What a value looks like, for completion detail ("bool", "MiB"…).
    pub value_hint: &'static str,
}

/// One top-level section and its keys.
#[derive(Debug, Clone, Copy)]
pub struct SectionSchema {
    pub name: &'static str,
    pub doc: &'static str,
    pub keys: &'static [KeyDoc],
}

const fn key(key: &'static str, doc: &'static str, value_hint: &'static str) -> KeyDoc {
    KeyDoc {
        key,
        doc,
        value_hint,
    }
}

/// The whole cogitator.yml surface. `startup:` keys only apply via
/// `--config` at launch; everything else hot-reloads.
pub const COGITATOR_SCHEMA: &[SectionSchema] = &[
    SectionSchema {
        name: "profiles",
        doc: "dbt profiles overrides — hot: editing these re-resolves the active profile/target.",
        keys: &[
            key("dir", "Directory containing profiles.yml (like --profiles-dir).", "path"),
            key("profile", "Active profile override; empty means the project's own.", "name"),
            key("target", "Active target override (like --target).", "name"),
            key("allow", "Profiles readable besides the project's own (like --profiles-allow).", "[names]"),
        ],
    },
    SectionSchema {
        name: "limits",
        doc: "Hot-tunable size caps.",
        keys: &[
            key("compiled_sql_mib", "dbt.compiledSql wire cap; bigger files answer uri-only.", "MiB"),
            key("artifact_mib", "target/ artifact files past this size are skipped.", "MiB"),
        ],
    },
    SectionSchema {
        name: "refresh",
        doc: "Context-refresh scheduling.",
        keys: &[key("debounce_ms", "Coalescing window for refresh triggers.", "ms")],
    },
    SectionSchema {
        name: "startup",
        doc: "Launch-only keys (install-once subsystems); honored via --config, a restart applies changes.",
        keys: &[
            key("debug", "Plain-text logging to log.txt beside the executable.", "bool"),
            key("otlp", "OTLP trace/metric export endpoint; empty uses the SDK default.", "url"),
            key("tree_sitter", "Tree-sitter document backend (false: phase-1 pipeline).", "bool"),
            key("fusion", "dbt Fusion binary path enabling compile diagnostics.", "path"),
        ],
    },
    SectionSchema {
        name: "lint",
        doc: "Opt-in style diagnostics; every check defaults to off.",
        keys: &[
            key("select_star", "Warn on SELECT * outside staging models.", "bool"),
            key("unaliased_columns", "Warn on bare columns in multi-relation FROM/JOINs.", "bool"),
            key("trailing_comma", "Warn on trailing commas the dialect rejects.", "bool"),
            key("undocumented_columns", "Hint on produced columns missing from the schema YAML.", "bool"),
            key("dead_models", "Hint on models nothing downstream uses.", "bool"),
        ],
    },
    SectionSchema {
        name: "warehouse",
        doc: "Warehouse introspection (duckdb builds only).",
        keys: &[key(
            "columns",
            "Complete source columns from the warehouse when the YAML has none.",
            "bool",
        )],
    },
];

/// Bit per lint check inside the shared AtomicU32 — one truth for the
/// config parser and the lint feature.
pub mod lint_flags {
    pub const SELECT_STAR: u32 = 1;
    pub const UNALIASED_COLUMNS: u32 = 1 << 1;
    pub const TRAILING_COMMA: u32 = 1 << 2;
    pub const UNDOCUMENTED_COLUMNS: u32 = 1 << 3;
    pub const DEAD_MODELS: u32 = 1 << 4;
}

/// The section entry by name.
pub fn section(name: &str) -> Option<&'static SectionSchema> {
    COGITATOR_SCHEMA.iter().find(|s| s.name == name)
}

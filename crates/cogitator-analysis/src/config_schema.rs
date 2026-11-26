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

impl SectionSchema {
    /// A section whose keys are the user's own (`vars:` names vars),
    /// so no key of it is unknown.
    pub fn is_free_form(&self) -> bool {
        self.name == "vars"
    }
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
        doc: "Style diagnostics default to off; the macro call, dispatch, syntax and model checks default to on.",
        keys: &[
            key("select_star", "Warn on SELECT * outside staging models.", "bool"),
            key("unaliased_columns", "Warn on bare columns in multi-relation FROM/JOINs.", "bool"),
            key("trailing_comma", "Warn on trailing commas the dialect rejects.", "bool"),
            key("undocumented_columns", "Hint on produced columns missing from the schema YAML.", "bool"),
            key("dead_models", "Hint on models nothing downstream uses.", "bool"),
            key("macro_calls", "Warn on macro calls whose arguments the macro does not take (default on).", "bool"),
            key("dispatch", "Warn on adapter.dispatch calls with no default__ nor active-adapter implementation (default on).", "bool"),
            key("syntax", "Report Jinja and SQL syntax errors as you type, without dbt compile (default on).", "bool"),
            key("model", "Report invalid models as you type: unknown materializations and config, self-refs and ref cycles, an empty model (default on).", "bool"),
        ],
    },
    SectionSchema {
        name: "log",
        doc: "What reaches the editor's log view (window/logMessage).",
        keys: &[key(
            "client_level",
            "Verbosity of the editor-visible log stream; $/setTrace adjusts it too (last writer wins).",
            "off|error|warn|info|debug",
        )],
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
    SectionSchema {
        name: "perf",
        doc: "Query-plan performance diagnostics; dbt.queryPlan always works, whatever is set here.",
        keys: &[
            key(
                "profile",
                "Which rules report as diagnostics: off (default), recommended or strict.",
                "off|recommended|strict",
            ),
            key(
                "catalog_stats",
                "Use catalog statistics when ordering findings; statistics never raise a severity.",
                "bool",
            ),
            key(
                "row_hints",
                "Show the row counts catalog statistics prove for each CTE as inlay hints (default true).",
                "bool",
            ),
            key(
                "inline_findings",
                "Also show the reported findings as inlay hints, with the whole finding as the tooltip (default false).",
                "bool",
            ),
        ],
    },
    SectionSchema {
        name: "inlay",
        doc: "Which inlay hints the server sends.",
        keys: &[
            key("vars", "Show the value each var() resolves to (default true).", "bool"),
            key(
                "env_vars",
                "Show the value each env_var() reads, secret-looking names masked (default true).",
                "bool",
            ),
            key(
                "jinja",
                "Show what short single-line {{ … }} expressions render to (default false).",
                "bool",
            ),
        ],
    },
    SectionSchema {
        name: "vars",
        doc: "Var overrides, like dbt's --vars: name → value. They win over dbt_project.yml and are passed to the dbt commands the server runs.",
        keys: &[],
    },
    SectionSchema {
        name: "perf_rules",
        doc: "Per-rule severity, by rule id; wins over the profile. A rule named here reports even under profile off.",
        keys: &[
            key("join.left_nullified", "WHERE makes this LEFT JOIN inner.", RULE_SEVERITIES),
            key("join.cartesian", "Join with no key.", RULE_SEVERITIES),
            key("join.distinct_fanout", "DISTINCT may hide join fan-out.", RULE_SEVERITIES),
            key("model.unused_cte", "CTE never used.", RULE_SEVERITIES),
            key("cross.select_star_chain", "SELECT * reading columns nothing downstream uses (also lint.select_star).", RULE_SEVERITIES),
            key("filter.non_sargable", "Wrapped column may block pruning.", RULE_SEVERITIES),
            key("incr.missing_guard", "Incremental model reprocesses all input each run.", RULE_SEVERITIES),
            key("incr.subquery_watermark", "Subquery filter does not prune.", RULE_SEVERITIES),
            key("set.union_distinct", "UNION dedupes; UNION ALL?", RULE_SEVERITIES),
            key("cross.unused_column", "No downstream use in this project.", RULE_SEVERITIES),
            key("cross.filter_late", "Every consumer repeats the same filter.", RULE_SEVERITIES),
            key("cross.ephemeral_repeat", "Ephemeral model recomputed in each consumer.", RULE_SEVERITIES),
            key("cross.duplicate_logic", "Same logic as another model.", RULE_SEVERITIES),
            key("order.in_cte", "ORDER BY has no effect here.", RULE_SEVERITIES),
            key("subq.not_in_nullable", "NOT IN over a nullable column can return nothing.", RULE_SEVERITIES),
            key("subq.correlated", "Correlated subquery.", RULE_SEVERITIES),
            key("join.redundant_left", "LEFT JOIN adds nothing.", RULE_SEVERITIES),
            key("join.key_type_mismatch", "Join keys have different types.", RULE_SEVERITIES),
            key("filter.partition_or", "OR defeats partition pruning.", RULE_SEVERITIES),
            key("filter.cluster_order", "Filter skips the leading cluster column.", RULE_SEVERITIES),
            key("filter.constant_false", "Filter is always false.", RULE_SEVERITIES),
            key("limit.no_scan_reduction", "LIMIT does not reduce bytes read.", RULE_SEVERITIES),
            key("incr.no_unique_key", "Merge strategy without a unique_key.", RULE_SEVERITIES),
            key("incr.could_be_incremental", "A table that could be built incrementally.", RULE_SEVERITIES),
        ],
    },
    SectionSchema {
        name: "preview",
        doc: "dbt.previewCte (the \"Preview rows\" lenses): how many rows a preview shows.",
        keys: &[
            key("rows", "Rows a preview shows when the call names no limit (default 20).", "rows"),
            key("max_rows", "The most rows a preview may ask for; larger limits are capped (default 1000).", "rows"),
        ],
    },
];

/// The `perf_rules` value set (one severity per entry).
const RULE_SEVERITIES: &str = "off|hint|info|warning";

/// Bit per lint check inside the shared AtomicU32 — one truth for the
/// config parser and the lint feature.
pub mod lint_flags {
    pub const SELECT_STAR: u32 = 1;
    pub const UNALIASED_COLUMNS: u32 = 1 << 1;
    pub const TRAILING_COMMA: u32 = 1 << 2;
    pub const UNDOCUMENTED_COLUMNS: u32 = 1 << 3;
    pub const DEAD_MODELS: u32 = 1 << 4;
    pub const MACRO_CALLS: u32 = 1 << 5;
    pub const DISPATCH: u32 = 1 << 6;
    pub const SYNTAX: u32 = 1 << 7;
    pub const MODEL: u32 = 1 << 8;
    /// The checks that are on unless cogitator.yml turns them off: they
    /// report what fails at compile or run time, not style.
    pub const DEFAULT_ON: u32 = MACRO_CALLS | DISPATCH | SYNTAX | MODEL;
}

/// The section entry by name.
pub fn section(name: &str) -> Option<&'static SectionSchema> {
    COGITATOR_SCHEMA.iter().find(|s| s.name == name)
}

/// The same table as a JSON Schema document (draft-07, the dialect
/// yaml-language-server understands): editors validate cogitator.yml
/// without the server running. Typed wire structs, one truth —
/// regenerating can never drift from what the parser reads.
pub mod json {
    use std::collections::BTreeMap;

    use serde::Serialize;

    use super::{KeyDoc, COGITATOR_SCHEMA};

    #[derive(Debug, Serialize)]
    pub struct ConfigJsonSchema {
        #[serde(rename = "$schema")]
        pub dialect: &'static str,
        #[serde(rename = "$id")]
        pub id: &'static str,
        pub title: &'static str,
        pub description: &'static str,
        #[serde(rename = "type")]
        pub ty: &'static str,
        #[serde(rename = "additionalProperties")]
        pub additional: bool,
        pub properties: BTreeMap<&'static str, Section>,
    }

    #[derive(Debug, Serialize)]
    pub struct Section {
        pub description: &'static str,
        #[serde(rename = "type")]
        pub ty: &'static str,
        #[serde(rename = "additionalProperties")]
        pub additional: bool,
        pub properties: BTreeMap<&'static str, Key>,
    }

    #[derive(Debug, Serialize)]
    pub struct Key {
        pub description: String,
        #[serde(rename = "type")]
        pub ty: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub minimum: Option<u64>,
        #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
        pub variants: Option<Vec<&'static str>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub items: Option<Items>,
    }

    #[derive(Debug, Serialize)]
    pub struct Items {
        #[serde(rename = "type")]
        pub ty: &'static str,
    }

    /// value_hint → JSON type: sizes/durations are integers, `[names]`
    /// a string array, `a|b|c` a closed string enum, all else string.
    fn key_json(doc: &KeyDoc) -> Key {
        let (ty, minimum, variants, items) = match doc.value_hint {
            "bool" => ("boolean", None, None, None),
            "MiB" | "ms" => ("integer", Some(0), None, None),
            "[names]" => ("array", None, None, Some(Items { ty: "string" })),
            hint if hint.contains('|') => ("string", None, Some(hint.split('|').collect()), None),
            _ => ("string", None, None, None),
        };
        Key {
            description: format!("{} ({})", doc.doc, doc.value_hint),
            ty,
            minimum,
            variants,
            items,
        }
    }

    /// The whole cogitator.yml schema document.
    pub fn config_json_schema() -> ConfigJsonSchema {
        ConfigJsonSchema {
            dialect: "http://json-schema.org/draft-07/schema#",
            id: "https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/schemas/cogitator.schema.json",
            title: "cogitator.yml",
            description: "The cogitator dbt language server's per-project \
                          configuration file. Every section except 'startup:' \
                          hot-reloads; 'startup:' keys apply only via --config \
                          at launch.",
            ty: "object",
            additional: false,
            properties: COGITATOR_SCHEMA
                .iter()
                .map(|section| {
                    (
                        section.name,
                        Section {
                            description: section.doc,
                            ty: "object",
                            additional: section.is_free_form(),
                            properties: section
                                .keys
                                .iter()
                                .map(|key| (key.key, key_json(key)))
                                .collect(),
                        },
                    )
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::plan::diagnostics_config::advice_rule_ids;

    #[test]
    fn perf_rule_keys_cover_exactly_the_registry() {
        let mut schema: Vec<&str> = section("perf_rules")
            .expect("perf_rules")
            .keys
            .iter()
            .map(|k| k.key)
            .collect();
        schema.sort_unstable();
        let mut registry: Vec<&str> = advice_rule_ids().collect();
        registry.sort_unstable();
        assert_eq!(schema, registry, "registry and perf_rules schema drifted");
    }
}

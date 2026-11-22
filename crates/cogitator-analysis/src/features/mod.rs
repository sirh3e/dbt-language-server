//! Feature algorithms (03 §9 + FEATURE_REQUESTS): pure functions over the
//! context snapshot and a parsed document — the dispatcher serializes
//! (functional core / imperative shell, 05 §3).

mod artifact_status;
mod call_hierarchy;
mod code_actions;
mod compiled;
mod completion;
mod dead_models;
mod definition;
mod diagnostics;
mod document_links;
mod extract;
pub mod formatting;
mod governance;
mod graph_nav;
mod hover;
mod impact;
mod inlay_hints;
mod jinja_preview;
mod lenses;
mod linked_editing;
pub mod lint;
mod model_info;
mod navigation;
pub mod plan;
pub mod quality;
mod references;
mod relation;
mod rename;
mod rename_macro;
mod signature;
mod structure;
mod tests_nav;
mod vars;
mod workspace_diagnostics;

pub use artifact_status::{artifact_diagnostics, freshness_lenses, run_lens};
pub use call_hierarchy::{
    macro_incoming_calls, macro_outgoing_calls, outgoing_in_text, prepare_call_hierarchy,
    prepare_macro_hierarchy,
};
pub use code_actions::convert_relation::{convert_literal_relations, RelationCatalog};
pub use code_actions::declare_missing::declare_missing_actions;
pub use code_actions::define_missing::define_missing_actions;
pub use code_actions::extract_cte::extract_cte_action;
pub use code_actions::governance_fixes::private_ref_actions;
pub use code_actions::refactors::{expand_star_action, extract_yaml_action};
pub use code_actions::split_model::split_action;
pub use code_actions::unit_test_scaffold::unit_test_scaffold_action;
pub use code_actions::yaml_columns::{
    add_missing_columns_action, remove_unproduced_columns_action,
};
pub use code_actions::yaml_fixes::{inherit_description_actions, scaffold_tests_action};
pub use code_actions::yaml_unproduced::unproduced_column_diagnostics;
pub use code_actions::{code_actions, model_entry_at, TargetChoice};
pub use compiled::{compiled_target, CompiledSql};
pub use completion::{completion, resolve_completion_documentation};
pub use dead_models::{dead_models, is_dead, DeadModel};
pub use definition::{definition, function_doc_target, go_to_schema, FunctionDoc};
pub use diagnostics::{
    citizen_diagnostics, dangling_doc_diagnostics, dangling_ref_diagnostics, packages_diagnostics,
};
pub use document_links::{document_links, document_links_at};
pub use extract::{extract_model_yaml, ExtractEdit};
pub use governance::{contract_diagnostics, private_ref_diagnostics};
pub use graph_nav::{
    model_graph, project_graph_json, GraphEntry, GraphNodeKind, ModelGraph, ProjectGraph,
};
pub use hover::{hover, hover_with_lineage};
pub use impact::{impact, impact_targets, ImpactReport, IMPACT_LINEAGE_CAP};
pub use inlay_hints::{inlay_hints, inlay_hints_with, InlayOptions};
pub use jinja_preview::{expand_jinja, ExpandedSql, PreviewInputs};
pub use lenses::code_lenses;
pub use linked_editing::linked_editing;
pub use model_info::{compiled_sql_path, model_info, ModelInfo, ModelVersions, VersionEntry};
pub use navigation::{document_highlights, document_symbols, references, workspace_symbols};
pub use references::references_at;
pub use relation::{relation_info, RelationInfo, RelationKind, TargetRelation};
pub use rename::{prepare_rename, rename_model};
pub use rename_macro::{prepare_rename_at, rename_at};
pub use signature::signature_help;
pub use structure::{folding_ranges, selection_ranges, semantic_tokens};
pub use tests_nav::{model_tests, model_tests_json, ModelTest, ModelTests};
pub use vars::{key_definition, var_key_at, var_references};
pub use workspace_diagnostics::project_diagnostics;

use std::path::Path;

use cogitator_lsp::{Position, Range};
use cogitator_parser::Token;

pub(crate) use crate::context::DbtContext;
pub(crate) use crate::Document;

fn file_uri(path: &Path) -> String {
    cogitator_lsp::fs_path_to_uri(path)
}

pub fn model_name_from_uri(uri: &str) -> Option<String> {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let base = Path::new(&path).file_name()?.to_string_lossy();
    base.strip_suffix(".sql").map(str::to_string)
}

// ---------------------------------------------------------------------------
// Completion (03 §9.4)
// ---------------------------------------------------------------------------

pub(crate) fn token_range(t: &Token) -> Range {
    Range {
        start: Position {
            line: t.line,
            character: t.column,
        },
        end: Position {
            line: t.line,
            character: t.column + t.len(),
        },
    }
}

/// 0-based position of a byte offset (byte columns, like tokens).
pub(crate) fn byte_position(text: &str, byte: u32) -> Position {
    let before = &text[..(byte as usize).min(text.len())];
    let line = before.matches('\n').count() as u32;
    let start = before.rfind('\n').map_or(0, |i| i + 1);
    Position {
        line,
        character: (before.len() - start) as u32,
    }
}

/// Where a var's value came from, in words.
pub(crate) fn var_source_words(v: &crate::context::VariableDetails) -> String {
    use crate::context::VarSource;
    let file = v
        .uri
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "initializationOptions".to_string());
    let line = v.range.start.line + 1;
    match &v.source {
        VarSource::Project => format!("{file}:{line}"),
        VarSource::Scoped(scope) => format!("{file}:{line}, vars.{scope}"),
        VarSource::Package(pkg) => format!("package {pkg}, {file}:{line}"),
        VarSource::Override if v.uri.as_os_str().is_empty() => {
            "override from initializationOptions".to_string()
        }
        VarSource::Override => format!("override, {file}:{line}"),
    }
}

pub(crate) fn display_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Every same-kind, same-literal occurrence of `token` as ranges —
/// shared by documentHighlight and linkedEditingRange.
pub(crate) fn same_literal_ranges(
    doc: &Document,
    token: &cogitator_parser::Token,
) -> Vec<cogitator_lsp::Range> {
    let literal = doc.parsed.literal(token);
    doc.parsed
        .tokens
        .iter()
        .filter(|t| t.kind == token.kind && doc.parsed.literal(t) == literal)
        .map(token_range)
        .collect()
}

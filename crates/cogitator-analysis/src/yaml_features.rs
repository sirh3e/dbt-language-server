//! dbt YAML intelligence (FEATURE_REQUESTS P3, spec 08 §2.6 — basic v1):
//! parse/deserialize diagnostics and heuristic property-name completion.

use std::path::Path;
use std::sync::LazyLock;

use cogitator_lsp::{
    completion_kind, diagnostic_severity, CompletionItem, Diagnostic, Position, Range,
};

/// Diagnostics for a dbt YAML document: YAML syntax errors at their reported
/// position, schema-level deserialize errors at the file start.
pub fn yaml_diagnostics(uri: &str, text: &str) -> Vec<Diagnostic> {
    let fs_path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let path = Path::new(&fs_path);
    let is_project = path.file_name().is_some_and(|f| f == "dbt_project.yml");

    if let Some((line, character, message)) = cogitator_schemas::yaml::syntax_error(text) {
        return vec![diag(
            Range::zero_width(Position { line, character }),
            format!("yaml syntax error: {message}"),
            diagnostic_severity::ERROR,
        )];
    }

    let result = if is_project {
        let resolved = crate::util::resolve_env_vars(text);
        cogitator_schemas::project::parse_dbt_project(&resolved, path).map(|_| ())
    } else {
        cogitator_schemas::properties::parse_properties(text, path).map(|_| ())
    };
    match result {
        Ok(()) => Vec::new(),
        Err(err) => vec![diag(
            Range::default(),
            format!("dbt schema error: {err}"),
            diagnostic_severity::ERROR,
        )],
    }
}

fn diag(range: Range, message: String, severity: i32) -> Diagnostic {
    Diagnostic {
        range,
        message,
        severity,
        code: "dbt-yaml".to_string(),
        source: "cogitator".to_string(),
    }
}

static ROOT_KEYS: LazyLock<Vec<String>> =
    LazyLock::new(cogitator_schemas::schema_keys::properties_root_keys);
static MODEL_KEYS: LazyLock<Vec<String>> =
    LazyLock::new(cogitator_schemas::schema_keys::model_item_keys);
static SOURCE_KEYS: LazyLock<Vec<String>> =
    LazyLock::new(cogitator_schemas::schema_keys::source_item_keys);
static PROJECT_KEYS: LazyLock<Vec<String>> =
    LazyLock::new(cogitator_schemas::schema_keys::project_root_keys);

/// Property-name completion in dbt `.yml` files (heuristic v1: section is
/// derived from the nearest top-level key above the cursor; the schema keys
/// come from the vendored dbt-jsonschema files).
pub fn yaml_completion(uri: &str, text: &str, pos: Position) -> Vec<CompletionItem> {
    let is_project = uri.ends_with("dbt_project.yml");
    let lines: Vec<&str> = text.split('\n').collect();
    let cursor_line = pos.line as usize;
    let indent = lines
        .get(cursor_line)
        .map(|l| l.len() - l.trim_start().len())
        .unwrap_or(0);

    let keys: &[String] = if is_project {
        &PROJECT_KEYS
    } else if indent == 0 {
        &ROOT_KEYS
    } else {
        // nearest zero-indent section above the cursor
        let section = lines[..cursor_line.min(lines.len())]
            .iter()
            .rev()
            .find(|l| !l.trim().is_empty() && !l.starts_with(' ') && l.trim_end().ends_with(':'))
            .map(|l| l.trim_end().trim_end_matches(':'))
            .unwrap_or("");
        match section {
            "models" => &MODEL_KEYS,
            "sources" => &SOURCE_KEYS,
            _ => &ROOT_KEYS,
        }
    };

    keys.iter()
        .map(|key| CompletionItem {
            label: key.clone(),
            detail: "dbt schema property".to_string(),
            documentation: String::new(),
            kind: completion_kind::VARIABLE,
            insert_text: format!("{key}: "),
            sort_text: key.clone(),
            data: None,
        })
        .collect()
}

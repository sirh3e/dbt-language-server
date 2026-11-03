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
    if path.file_name().is_some_and(|f| f == "profiles.yml") {
        return profiles_diagnostics(path, text);
    }
    // packages.yml has its own schema: syntax check only here — the
    // declared-vs-installed pass rides in from the context layer
    if path
        .file_name()
        .is_some_and(|f| f == "packages.yml" || f == "dependencies.yml")
    {
        return match cogitator_schemas::yaml::syntax_error(text) {
            Some((line, character, message)) => vec![diag(
                Range::zero_width(Position { line, character }),
                format!("yaml syntax error: {message}"),
                diagnostic_severity::ERROR,
            )],
            None => Vec::new(),
        };
    }
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
        Ok(()) if !is_project => unknown_key_diagnostics(text),
        Ok(()) => Vec::new(),
        Err(err) => vec![schema_error_diag(text, &err)],
    }
}

/// Unknown keys never reach serde (the prune drops them so a strict
/// union cannot reject a fine file) — so flag them HERE, from the
/// marked tree, with real positions and did-you-mean from the same key
/// lists completion uses.
fn unknown_key_diagnostics(text: &str) -> Vec<Diagnostic> {
    use cogitator_error::guidance::{closest_matches, Guidance};
    use cogitator_schemas::yaml::{as_str, entries, get, items, position_of};
    use saphyr::{LoadableYamlNode, MarkedYaml};

    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return Vec::new();
    };
    let Some(root) = docs.first() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut flag = |key: &MarkedYaml<'_>, known: &[String]| {
        let Some(name) = as_str(key) else { return };
        if known.iter().any(|k| k == name) {
            return;
        }
        let position = position_of(key);
        let guidance = Guidance::new(format!("'{name}' is not a dbt schema key here"))
            .suggest(closest_matches(name, known.iter().map(String::as_str)));
        out.push(diag(
            Range::zero_width(position),
            guidance.to_diagnostic(),
            diagnostic_severity::WARNING,
        ));
    };
    for (section, keys) in [("models", &*MODEL_KEYS), ("sources", &*SOURCE_KEYS)] {
        if let Some(list) = get(root, section) {
            for entry in items(list) {
                for (key, _) in entries(entry) {
                    flag(key, keys);
                }
            }
        }
    }
    for (key, _) in entries(root) {
        flag(key, &ROOT_KEYS);
    }
    out
}

/// Elm-grade schema errors: an unknown key gets a REAL position (its
/// line in the text) plus a did-you-mean from the keys serde expected;
/// anything opaque gets a clean headline instead of raw serde output.
fn schema_error_diag(text: &str, err: &cogitator_error::Error) -> Diagnostic {
    use cogitator_error::guidance::{closest_matches, Guidance};

    static UNKNOWN_FIELD: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"unknown field `([^`]+)`").expect("static regex"));
    static BACKTICKED: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"`([^`]+)`").expect("static regex"));

    let raw = err.to_string();
    if let Some(captures) = UNKNOWN_FIELD.captures(&raw) {
        let field = captures[1].to_string();
        let expected: Vec<String> = BACKTICKED
            .captures_iter(&raw)
            .map(|c| c[1].to_string())
            .filter(|k| *k != field)
            .collect();
        let range = position_of_key(text, &field).unwrap_or_default();
        let guidance = Guidance::new(format!("'{field}' is not a dbt schema key here"))
            .suggest(closest_matches(&field, expected.iter().map(String::as_str)));
        return diag(range, guidance.to_diagnostic(), diagnostic_severity::ERROR);
    }
    let cleaned = raw.strip_prefix("json: ").unwrap_or(&raw);
    diag(
        Range::default(),
        format!("this file does not match the dbt schema: {cleaned}"),
        diagnostic_severity::ERROR,
    )
}

/// The first line whose first token is `key:` — good enough to anchor
/// an unknown-field error where the author typed it.
fn position_of_key(text: &str, key: &str) -> Option<Range> {
    for (line_no, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(key) && trimmed[key.len()..].trim_start().starts_with(':') {
            let character = (line.len() - trimmed.len()) as u32;
            return Some(Range::zero_width(Position {
                line: line_no as u32,
                character,
            }));
        }
    }
    None
}

fn diag(range: Range, message: String, severity: i32) -> Diagnostic {
    Diagnostic::plain(range, message, severity, "dbt-yaml", "cogitator")
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

/// profiles.yml: connection-auth validation as diagnostics. Every profile
/// in the file is validated — the user is editing their own file; the
/// allowlist only gates what the dbt.profiles command exposes.
fn profiles_diagnostics(path: &Path, text: &str) -> Vec<Diagnostic> {
    if let Some((line, character, message)) = cogitator_schemas::yaml::syntax_error(text) {
        return vec![diag(
            Range::zero_width(Position { line, character }),
            format!("yaml syntax error: {message}"),
            diagnostic_severity::ERROR,
        )];
    }
    let inventory = crate::profiles::inventory(path, text, &crate::profiles::ProfileScope::All);
    let mut out = Vec::new();
    for profile in &inventory.profiles {
        for target in &profile.targets {
            for issue in &target.auth.issues {
                out.push(diag(
                    Range::zero_width(issue.position),
                    format!(
                        "auth ({}/{}, {}): {}",
                        profile.name,
                        target.name,
                        target.auth.method.as_str(),
                        issue.message
                    ),
                    diagnostic_severity::WARNING,
                ));
            }
        }
    }
    out
}

//! Uniqueness an entry declares outside its column tests: `constraints:`
//! (column- and model-level) and `dbt_utils.unique_combination_of_columns`.
//! Malformed entries are skipped: like tests, these are evidence only.

use crate::yaml;

/// The lower-cased `type:` of each item of a node's `constraints:`.
pub(super) fn constraint_types(node: &saphyr::MarkedYaml<'_>) -> Vec<String> {
    yaml::get(node, "constraints")
        .map(|list| {
            yaml::items(list)
                .filter_map(|c| yaml::get(c, "type").and_then(yaml::as_str))
                .map(|t| t.trim().to_ascii_lowercase())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// The column sets a model entry declares unique: `constraints` of type
/// `primary_key` or `unique` with `columns:`, and strict
/// `unique_combination_of_columns` tests (`tests:` or `data_tests:`,
/// arguments inline or under `arguments:`). Sorted, deduplicated.
pub(super) fn unique_keys_of(entry: &saphyr::MarkedYaml<'_>) -> Vec<Vec<String>> {
    let mut keys: Vec<Vec<String>> = Vec::new();
    if let Some(list) = yaml::get(entry, "constraints") {
        for item in yaml::items(list) {
            let kind = yaml::get(item, "type")
                .and_then(yaml::as_str)
                .map(|t| t.trim().to_ascii_lowercase());
            if !matches!(kind.as_deref(), Some("primary_key" | "unique")) {
                continue;
            }
            if let Some(cols) = yaml::get(item, "columns").map(names) {
                keys.push(cols);
            }
        }
    }
    for key in ["tests", "data_tests"] {
        let Some(list) = yaml::get(entry, key) else {
            continue;
        };
        for item in yaml::items(list) {
            if let Some(cols) = combination(item) {
                keys.push(cols);
            }
        }
    }
    let mut keys: Vec<Vec<String>> = keys
        .into_iter()
        .filter(|k| !k.is_empty())
        .map(|mut k| {
            k.sort();
            k.dedup();
            k
        })
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

fn names(node: &saphyr::MarkedYaml<'_>) -> Vec<String> {
    if let Some(one) = yaml::as_str(node) {
        return vec![one.trim().to_ascii_lowercase()];
    }
    yaml::items(node)
        .filter_map(yaml::as_str)
        .map(|n| n.trim().to_ascii_lowercase())
        .filter(|n| !n.is_empty())
        .collect()
}

/// One `- dbt_utils.unique_combination_of_columns: {combination_of_columns: [...]}`
/// test, strict only (no `where`, not `severity: warn`, not disabled).
fn combination(item: &saphyr::MarkedYaml<'_>) -> Option<Vec<String>> {
    let serde_json::Value::Object(map) = yaml::to_json(item) else {
        return None;
    };
    let (name, args) = map.iter().next()?;
    let short = name.rsplit('.').next().unwrap_or(name);
    if !short.eq_ignore_ascii_case("unique_combination_of_columns") {
        return None;
    }
    let setting = |key: &str| {
        args.get(key)
            .or_else(|| args.get("config").and_then(|c| c.get(key)))
    };
    let limited = setting("where").is_some_and(|w| !w.is_null())
        || setting("severity")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|s| s.eq_ignore_ascii_case("warn"))
        || setting("enabled") == Some(&serde_json::Value::Bool(false));
    if limited {
        return None;
    }
    let columns = args.get("combination_of_columns").or_else(|| {
        args.get("arguments")
            .and_then(|a| a.get("combination_of_columns"))
    })?;
    let list: Vec<String> = match columns {
        serde_json::Value::String(one) => vec![one.trim().to_ascii_lowercase()],
        serde_json::Value::Array(many) => many
            .iter()
            .filter_map(serde_json::Value::as_str)
            .map(|n| n.trim().to_ascii_lowercase())
            .collect(),
        _ => return None,
    };
    Some(list)
}

#[cfg(test)]
mod tests {
    use crate::properties::parse_properties;
    use std::path::Path;

    #[test]
    fn constraints_and_combination_tests_are_read() {
        let text = "version: 2\nmodels:\n  - name: m\n    constraints:\n      - type: primary_key\n        columns: [Id, day]\n      - type: check\n        columns: [x]\n    data_tests:\n      - dbt_utils.unique_combination_of_columns:\n          combination_of_columns: [a, b]\n      - dbt_utils.unique_combination_of_columns:\n          combination_of_columns: [c, d]\n          config: {severity: warn}\n    columns:\n      - name: id\n        constraints:\n          - type: primary_key\n      - name: code\n        constraints:\n          - type: not_null\n";
        let parsed = parse_properties(text, Path::new("m.yml")).unwrap();
        let model = &parsed.models[0];
        assert_eq!(
            model.unique_keys,
            vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["day".to_string(), "id".to_string()]
            ]
        );
        let id = &model.columns[0];
        assert!(id.proven_by("unique") && id.proven_by("not_null") && id.is_primary_key());
        let code = &model.columns[1];
        assert!(code.proven_by("not_null") && !code.proven_by("unique"));
    }
}

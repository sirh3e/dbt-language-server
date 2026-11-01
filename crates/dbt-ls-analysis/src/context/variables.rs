//! Project variables from dbt_project.yml (03 §4).

use std::collections::HashMap;
use std::path::Path;

use dbt_ls_lsp::Range;
use dbt_ls_schemas::project::ProjectFile;

use super::VariableDetails;

/// Project variables (03 §4).
pub(super) fn build_variables(
    root: &Path,
    project: &ProjectFile,
) -> HashMap<String, VariableDetails> {
    let mut vars = HashMap::new();
    let project_uri = root.join("dbt_project.yml");

    for (key, value) in &project.vars {
        if key == &project.name {
            continue;
        }
        let pos = project.var_positions.get(key).copied().unwrap_or_default();
        vars.insert(
            key.clone(),
            VariableDetails {
                name: key.clone(),
                value: value.clone(),
                uri: project_uri.clone(),
                range: Range::zero_width(pos),
            },
        );
    }

    // Project-scoped vars override, skipping null values (03 §4).
    if let Some(serde_json::Value::Object(scoped)) = project.vars.get(&project.name) {
        let positions = project.nested_var_positions.get(&project.name);
        for (key, value) in scoped {
            if value.is_null() {
                continue;
            }
            let pos = positions
                .and_then(|m| m.get(key))
                .copied()
                .unwrap_or_default();
            vars.insert(
                key.clone(),
                VariableDetails {
                    name: key.clone(),
                    value: value.clone(),
                    uri: project_uri.clone(),
                    range: Range::zero_width(pos),
                },
            );
        }
    }

    vars
}

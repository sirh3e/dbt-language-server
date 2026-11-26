//! "Add a unique test on <relation>.<key>": the YAML half of
//! `join.fanout`'s fixes. The finding names the relation and key in its
//! diagnostic's `data.subject`; this edits the YAML that documents that
//! column, line-wise through `yaml_text`. A column the YAML does not
//! list, a relation without YAML, or a `tests: [...]` flow list gets no
//! action: the edit would have to invent structure.

use std::path::{Path, PathBuf};

use cogitator_lsp::{
    CodeAction, Diagnostic, DiagnosticSubject, DocumentChange, TextDocumentEdit, WorkspaceEdit,
};
use cogitator_schemas::ColumnEntry;

use super::yaml_text::{append_to_list, entry_at, key_line};
use crate::DbtContext;

/// One action per context diagnostic whose subject is a key not known
/// to be unique, deduplicated by relation and column.
pub fn unique_test_actions(
    ctx: &DbtContext,
    diagnostics: &[Diagnostic],
    read: &dyn Fn(&Path) -> Option<String>,
) -> Vec<CodeAction> {
    let mut seen: Vec<(String, Option<String>, String)> = Vec::new();
    let mut out = Vec::new();
    for diagnostic in diagnostics {
        let Some(DiagnosticSubject::UniqueKey {
            relation,
            source,
            column,
        }) = diagnostic.data.as_ref().and_then(|d| d.subject.as_ref())
        else {
            continue;
        };
        let key = (
            relation.clone(),
            source.clone(),
            column.to_ascii_lowercase(),
        );
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        if let Some(action) = action(ctx, relation, source.as_deref(), column, diagnostic, read) {
            out.push(action);
        }
    }
    out
}

fn action(
    ctx: &DbtContext,
    relation: &str,
    source: Option<&str>,
    column: &str,
    diagnostic: &Diagnostic,
    read: &dyn Fn(&Path) -> Option<String>,
) -> Option<CodeAction> {
    let (path, columns, label) = documented(ctx, relation, source)?;
    let entry = columns
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(column))?;
    if entry.proven_by("unique") {
        return None;
    }
    let text = read(&path)?;
    let lines: Vec<&str> = text.lines().collect();
    let item = entry_at(&lines, entry.name_pos.line as usize)?;
    // a column using `tests:` keeps it; otherwise dbt's current key
    let list = if key_line(&lines, item, "tests").is_some() {
        "tests"
    } else {
        "data_tests"
    };
    let edit = append_to_list(
        &text,
        entry.name_pos.line as usize,
        list,
        &[vec!["- unique".to_string()]],
    )?;
    Some(CodeAction {
        title: format!("Add a unique test on {label}.{}", entry.name),
        kind: Some("quickfix"),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            document_changes: vec![DocumentChange::Edit(TextDocumentEdit::new(
                crate::features::file_uri(&path),
                vec![edit],
            ))],
        }),
        command: None,
        is_preferred: None,
        data: None,
    })
}

/// The YAML file documenting the relation, its columns, and its name
/// as the title says it.
fn documented<'a>(
    ctx: &'a DbtContext,
    relation: &str,
    source: Option<&str>,
) -> Option<(PathBuf, &'a [ColumnEntry], String)> {
    match source {
        None => {
            let details = ctx.model_detail_map.get(relation)?;
            Some((
                details.schema_uri.clone()?,
                details.columns.as_slice(),
                relation.to_string(),
            ))
        }
        Some(source) => {
            let table = ctx.source_detail_map.get(source)?.tables.get(relation)?;
            Some((
                table.uri.clone(),
                table.columns.as_slice(),
                format!("{source}.{relation}"),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_lsp::{diagnostic_severity, DiagnosticData, FixKind, Range};

    fn diagnostic(relation: &str, column: &str) -> Diagnostic {
        let mut d = Diagnostic::plain(
            Range::default(),
            "m".into(),
            diagnostic_severity::HINT,
            "join.fanout",
            "cogitator",
        );
        d.data = Some(DiagnosticData {
            fix: FixKind::MultiEdit { fixes: Vec::new() },
            token_range: Range::default(),
            suggestions: Vec::new(),
            subject: Some(DiagnosticSubject::UniqueKey {
                relation: relation.into(),
                source: None,
                column: column.into(),
            }),
        });
        d
    }

    fn ctx(yaml: &str) -> (DbtContext, PathBuf, String) {
        let path = PathBuf::from("/p/models/schema.yml");
        let parsed = cogitator_schemas::parse_properties(yaml, &path).expect("yaml");
        let mut ctx = DbtContext::default();
        for model in parsed.models {
            ctx.model_detail_map.insert(
                model.name.clone(),
                crate::context::ModelDetails {
                    schema_uri: Some(path.clone()),
                    columns: model.columns,
                    ..Default::default()
                },
            );
        }
        (ctx, path, yaml.to_string())
    }

    fn edited(yaml: &str, relation: &str, column: &str) -> Option<(String, String)> {
        let (ctx, path, text) = ctx(yaml);
        let read = |p: &Path| (p == path).then(|| text.clone());
        let actions = unique_test_actions(&ctx, &[diagnostic(relation, column)], &read);
        let action = actions.into_iter().next()?;
        let DocumentChange::Edit(edit) = &action.edit?.document_changes[0] else {
            return None;
        };
        let e = &edit.edits[0];
        let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
        let at = e.range.start.line as usize;
        for (i, l) in e.new_text.lines().enumerate() {
            lines.insert(at + i, l.to_string());
        }
        Some((action.title, lines.join("\n")))
    }

    #[test]
    fn adds_data_tests_under_an_undecorated_column() {
        let yaml = "version: 2\nmodels:\n  - name: orders\n    columns:\n      - name: customer_id\n        description: who\n";
        let (title, text) = edited(yaml, "orders", "customer_id").expect("action");
        assert_eq!(title, "Add a unique test on orders.customer_id");
        assert_eq!(
            text,
            "version: 2\nmodels:\n  - name: orders\n    columns:\n      - name: customer_id\n        description: who\n        data_tests:\n          - unique"
        );
    }

    #[test]
    fn appends_to_an_existing_tests_list() {
        let yaml = "version: 2\nmodels:\n  - name: orders\n    columns:\n      - name: id\n        tests:\n          - not_null\n";
        let (_, text) = edited(yaml, "orders", "id").expect("action");
        assert!(
            text.ends_with("        tests:\n          - not_null\n          - unique"),
            "{text}"
        );
    }

    #[test]
    fn nothing_for_an_undocumented_column_or_one_already_unique() {
        let yaml = "version: 2\nmodels:\n  - name: orders\n    columns:\n      - name: id\n        data_tests: [unique]\n";
        assert!(edited(yaml, "orders", "id").is_none());
        assert!(edited(yaml, "orders", "missing").is_none());
    }
}

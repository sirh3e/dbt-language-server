//! What changed in a model's column lineage between two versions of its
//! text: columns added, removed, and columns whose upstream sources
//! changed. Either side without lineage (a Jinja control block, SQL the
//! front end cannot bind) is said, never guessed around.

use serde::Serialize;

use crate::lineage::{ColumnLineage, ModelLineage, SourceRef};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<ChangedColumn>,
    /// Why one side has no lineage; then the lists are empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unknown: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangedColumn {
    pub name: String,
    pub before: Vec<String>,
    pub after: Vec<String>,
}

/// `orders.amount`, `source jaffle.payments.amount`, `? raw.x`.
pub fn source_label(source: &SourceRef) -> String {
    match source {
        SourceRef::Model { name, column } => format!("{name}.{column}"),
        SourceRef::Source {
            source,
            table,
            column,
        } => format!("source {source}.{table}.{column}"),
        SourceRef::Unknown { name, column } => format!("? {name}.{column}"),
    }
}

fn labels(column: &ColumnLineage) -> Vec<String> {
    let mut out: Vec<String> = column.sources.iter().map(source_label).collect();
    out.sort();
    out.dedup();
    out
}

/// One line per column, `name <- a.x, b.y`, in output order: the text
/// the lineage diff is a unified diff of.
pub fn lineage_text(lineage: Option<&ModelLineage>) -> String {
    let Some(lineage) = lineage else {
        return "-- no column lineage (Jinja control blocks or SQL the front end cannot bind)\n"
            .to_string();
    };
    lineage
        .columns
        .iter()
        .map(|c| format!("{} <- {}\n", c.name, labels(c).join(", ")))
        .collect()
}

pub fn lineage_diff(before: Option<&ModelLineage>, after: Option<&ModelLineage>) -> LineageDiff {
    let (Some(before), Some(after)) = (before, after) else {
        let side = if before.is_none() { "old" } else { "current" };
        return LineageDiff {
            unknown: Some(format!(
                "the {side} version has no column lineage (Jinja control blocks or SQL that does not bind)"
            )),
            ..Default::default()
        };
    };
    let find = |lineage: &'_ ModelLineage, name: &str| {
        lineage
            .columns
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
            .map(labels)
    };
    let mut diff = LineageDiff::default();
    for column in &after.columns {
        match find(before, &column.name) {
            None => diff.added.push(column.name.clone()),
            Some(old) => {
                let new = labels(column);
                if old != new {
                    diff.changed.push(ChangedColumn {
                        name: column.name.clone(),
                        before: old,
                        after: new,
                    });
                }
            }
        }
    }
    for column in &before.columns {
        if find(after, &column.name).is_none() {
            diff.removed.push(column.name.clone());
        }
    }
    diff
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, sources: &[(&str, &str)]) -> ColumnLineage {
        ColumnLineage {
            name: name.to_string(),
            sources: sources
                .iter()
                .map(|(m, c)| SourceRef::Model {
                    name: m.to_string(),
                    column: c.to_string(),
                })
                .collect(),
            direct: false,
            derived: false,
            end: None,
        }
    }

    fn model(columns: Vec<ColumnLineage>) -> ModelLineage {
        ModelLineage {
            columns,
            origin: None,
        }
    }

    #[test]
    fn added_removed_and_changed() {
        let before = model(vec![
            column("id", &[("orders", "id")]),
            column("amount", &[("payments", "amount")]),
            column("gone", &[]),
        ]);
        let after = model(vec![
            column("ID", &[("orders", "id")]),
            column("amount", &[("payments", "amount"), ("refunds", "amount")]),
            column("new_col", &[("orders", "status")]),
        ]);
        let diff = lineage_diff(Some(&before), Some(&after));
        assert_eq!(diff.added, ["new_col"]);
        assert_eq!(diff.removed, ["gone"]);
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].after, ["payments.amount", "refunds.amount"]);
        assert!(lineage_text(Some(&after)).starts_with("ID <- orders.id\n"));
    }

    #[test]
    fn a_side_without_lineage_is_said() {
        let diff = lineage_diff(None, Some(&model(vec![])));
        assert!(diff.unknown.unwrap().contains("old"));
        assert!(lineage_text(None).contains("no column lineage"));
    }
}

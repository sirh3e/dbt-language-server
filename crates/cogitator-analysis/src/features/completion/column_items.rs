//! Column items, ranked by where they come from. Every column of every
//! relation in scope is offered twice — `alias.col` and bare `col` — and
//! both rank above anything global (`sortText` tier `0`, before CTE names
//! `a`, buffer words `b`, relations `c`, functions `m`, keywords `z`):
//!
//! | sortText | item |
//! |---|---|
//! | `00…` | `alias.col` of a name two relations in scope have |
//! | `01…` | bare `col` that only one relation has |
//! | `02…` | `alias.col` of such a column |
//! | `03…` | bare `col` that is ambiguous (the database would refuse it) |
//!
//! A qualified item filters as its bare column name (`filterText`), so a
//! client that matches the typed word keeps it beside the bare item and
//! orders the two by `sortText` alone; the best item is `preselect`ed.

use cogitator_lsp::{completion_kind, CompletionItem};

use super::scope::sql::{self, Column, Lookup, SqlScope};
use super::scope::ScopeIndex;

/// A relation in scope with the columns it holds.
pub(super) struct InScope {
    pub(super) qualifier: String,
    pub(super) columns: Vec<Column>,
}

/// The relations of the cursor's SELECT: the parsed scope's, else every
/// FROM/JOIN of the document as the token scope reads them.
pub(super) fn in_scope(lookup: &Lookup<'_>, scope: Option<&SqlScope<'_>>) -> Vec<InScope> {
    if let Some(scope) = scope.filter(|s| !s.relations.is_empty()) {
        return scope
            .relations
            .iter()
            .map(|s| InScope {
                qualifier: s.qualifier.clone(),
                columns: scope.columns(lookup, s),
            })
            .collect();
    }
    let index = ScopeIndex::build(lookup.doc);
    let mut out: Vec<InScope> = Vec::new();
    let mut targets = Vec::new();
    // the token scope lists an aliased relation under its alias first,
    // then under its own name: keep the first
    for (key, target) in index.relations() {
        if targets.contains(&target) {
            continue;
        }
        targets.push(target);
        out.push(InScope {
            qualifier: key.clone(),
            columns: sql::target_columns(lookup, target),
        });
    }
    out
}

fn detail(column: &Column) -> String {
    match &column.data_type {
        Some(t) if !t.is_empty() => format!("{} · {t}", column.relation),
        _ => column.relation.clone(),
    }
}

fn field(label: String, column: &Column, sort: String) -> CompletionItem {
    CompletionItem {
        insert_text: label.clone(),
        filter_text: Some(column.name.clone()).filter(|name| *name != label),
        label,
        detail: detail(column),
        documentation: column.description.clone(),
        kind: completion_kind::FIELD,
        sort_text: sort,
        ..Default::default()
    }
}

/// After `qualifier.`: that relation's columns, in its own order.
pub(super) fn member_items(columns: &[Column]) -> Vec<CompletionItem> {
    let mut items: Vec<CompletionItem> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| field(c.name.clone(), c, format!("0{i:04}")))
        .collect();
    if let Some(first) = items.first_mut() {
        first.preselect = true;
    }
    items
}

/// A column position: every relation's columns, qualified and bare.
pub(super) fn column_items(scopes: &[InScope]) -> Vec<CompletionItem> {
    let holders = |name: &str| {
        scopes
            .iter()
            .filter(|s| s.columns.iter().any(|c| c.name.eq_ignore_ascii_case(name)))
            .count()
    };
    let mut items = Vec::new();
    let mut bare = std::collections::HashSet::new();
    for scope in scopes {
        for column in &scope.columns {
            let ambiguous = holders(&column.name) > 1;
            let qualified = format!("{}.{}", scope.qualifier, column.name);
            let tier = if ambiguous { '0' } else { '2' };
            items.push(field(
                qualified.clone(),
                column,
                format!("0{tier}{qualified}"),
            ));
            if bare.insert(column.name.to_lowercase()) {
                let tier = if ambiguous { '3' } else { '1' };
                let mut item = field(
                    column.name.clone(),
                    column,
                    format!("0{tier}{}", column.name),
                );
                if ambiguous {
                    item.detail = format!("ambiguous: {}", relations_of(scopes, &column.name));
                }
                items.push(item);
            }
        }
    }
    if let Some(best) = items
        .iter_mut()
        .min_by(|a, b| a.sort_text.cmp(&b.sort_text))
    {
        best.preselect = true;
    }
    items
}

fn relations_of(scopes: &[InScope], name: &str) -> String {
    scopes
        .iter()
        .filter(|s| s.columns.iter().any(|c| c.name.eq_ignore_ascii_case(name)))
        .map(|s| s.qualifier.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str, relation: &str) -> Column {
        Column {
            name: name.into(),
            relation: relation.into(),
            data_type: None,
            description: String::new(),
        }
    }

    #[test]
    fn ambiguous_names_rank_their_qualified_form_first() {
        let scopes = [
            InScope {
                qualifier: "o".into(),
                columns: vec![
                    column("order_id", "orders"),
                    column("customer_id", "orders"),
                ],
            },
            InScope {
                qualifier: "c".into(),
                columns: vec![column("customer_id", "customers")],
            },
        ];
        let mut items = column_items(&scopes);
        items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "c.customer_id",
                "o.customer_id",
                "order_id",
                "o.order_id",
                "customer_id",
            ]
        );
        assert!(items[0].preselect);
        assert_eq!(items[0].filter_text.as_deref(), Some("customer_id"));
        assert_eq!(items[4].detail, "ambiguous: o, c");
    }
}

//! Column lineage (FEATURE_REQUESTS): which upstream columns feed each
//! output column of a model. Pure over the document text; exposed through
//! the `dbt.columnLineage` workspace command.
//!
//! Pipeline: the already-parsed token stream locates the Jinja constructs →
//! the SQL is rebuilt with *named* placeholders (`__dbt_rel_N__` ↔ ref/source
//! targets) → sqlparser parses the masked statement → CTE-aware resolution
//! maps every projected column back to models, sources or CTE outputs.
//! Templates with Jinja control blocks decline (the 07 §3.2 confidence rule:
//! blanked branches would fabricate lineage).

use std::collections::HashMap;

use cogitator_dialects::Dialect;
use cogitator_parser::{JinjaKind, ParsedDocument, Token, TokenKind};
use serde::Serialize;
use sqlparser::ast::{Expr, Select, SelectItem, SetExpr, Statement, TableFactor, TableWithJoins};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum SourceRef {
    /// Another dbt model (`{{ ref('…') }}`).
    Model { name: String, column: String },
    /// A dbt source table (`{{ source('a', 'b') }}`).
    Source {
        source: String,
        table: String,
        column: String,
    },
    /// A relation the file does not define (raw table, outer CTE, …).
    Unknown { name: String, column: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnLineage {
    pub name: String,
    pub sources: Vec<SourceRef>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelLineage {
    pub columns: Vec<ColumnLineage>,
}

/// Relation targets a FROM/JOIN item can resolve to.
#[derive(Debug, Clone)]
enum Relation {
    Model(String),
    Source(String, String),
    Cte(String),
    Unknown(String),
}

/// Entry point: lineage for one model document. `None` when the template
/// declines (control blocks) or the masked SQL does not parse.
pub fn column_lineage(doc: &ParsedDocument, dialect: &Dialect) -> Option<ModelLineage> {
    let (masked, placeholders) = mask_for_lineage(doc)?;
    let statements = sqlparser::parser::Parser::parse_sql(
        cogitator_parser::backend::sql_dialect(dialect).as_ref(),
        &masked,
    )
    .ok()?;
    let query = statements.iter().find_map(|s| match s {
        Statement::Query(q) => Some(q),
        _ => None,
    })?;

    // CTEs resolve in declaration order and may reference earlier ones.
    let mut ctes: HashMap<String, Vec<ColumnLineage>> = HashMap::new();
    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            let name = cte.alias.name.value.clone();
            if let SetExpr::Select(select) = cte.query.body.as_ref() {
                let columns = select_lineage(select, &placeholders, &ctes);
                ctes.insert(name, columns);
            }
        }
    }

    let SetExpr::Select(select) = query.body.as_ref() else {
        return None; // set operations: take nothing rather than guess (v1)
    };
    Some(ModelLineage {
        columns: select_lineage(select, &placeholders, &ctes),
    })
}

// ---------------------------------------------------------------------------
// Masking
// ---------------------------------------------------------------------------

/// Rebuild the SQL with named placeholder identifiers for Jinja expressions.
fn mask_for_lineage(doc: &ParsedDocument) -> Option<(String, HashMap<String, Relation>)> {
    let text: &str = &doc.text;
    let mut masked = String::with_capacity(text.len());
    let mut placeholders = HashMap::new();
    let mut counter = 0usize;

    let mut pos = 0usize;
    let mut i = 0;
    while i < doc.tokens.len() {
        let token = &doc.tokens[i];
        if let TokenKind::JinjaOpen(kind) = token.kind {
            let close = doc.tokens[i..]
                .iter()
                .position(|t| matches!(t.kind, TokenKind::JinjaClose(k) if k == kind))
                .map(|off| i + off)?;
            let (start, end) = (token.start as usize, doc.tokens[close].end as usize);
            masked.push_str(&text[pos..start.max(pos).min(text.len())]);
            match kind {
                JinjaKind::Expr => {
                    let region = &doc.tokens[i..=close];
                    let relation = relation_in_region(doc, region);
                    let placeholder = format!("__dbt_rel_{counter}__");
                    counter += 1;
                    if let Some(relation) = relation {
                        placeholders.insert(placeholder.clone(), relation);
                    }
                    masked.push_str(&placeholder);
                }
                JinjaKind::Stmt => {
                    // config/set blocks blank; control flow declines (07 §3.2)
                    let tag = doc.tokens[i + 1..close]
                        .iter()
                        .find(|t| t.kind == TokenKind::Ident || t.kind == TokenKind::Config)
                        .map(|t| doc.literal(t))
                        .unwrap_or("");
                    if !matches!(tag, "config" | "set" | "endset") {
                        return None;
                    }
                }
            }
            pos = end;
            i = close + 1;
            continue;
        }
        i += 1;
    }
    masked.push_str(&text[pos.min(text.len())..]);

    // comments are not tokens; strip them from the rebuilt text
    let masked = strip_jinja_comments(&masked);
    Some((masked, placeholders))
}

fn relation_in_region(doc: &ParsedDocument, region: &[Token]) -> Option<Relation> {
    let reference = region.iter().find(|t| t.kind == TokenKind::Ref);
    if let Some(reference) = reference {
        return Some(Relation::Model(doc.literal(reference).to_string()));
    }
    let source = region.iter().find(|t| t.kind == TokenKind::SourceName)?;
    let table = region.iter().find(|t| t.kind == TokenKind::SourceTable)?;
    Some(Relation::Source(
        doc.literal(source).to_string(),
        doc.literal(table).to_string(),
    ))
}

fn strip_jinja_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("{#") {
        out.push_str(&rest[..start]);
        match rest[start..].find("#}") {
            Some(end) => rest = &rest[start + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

fn select_lineage(
    select: &Select,
    placeholders: &HashMap<String, Relation>,
    ctes: &HashMap<String, Vec<ColumnLineage>>,
) -> Vec<ColumnLineage> {
    let scope = scope_of(&select.from, placeholders, ctes);

    select
        .projection
        .iter()
        .filter_map(|item| match item {
            SelectItem::UnnamedExpr(expr) => {
                let name = output_name(expr)?;
                Some(ColumnLineage {
                    name,
                    sources: expr_sources(expr, &scope, ctes),
                })
            }
            SelectItem::ExprWithAlias { expr, alias } => Some(ColumnLineage {
                name: alias.value.clone(),
                sources: expr_sources(expr, &scope, ctes),
            }),
            SelectItem::Wildcard(_) => Some(ColumnLineage {
                name: "*".to_string(),
                sources: scope
                    .values()
                    .flat_map(|rel| resolve(rel, "*", ctes))
                    .collect(),
            }),
            SelectItem::ExprWithAliases { .. } => None, // multi-alias items: skip (v1)
            SelectItem::QualifiedWildcard(kind, _) => {
                let alias = kind.to_string();
                let alias = alias.trim_end_matches(".*");
                scope.get(alias).map(|rel| ColumnLineage {
                    name: format!("{alias}.*"),
                    sources: resolve(rel, "*", ctes),
                })
            }
        })
        .collect()
}

/// alias/name → relation for every FROM/JOIN item of one SELECT.
fn scope_of(
    from: &[TableWithJoins],
    placeholders: &HashMap<String, Relation>,
    ctes: &HashMap<String, Vec<ColumnLineage>>,
) -> HashMap<String, Relation> {
    let mut scope = HashMap::new();
    let mut add = |factor: &TableFactor| {
        if let TableFactor::Table { name, alias, .. } = factor {
            let table = name
                .0
                .last()
                .map(|part| part.to_string().trim_matches('"').to_string())
                .unwrap_or_default();
            let relation = placeholders
                .get(&table)
                .cloned()
                .or_else(|| {
                    ctes.contains_key(&table)
                        .then(|| Relation::Cte(table.clone()))
                })
                .unwrap_or(Relation::Unknown(table.clone()));
            let key = alias
                .as_ref()
                .map(|a| a.name.value.clone())
                .unwrap_or(table);
            scope.insert(key, relation);
        }
    };
    for item in from {
        add(&item.relation);
        for join in &item.joins {
            add(&join.relation);
        }
    }
    scope
}

fn output_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(ident) => Some(ident.value.clone()),
        Expr::CompoundIdentifier(parts) => parts.last().map(|p| p.value.clone()),
        _ => None,
    }
}

/// Every column reference inside `expr`, resolved through the scope.
fn expr_sources(
    expr: &Expr,
    scope: &HashMap<String, Relation>,
    ctes: &HashMap<String, Vec<ColumnLineage>>,
) -> Vec<SourceRef> {
    let mut refs: Vec<(Option<String>, String)> = Vec::new();
    collect_column_refs(expr, &mut refs);

    let mut out = Vec::new();
    for (qualifier, column) in refs {
        match qualifier {
            Some(q) => {
                if let Some(rel) = scope.get(&q) {
                    out.extend(resolve(rel, &column, ctes));
                } else {
                    out.push(SourceRef::Unknown { name: q, column });
                }
            }
            None => match scope.len() {
                1 => out.extend(resolve(
                    scope.values().next().expect("len 1"),
                    &column,
                    ctes,
                )),
                _ => out.extend(scope.values().flat_map(|rel| resolve(rel, &column, ctes))),
            },
        }
    }
    out.dedup();
    out
}

/// Resolve one (relation, column); CTE references resolve transitively into
/// the CTE's own lineage.
fn resolve(
    relation: &Relation,
    column: &str,
    ctes: &HashMap<String, Vec<ColumnLineage>>,
) -> Vec<SourceRef> {
    match relation {
        Relation::Model(name) => {
            vec![SourceRef::Model {
                name: name.clone(),
                column: column.to_string(),
            }]
        }
        Relation::Source(source, table) => vec![SourceRef::Source {
            source: source.clone(),
            table: table.clone(),
            column: column.to_string(),
        }],
        Relation::Unknown(name) => {
            vec![SourceRef::Unknown {
                name: name.clone(),
                column: column.to_string(),
            }]
        }
        Relation::Cte(name) => {
            let Some(columns) = ctes.get(name) else {
                return vec![SourceRef::Unknown {
                    name: name.clone(),
                    column: column.to_string(),
                }];
            };
            // `*` flattens the whole CTE; a named lookup also matches the
            // CTE's own `*` column, substituting the requested name so
            // `select * from x` CTEs keep column granularity.
            let matched: Vec<SourceRef> = columns
                .iter()
                .filter(|c| column == "*" || c.name == column || c.name == "*")
                .flat_map(|c| c.sources.iter().cloned())
                .map(|source| substitute_star(source, column))
                .collect();
            if matched.is_empty() {
                vec![SourceRef::Unknown {
                    name: name.clone(),
                    column: column.to_string(),
                }]
            } else {
                matched
            }
        }
    }
}

/// Minimal expression walker collecting (qualifier, column) references.
/// (sqlparser's `visitor` feature is unusable from crates.io at 0.63 — its
/// derive crate version is unpublished — so the common variants are walked
/// by hand; unhandled variants contribute nothing.)
fn collect_column_refs(expr: &Expr, out: &mut Vec<(Option<String>, String)>) {
    match expr {
        Expr::Identifier(ident) => out.push((None, ident.value.clone())),
        Expr::CompoundIdentifier(parts) if parts.len() >= 2 => out.push((
            Some(
                parts[parts.len() - 2]
                    .to_string()
                    .trim_matches('"')
                    .to_string(),
            ),
            parts
                .last()
                .expect("len >= 2")
                .to_string()
                .trim_matches('"')
                .to_string(),
        )),
        Expr::BinaryOp { left, right, .. } => {
            collect_column_refs(left, out);
            collect_column_refs(right, out);
        }
        Expr::UnaryOp { expr, .. }
        | Expr::Nested(expr)
        | Expr::Cast { expr, .. }
        | Expr::IsNull(expr)
        | Expr::IsNotNull(expr)
        | Expr::Collate { expr, .. } => collect_column_refs(expr, out),
        Expr::Function(function) => {
            use sqlparser::ast::{FunctionArg, FunctionArgExpr, FunctionArguments};
            if let FunctionArguments::List(list) = &function.args {
                for arg in &list.args {
                    let value = match arg {
                        FunctionArg::Named { arg, .. } | FunctionArg::Unnamed(arg) => arg,
                        _ => continue,
                    };
                    if let FunctionArgExpr::Expr(e) = value {
                        collect_column_refs(e, out);
                    }
                }
            }
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => {
            if let Some(operand) = operand {
                collect_column_refs(operand, out);
            }
            for when in conditions {
                collect_column_refs(&when.condition, out);
                collect_column_refs(&when.result, out);
            }
            if let Some(else_result) = else_result {
                collect_column_refs(else_result, out);
            }
        }
        Expr::InList { expr, list, .. } => {
            collect_column_refs(expr, out);
            for item in list {
                collect_column_refs(item, out);
            }
        }
        Expr::Between {
            expr, low, high, ..
        } => {
            collect_column_refs(expr, out);
            collect_column_refs(low, out);
            collect_column_refs(high, out);
        }
        Expr::Like { expr, pattern, .. } | Expr::ILike { expr, pattern, .. } => {
            collect_column_refs(expr, out);
            collect_column_refs(pattern, out);
        }
        Expr::Tuple(items) => {
            for item in items {
                collect_column_refs(item, out);
            }
        }
        _ => {}
    }
}

/// Replace a `*` source column with the concretely requested column.
fn substitute_star(source: SourceRef, requested: &str) -> SourceRef {
    if requested == "*" {
        return source;
    }
    match source {
        SourceRef::Model { name, column } if column == "*" => SourceRef::Model {
            name,
            column: requested.to_string(),
        },
        SourceRef::Source {
            source,
            table,
            column,
        } if column == "*" => SourceRef::Source {
            source,
            table,
            column: requested.to_string(),
        },
        SourceRef::Unknown { name, column } if column == "*" => SourceRef::Unknown {
            name,
            column: requested.to_string(),
        },
        other => other,
    }
}

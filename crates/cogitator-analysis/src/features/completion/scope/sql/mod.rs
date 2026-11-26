//! The cursor's scope read with sqlparser: the statement the cursor is
//! in, Jinja masked ([`text`]), parsed — whole when it parses, else cut
//! back at the cursor — and walked for the cursor's clause and the FROM
//! items of its SELECT ([`walk`]). [`columns`] says what each item holds.
//! `None` when nothing parses; completion then falls back to the token
//! scope of [`super::ScopeIndex`].

mod columns;
mod text;
mod walk;

#[cfg(test)]
mod tests;

use std::borrow::Cow;
use std::collections::HashMap;

use cogitator_dialects::Dialect;
use cogitator_parser::{ParsedDocument, TokenKind};
use sqlparser::ast::{Ident, ObjectName, Query, TableAlias};
use sqlparser::parser::Parser;

use super::RelationTarget;
use crate::Document;
use text::Text;

pub(crate) use columns::{target_columns, Column, Lookup};
pub(crate) use walk::Clause;

/// What a FROM item reads.
#[derive(Debug, Clone)]
pub(crate) enum Relation {
    Target(RelationTarget),
    /// A derived table: `(select …) alias`.
    Derived(Box<Query>),
}

/// One FROM item of the cursor's SELECT.
#[derive(Debug, Clone)]
pub(crate) struct Scoped {
    /// What `qualifier.col` names it by: its alias, else its own name.
    pub(crate) qualifier: String,
    pub(crate) relation: Relation,
}

/// The cursor's clause and what its SELECT reads.
pub(crate) struct SqlScope<'d> {
    pub(crate) clause: Clause,
    pub(crate) relations: Vec<Scoped>,
    /// CTE name (lowercase) → its query, from wherever it parsed.
    ctes: HashMap<String, Query>,
    namer: Namer<'d>,
}

impl SqlScope<'_> {
    /// The CTE names this statement defines, lowercase.
    pub(crate) fn has_cte(&self, name: &str) -> bool {
        self.ctes.contains_key(&name.to_lowercase())
    }
}

/// Resolves FROM names: masked Jinja to the dbt relation it renders.
struct Namer<'d> {
    text: Text,
    parsed: Cow<'d, ParsedDocument>,
    cte_names: Vec<String>,
}

/// The cursor's scope, or `None` when its statement does not parse at
/// all. `offset` is the cursor's byte offset in `doc.text`.
pub(crate) fn analyze<'d>(
    doc: &'d Document,
    dialect: &Dialect,
    offset: usize,
) -> Option<SqlScope<'d>> {
    let parsed = text::current_parse(doc, dialect)?;
    let masked = text::masked(&parsed, dialect)?;
    let text = Text::new(&masked, offset);
    let sql_dialect = cogitator_parser::pipeline::sql_dialect(dialect);
    let tokens = text.tokens(&*sql_dialect)?;
    let statement = text::statement(&tokens, &text);
    let regions: Vec<(usize, usize)> = text::query_regions(&tokens)
        .into_iter()
        .filter(|&(lo, hi)| statement.0 <= lo && hi <= statement.1)
        .collect();
    let region = text::innermost(&regions, text.cursor, text.ph_end(), statement);

    let parse = |sql: &str| {
        Parser::new(&*sql_dialect)
            .try_with_sql(sql)
            .ok()?
            .parse_query()
            .ok()
    };
    let query = text::variants(&text, &tokens, region)
        .iter()
        .filter_map(|sql| parse(sql))
        .find(walk::has)?;

    let mut ctes = HashMap::new();
    collect_ctes(&query, &mut ctes);
    if let Some(whole) = text::variants(&text, &tokens, statement)
        .iter()
        .find_map(|sql| parse(sql))
    {
        collect_ctes(&whole, &mut ctes);
    }
    // a statement that does not parse whole: each CTE body on its own
    for (name, def) in &parsed.def_tokens {
        if ctes.contains_key(&name.to_lowercase()) {
            continue;
        }
        let at = def.start as usize;
        let at = if at >= text.cursor {
            at + text::PH.len()
        } else {
            at
        };
        let body = regions.iter().find(|&&(lo, _)| lo > at);
        if let Some(q) = body.and_then(|&b| {
            text::variants(&text, &tokens, b)
                .iter()
                .find_map(|sql| parse(sql))
        }) {
            ctes.insert(name.to_lowercase(), *q);
        }
    }

    let namer = Namer {
        cte_names: ctes.keys().cloned().collect(),
        text,
        parsed,
    };
    let found = walk::find(&query)?;
    let relations = found
        .select
        .map(|select| namer.scoped(&walk::items(select)))
        .unwrap_or_default();
    Some(SqlScope {
        clause: found.clause,
        relations,
        ctes,
        namer,
    })
}

fn collect_ctes(query: &Query, out: &mut HashMap<String, Query>) {
    if let Some(with) = &query.with {
        for cte in &with.cte_tables {
            out.entry(cte.alias.name.value.to_lowercase())
                .or_insert_with(|| (*cte.query).clone());
        }
    }
}

impl Namer<'_> {
    fn scoped(&self, items: &[walk::Item<'_>]) -> Vec<Scoped> {
        items
            .iter()
            .map(|item| match item {
                walk::Item::Table { name, alias } => {
                    let target = self.target(name);
                    Scoped {
                        qualifier: alias_name(*alias)
                            .unwrap_or_else(|| default_qualifier(name, &target)),
                        relation: Relation::Target(target),
                    }
                }
                walk::Item::Derived { query, alias } => Scoped {
                    qualifier: alias.name.value.clone(),
                    relation: Relation::Derived(Box::new((*query).clone())),
                },
            })
            .collect()
    }

    fn target(&self, name: &ObjectName) -> RelationTarget {
        let idents: Vec<&Ident> = name.0.iter().filter_map(|p| p.as_ident()).collect();
        if let [ident] = idents[..] {
            if is_masked(&ident.value) {
                if let Some(target) = self.jinja_target(ident) {
                    return target;
                }
            }
        }
        let last = idents.last().map(|i| i.value.as_str()).unwrap_or_default();
        if self.cte_names.iter().any(|c| c.eq_ignore_ascii_case(last)) {
            RelationTarget::Cte(last.to_string())
        } else {
            RelationTarget::Unknown(name.to_string())
        }
    }

    /// The dbt relation of a masked `{{ … }}`: its ref or source tokens.
    fn jinja_target(&self, ident: &Ident) -> Option<RelationTarget> {
        let start = self.text.doc_offset(self.text.offset(ident.span.start)?);
        let end = start + ident.value.len();
        let tokens = self
            .parsed
            .tokens
            .iter()
            .filter(|t| (start..end).contains(&(t.start as usize)));
        let mut source = None;
        for token in tokens {
            let literal = || self.parsed.literal(token).to_string();
            match token.kind {
                TokenKind::Ref => return Some(RelationTarget::Model(literal())),
                TokenKind::SourceName => source = Some(literal()),
                TokenKind::SourceTable => {
                    return Some(RelationTarget::SourceTable {
                        source: source.unwrap_or_default(),
                        table: literal(),
                    })
                }
                _ => {}
            }
        }
        self.parsed
            .text
            .get(start..end)
            .map(|s| RelationTarget::Unknown(s.to_string()))
    }
}

fn alias_name(alias: Option<&TableAlias>) -> Option<String> {
    alias.map(|a| a.name.value.clone())
}

/// An unaliased FROM item is named by its relation's own name.
fn default_qualifier(name: &ObjectName, target: &RelationTarget) -> String {
    match target {
        RelationTarget::Model(n) | RelationTarget::Cte(n) => n.clone(),
        RelationTarget::SourceTable { table, .. } => table.clone(),
        RelationTarget::Unknown(_) => name
            .0
            .last()
            .and_then(|p| p.as_ident())
            .map(|i| i.value.clone())
            .unwrap_or_default(),
    }
}

/// `_jjj…`: what the masking makes of a `{{ … }}`.
fn is_masked(value: &str) -> bool {
    value.len() > 1 && value.starts_with('_') && value[1..].bytes().all(|b| b == b'j')
}

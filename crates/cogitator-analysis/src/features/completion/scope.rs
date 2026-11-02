//! Document FROM/JOIN scope: token-based alias → relation resolution.
//! Deliberately not the lineage SQL parse — this must work on mid-edit
//! buffers where full parsing declines.

use std::path::PathBuf;

use cogitator_lsp::Position;
use cogitator_parser::{Keyword, TokenKind};

use crate::context::DbtContext;

use super::Document;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationTarget {
    Model(String),
    SourceTable { source: String, table: String },
    Cte(String),
    Unknown(String),
}

#[derive(Debug, Default)]
pub struct ScopeIndex {
    relations: Vec<(String, RelationTarget)>,
}

impl ScopeIndex {
    pub fn build(doc: &Document) -> Self {
        let tokens = &doc.parsed.tokens;
        let mut relations = Vec::new();

        let mut i = 0;
        while i < tokens.len() {
            if !matches!(
                tokens[i].kind,
                TokenKind::Word(Keyword::FROM) | TokenKind::Word(Keyword::JOIN)
            ) {
                i += 1;
                continue;
            }
            // relation: a jinja ref/source region or a bare name
            let mut j = i + 1;
            let mut source_name: Option<String> = None;
            let mut target: Option<RelationTarget> = None;
            let mut in_jinja = false;
            while j < tokens.len() && target.is_none() {
                match tokens[j].kind {
                    TokenKind::JinjaOpen(_) => in_jinja = true,
                    TokenKind::JinjaClose(_) => in_jinja = false,
                    TokenKind::Symbol('(')
                    | TokenKind::Symbol(')')
                    | TokenKind::Symbol(',')
                    | TokenKind::StringLit => {}
                    // inside `{{ … }}` only refined dbt tokens matter — the
                    // callee ident (`ref`, `source`) is not a relation name
                    TokenKind::Ident | TokenKind::Word(Keyword::NoKeyword) if in_jinja => {}
                    TokenKind::Ref => {
                        target = Some(RelationTarget::Model(
                            doc.parsed.literal(&tokens[j]).to_string(),
                        ));
                    }
                    TokenKind::SourceName => {
                        source_name = Some(doc.parsed.literal(&tokens[j]).to_string());
                    }
                    TokenKind::SourceTable => {
                        target = Some(RelationTarget::SourceTable {
                            source: source_name.clone().unwrap_or_default(),
                            table: doc.parsed.literal(&tokens[j]).to_string(),
                        });
                    }
                    TokenKind::Word(Keyword::NoKeyword) | TokenKind::Ident => {
                        let name = doc.parsed.literal(&tokens[j]).to_string();
                        target = Some(if doc.parsed.def_tokens.contains_key(name.as_str()) {
                            RelationTarget::Cte(name)
                        } else {
                            RelationTarget::Unknown(name)
                        });
                    }
                    _ => break, // keywords/operators: malformed FROM — give up
                }
                j += 1;
            }
            let Some(target) = target else {
                i = j;
                continue;
            };

            // skip the tail of the jinja call (`') }}`), then optional AS + alias
            while j < tokens.len()
                && matches!(
                    tokens[j].kind,
                    TokenKind::JinjaClose(_)
                        | TokenKind::Symbol(')')
                        | TokenKind::Symbol('(')
                        | TokenKind::StringLit
                )
            {
                j += 1;
            }
            if j < tokens.len() && tokens[j].kind == TokenKind::Word(Keyword::AS) {
                j += 1;
            }
            let alias = tokens.get(j).and_then(|t| {
                matches!(t.kind, TokenKind::Word(Keyword::NoKeyword))
                    .then(|| doc.parsed.literal(t).to_string())
            });

            let name = match &target {
                RelationTarget::Model(n) | RelationTarget::Cte(n) | RelationTarget::Unknown(n) => {
                    n.clone()
                }
                RelationTarget::SourceTable { table, .. } => table.clone(),
            };
            if let Some(alias) = alias {
                relations.push((alias, target.clone()));
            }
            relations.push((name, target));
            i = j + 1;
        }

        ScopeIndex { relations }
    }

    pub fn resolve(&self, qualifier: &str) -> Option<&RelationTarget> {
        self.relations
            .iter()
            .find(|(key, _)| key == qualifier)
            .map(|(_, target)| target)
    }

    pub fn targets(&self) -> impl Iterator<Item = &RelationTarget> {
        self.relations.iter().map(|(_, t)| t)
    }
}

/// Approximate columns of a CTE: identifiers in its `select … from` window.
pub fn cte_columns(doc: &Document, name: &str) -> Vec<String> {
    let Some(def) = doc.parsed.def_tokens.get(name) else {
        return Vec::new();
    };
    let tokens = &doc.parsed.tokens;
    let Some(start) = tokens.iter().position(|t| t.start == def.start) else {
        return Vec::new();
    };
    // skip to the opening paren of the CTE body
    let Some(open) = tokens[start..]
        .iter()
        .position(|t| t.kind == TokenKind::Symbol('('))
    else {
        return Vec::new();
    };
    let mut depth = 0i32;
    let mut in_select = false;
    let mut columns = Vec::new();
    for token in &tokens[start + open..] {
        match token.kind {
            TokenKind::Symbol('(') => depth += 1,
            TokenKind::Symbol(')') => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            TokenKind::Word(Keyword::SELECT) if depth == 1 => in_select = true,
            TokenKind::Word(Keyword::FROM) if depth == 1 => in_select = false,
            TokenKind::Word(Keyword::NoKeyword) if in_select && depth == 1 => {
                let literal = doc.parsed.literal(token).to_string();
                if !columns.contains(&literal) {
                    columns.push(literal);
                }
            }
            _ => {}
        }
    }
    columns
}

/// A column resolved against the document's relations (hover/definition).
#[derive(Debug, Clone)]
pub(crate) struct ColumnHit {
    pub(crate) relation: String,
    pub(crate) description: String,
    pub(crate) yaml: Option<(PathBuf, Position)>,
}

/// Resolve a bare identifier as a documented column: the FROM/JOIN scope
/// first, then every relation the document refs/sources.
pub(crate) fn resolve_column(
    ctx: &DbtContext,
    doc: &super::Document,
    name: &str,
) -> Option<ColumnHit> {
    let scope = ScopeIndex::build(doc);
    let mut targets: Vec<RelationTarget> = scope.targets().cloned().collect();
    for token in &doc.parsed.tokens {
        match token.kind {
            TokenKind::Ref => {
                targets.push(RelationTarget::Model(doc.parsed.literal(token).to_string()))
            }
            TokenKind::SourceTable => {
                // the source name sits a few tokens back (02 §3.6 window)
                if let Some(idx) = doc
                    .parsed
                    .tokens
                    .iter()
                    .position(|t| t.start == token.start)
                {
                    if let Some(source) = doc.parsed.lookback_find(idx, TokenKind::SourceName, 4) {
                        targets.push(RelationTarget::SourceTable {
                            source: doc.parsed.literal(source).to_string(),
                            table: doc.parsed.literal(token).to_string(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    for target in &targets {
        match target {
            RelationTarget::Model(model) => {
                if let Some(details) = ctx.model_detail_map.get(model) {
                    if let Some(column) = details.columns.iter().find(|c| c.name == name) {
                        return Some(ColumnHit {
                            relation: model.clone(),
                            description: column.description.clone(),
                            yaml: details
                                .schema_uri
                                .clone()
                                .map(|path| (path, column.name_pos)),
                        });
                    }
                }
            }
            RelationTarget::SourceTable { source, table } => {
                if let Some(t) = ctx
                    .source_detail_map
                    .get(source)
                    .and_then(|s| s.tables.get(table))
                {
                    if let Some(column) = t.columns.iter().find(|c| c.name == name) {
                        return Some(ColumnHit {
                            relation: format!("{source}.{table}"),
                            description: column.description.clone(),
                            yaml: Some((t.uri.clone(), column.name_pos)),
                        });
                    }
                }
            }
            _ => {}
        }
    }
    None
}

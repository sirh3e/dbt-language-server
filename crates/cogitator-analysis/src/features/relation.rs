//! dbt.relation: the database.schema.identifier coordinates of the node
//! under the cursor — a `source()` table, a `ref()` target, or the file's
//! own model. dbt *defaults* only: source schema falls back to the source
//! name, identifiers to the declared name, and model coordinates come from
//! the active profiles target. Custom `generate_schema_name`-style macros
//! and `models:` config trees (`+schema`/`+database`) are not evaluated.

use cogitator_lsp::Position;
use cogitator_parser::TokenKind;
use serde::Serialize;

use super::{model_name_from_uri, DbtContext, Document};

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RelationKind {
    Model,
    Source,
}

/// The dbt.relation result. Options are explicit `null` on the wire.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelationInfo {
    pub kind: RelationKind,
    /// Model name, or the source table's declared name.
    pub name: String,
    /// The source's name; `null` for models.
    pub source: Option<String>,
    pub database: Option<String>,
    pub schema: Option<String>,
    pub identifier: String,
    /// Dot-join of the known parts, unquoted.
    pub relation: String,
}

/// The active profiles target's coordinates, handed in by the shell.
#[derive(Debug, Default)]
pub struct TargetRelation {
    pub database: Option<String>,
    pub schema: Option<String>,
}

/// `pos` targets a `source()` table or `ref()` under the cursor; without a
/// position (or on any other token) the file's own model answers.
pub fn relation_info(
    ctx: &DbtContext,
    doc: Option<&Document>,
    uri: &str,
    pos: Option<Position>,
    target: Option<&TargetRelation>,
) -> Option<RelationInfo> {
    if let (Some(doc), Some(pos)) = (doc, pos) {
        if let Some(info) = relation_at_cursor(ctx, doc, pos, target) {
            return Some(info);
        }
    }
    model_relation(ctx, &model_name_from_uri(uri)?, target)
}

fn relation_at_cursor(
    ctx: &DbtContext,
    doc: &Document,
    pos: Position,
    target: Option<&TargetRelation>,
) -> Option<RelationInfo> {
    let parsed = &doc.parsed;
    let idx = parsed.find_token_at_cursor(pos.line, pos.character)?;
    let token = &parsed.tokens[idx];
    let literal = parsed.literal(token);
    match token.kind {
        TokenKind::Ref => model_relation(ctx, literal, target),
        TokenKind::SourceTable => {
            let source = parsed.lookback_find(idx, TokenKind::SourceName, 4)?;
            source_relation(ctx, parsed.literal(source), literal, target)
        }
        _ => None,
    }
}

fn model_relation(
    ctx: &DbtContext,
    model: &str,
    target: Option<&TargetRelation>,
) -> Option<RelationInfo> {
    // The detail-map key is already alias-resolved (context build), so the
    // key doubles as the relation identifier.
    if !ctx.model_detail_map.contains_key(model) {
        return None;
    }
    let database = target.and_then(|t| t.database.clone());
    let schema = target.and_then(|t| t.schema.clone());
    Some(assemble(
        RelationKind::Model,
        model.to_string(),
        None,
        database,
        schema,
        model.to_string(),
    ))
}

fn source_relation(
    ctx: &DbtContext,
    source_name: &str,
    table_name: &str,
    target: Option<&TargetRelation>,
) -> Option<RelationInfo> {
    let source = ctx.source_detail_map.get(source_name)?;
    let table = source.tables.get(table_name)?;
    let database = source
        .database
        .clone()
        .or_else(|| target.and_then(|t| t.database.clone()));
    // dbt default: an undeclared source schema is the source's name
    let schema = source
        .schema
        .clone()
        .or_else(|| Some(source_name.to_string()));
    let identifier = table
        .identifier
        .clone()
        .unwrap_or_else(|| table_name.to_string());
    Some(assemble(
        RelationKind::Source,
        table_name.to_string(),
        Some(source_name.to_string()),
        database,
        schema,
        identifier,
    ))
}

fn assemble(
    kind: RelationKind,
    name: String,
    source: Option<String>,
    database: Option<String>,
    schema: Option<String>,
    identifier: String,
) -> RelationInfo {
    let relation = [
        database.as_deref(),
        schema.as_deref(),
        Some(identifier.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(".");
    RelationInfo {
        kind,
        name,
        source,
        database,
        schema,
        identifier,
        relation,
    }
}

//! The column name resolver: what a model or a source table has, read off
//! the project's published schema snapshot (`features::plan::schemas`),
//! which the background job built from every model's SQL through its
//! upstream models (`select *` and CTEs included), with the catalog and
//! the properties YAML filling in what the SQL cannot say.
//!
//! The snapshot gives the names, their order and their types (proven from
//! the SQL, else the catalog's, else the YAML's); the YAML gives each
//! column its description, and the spelling of its type where nothing
//! better is known — a declared type the SQL contradicts is the mismatch
//! diagnostics' business, not what the completion says. Without a snapshot (before the first job, or a
//! relation it does not know) the caller falls back to what it can read
//! itself.

use cogitator_dialects::Dialect;
use cogitator_ir::schema::ModelSchema;
use cogitator_ir::SqlType;
use cogitator_schemas::ColumnEntry;

use super::super::super::column_info::ColumnOf;
use super::columns::{Column, Lookup};
use crate::features::column_types::describe;

/// The model `name`'s columns as the schema job resolved them; `None`
/// when the snapshot does not know the model or knows none of its columns.
pub(super) fn model(lookup: &Lookup<'_>, name: &str) -> Option<Vec<Column>> {
    let schema = lookup.schemas?.model(name)?;
    let yaml = lookup
        .ctx
        .model_detail_map
        .get(name)
        .map_or(&[][..], |m| m.columns.as_slice());
    resolved(schema, yaml, name, &lookup.ctx.dialect)
        .map(|columns| of(columns, ColumnOf::Model(name.to_string())))
}

/// The columns, each saying what it is a column of (`column_info`).
fn of(columns: Vec<Column>, relation: ColumnOf) -> Vec<Column> {
    columns
        .into_iter()
        .map(|c| Column {
            of: Some(relation.clone()),
            ..c
        })
        .collect()
}

/// The source table's columns as the schema job resolved them.
pub(super) fn source(lookup: &Lookup<'_>, source: &str, table: &str) -> Option<Vec<Column>> {
    let schema = lookup.schemas?.source(source, table)?;
    let yaml = lookup
        .ctx
        .source_detail_map
        .get(source)
        .and_then(|s| s.tables.get(table))
        .map_or(&[][..], |t| t.columns.as_slice());
    resolved(
        schema,
        yaml,
        &format!("{source}.{table}"),
        &lookup.ctx.dialect,
    )
    .map(|columns| {
        of(
            columns,
            ColumnOf::Source {
                source: source.to_string(),
                table: table.to_string(),
            },
        )
    })
}

/// A schema's columns in its order, each with its YAML description and the
/// type the job resolved, else the one the YAML declares.
pub(super) fn resolved(
    schema: &ModelSchema,
    yaml: &[ColumnEntry],
    relation: &str,
    dialect: &Dialect,
) -> Option<Vec<Column>> {
    if schema.columns.is_empty() {
        return None;
    }
    let columns = schema
        .columns
        .iter()
        .map(|col| {
            let entry = yaml.iter().find(|e| e.name.eq_ignore_ascii_case(&col.name));
            Column {
                name: col.name.to_string(),
                relation: relation.to_string(),
                data_type: type_name(&col.ty.ty, dialect)
                    .or_else(|| entry.and_then(|e| e.data_type.clone())),
                description: entry.map(|e| e.description.clone()).unwrap_or_default(),
                of: None,
            }
        })
        .collect();
    Some(columns)
}

/// A short name for a proven type: the dialect's spelling when it is
/// exact, else its family (`integer`, `timestamp`); nothing when unknown.
pub(super) fn type_name(ty: &SqlType, dialect: &Dialect) -> Option<String> {
    if matches!(ty, SqlType::Unknown | SqlType::Null) {
        return None;
    }
    let text = describe(ty, dialect);
    let short = text.split(" (").next().unwrap_or(&text).trim();
    (!short.is_empty()).then(|| short.to_string())
}

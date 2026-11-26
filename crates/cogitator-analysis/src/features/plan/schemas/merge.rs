//! Assembles a `ModelSchema` from what is known: SQL inference, catalog
//! types and YAML. Column lookups are by name through hash maps, which are
//! never iterated, so column order always comes from the leading source.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use cogitator_dialects::Dialect;
use cogitator_ir::schema::{Completeness, ModelSchema, ProjectSchemas, SchemaCol};
use cogitator_ir::{ColType, Nullability, SqlType};
use cogitator_schemas::ColumnEntry;
use cogitator_sql_front::parse_sql_type;

use super::infer::Inferred;
use crate::artifacts::catalog::{CatalogColumn, CatalogTable};
use crate::features::column_types::Relation;

pub(super) struct BuiltSchema {
    pub(super) schema: ModelSchema,
    pub(super) capped: bool,
}

/// What the model's own entry adds to its inferred columns.
pub(super) struct ModelFacts<'a> {
    pub(super) yaml: &'a [ColumnEntry],
    /// `contract: {enforced: true}`: a YAML `data_type` is then proven.
    pub(super) contract: bool,
    /// Column sets the entry or the manifest declares unique (model-level
    /// constraints, combination tests, an incremental `unique_key`),
    /// lower-cased.
    pub(super) keys: Vec<Vec<String>>,
    /// The schemas built so far: every upstream model and source.
    pub(super) upstream: &'a ProjectSchemas,
}

/// SQL inference when there is one; otherwise whatever YAML and the
/// catalog list, which is never a complete list.
pub(super) fn model_schema(
    inferred: Option<&Inferred>,
    facts: &ModelFacts<'_>,
    catalog: Option<&CatalogTable>,
    dialect: &Dialect,
    cap: usize,
) -> Option<BuiltSchema> {
    let yaml = facts.yaml;
    let catalog_cols = catalog.map_or(&[][..], |c| c.columns.as_slice());
    let Some(inferred) = inferred else {
        let mut built = listed(yaml, catalog_cols, dialect, cap, Completeness::Open)?;
        mark_keys(&mut built, &facts.keys, facts.contract);
        return Some(built);
    };
    let by_catalog = catalog_index(catalog_cols);
    let by_yaml = yaml_index(yaml);
    let mut columns: Vec<SchemaCol> = Vec::with_capacity(inferred.columns.len());
    for col in &inferred.columns {
        let key = col.name.to_lowercase();
        let mut ty = col.ty.clone(); // boundary: the schema owns its types
        let mut proven =
            ty.ty != SqlType::Unknown && feeds_proven(inferred, &col.name, facts.upstream);
        if ty.ty == SqlType::Unknown {
            if let Some(found) = by_catalog.get(&key) {
                ty.ty = parse_sql_type(&found.data_type, dialect);
                proven = ty.ty != SqlType::Unknown;
            }
        }
        let entry = by_yaml.get(&key).copied();
        if ty.ty == SqlType::Unknown {
            if let Some(raw) = entry.and_then(|e| e.data_type.as_deref()) {
                ty.ty = parse_sql_type(raw, dialect);
                proven = facts.contract && ty.ty != SqlType::Unknown;
            }
        }
        let mut column = with_tests(col.name.clone(), ty, entry);
        column.proven_type = proven;
        columns.push(column);
    }
    let completeness = if inferred.open {
        append_listed(&mut columns, yaml, catalog_cols, dialect);
        Completeness::Open
    } else {
        Completeness::Closed
    };
    let mut built = finish(columns, completeness, cap);
    let mut keys = facts.keys.clone();
    if !inferred.template_unique_key.is_empty() {
        keys.push(inferred.template_unique_key.clone());
    }
    mark_keys(&mut built, &keys, false);
    Some(built)
}

/// Marks single-column keys' columns unique and records composite keys
/// in `unique_sets` (and, under an enforced contract, makes the
/// YAML-typed columns proven), refreshing the fingerprint.
fn mark_keys(built: &mut BuiltSchema, keys: &[Vec<String>], contract: bool) {
    let mut columns: Vec<SchemaCol> = built.schema.columns.to_vec();
    let mut sets: Vec<Vec<Arc<str>>> = Vec::new();
    for key in keys {
        let mut set: Vec<Arc<str>> = key
            .iter()
            .map(|k| Arc::from(k.to_ascii_lowercase()))
            .collect();
        set.sort();
        set.dedup();
        if set.len() > 1 && !sets.contains(&set) {
            sets.push(set);
        }
    }
    sets.sort();
    for column in &mut columns {
        if keys
            .iter()
            .any(|k| k.len() == 1 && k[0].eq_ignore_ascii_case(&column.name))
        {
            column.unique = true;
        }
        if contract && column.ty.ty != SqlType::Unknown {
            column.proven_type = true;
        }
    }
    let mut hasher = DefaultHasher::new();
    columns.hash(&mut hasher);
    built.schema.completeness.hash(&mut hasher);
    if !sets.is_empty() {
        sets.hash(&mut hasher);
    }
    built.schema.fingerprint = hasher.finish();
    built.schema.columns = Arc::from(columns);
    built.schema.unique_sets = sets;
}

/// True when every upstream column the output column's type rests on is
/// itself proven in the schemas built so far (casts and literals rest on
/// nothing and are proven by themselves).
fn feeds_proven(inferred: &Inferred, name: &str, upstream: &ProjectSchemas) -> bool {
    let Some(raw) = inferred
        .typed
        .iter()
        .find(|c| c.name.eq_ignore_ascii_case(name))
    else {
        return false;
    };
    raw.feeds.iter().all(|feed| {
        let schema = match &feed.relation {
            Relation::Model { name } => upstream.model(name),
            Relation::Source { source, table } => upstream.source(source, table),
            Relation::This | Relation::Other => None,
        };
        schema.is_some_and(|s| {
            s.columns
                .iter()
                .any(|c| c.proven_type && c.name.eq_ignore_ascii_case(&feed.column))
        })
    })
}

/// Catalog columns are the whole list; YAML adds tests and type gaps but
/// cannot close a source on its own.
pub(super) fn source_schema(
    yaml: &[ColumnEntry],
    catalog: Option<&CatalogTable>,
    dialect: &Dialect,
    cap: usize,
) -> Option<BuiltSchema> {
    let catalog_cols = catalog.map_or(&[][..], |c| c.columns.as_slice());
    let completeness = if catalog_cols.is_empty() {
        Completeness::Open
    } else {
        Completeness::Closed
    };
    listed(yaml, catalog_cols, dialect, cap, completeness)
}

/// Catalog columns first, in warehouse order, then YAML-only columns.
fn listed(
    yaml: &[ColumnEntry],
    catalog: &[CatalogColumn],
    dialect: &Dialect,
    cap: usize,
    completeness: Completeness,
) -> Option<BuiltSchema> {
    if yaml.is_empty() && catalog.is_empty() {
        return None;
    }
    let by_yaml = yaml_index(yaml);
    let mut columns: Vec<SchemaCol> = catalog
        .iter()
        .map(|c| {
            let entry = by_yaml.get(&c.name.to_lowercase()).copied();
            let mut ty = parse_sql_type(&c.data_type, dialect);
            let proven = ty != SqlType::Unknown;
            if ty == SqlType::Unknown {
                if let Some(raw) = entry.and_then(|e| e.data_type.as_deref()) {
                    ty = parse_sql_type(raw, dialect);
                }
            }
            let mut column = with_tests(
                Arc::from(c.name.as_str()),
                ColType::new(ty, Nullability::Unknown),
                entry,
            );
            column.proven_type = proven;
            column
        })
        .collect();
    if completeness == Completeness::Open {
        append_listed(&mut columns, yaml, &[], dialect);
    }
    Some(finish(columns, completeness, cap))
}

/// Appends listed columns not already present, for an `Open` schema: extra
/// names are evidence of columns, and an open list claims no completeness.
fn append_listed(
    columns: &mut Vec<SchemaCol>,
    yaml: &[ColumnEntry],
    catalog: &[CatalogColumn],
    dialect: &Dialect,
) {
    let mut seen: std::collections::HashSet<String> =
        columns.iter().map(|c| c.name.to_lowercase()).collect();
    for column in catalog {
        if seen.insert(column.name.to_lowercase()) {
            columns.push(SchemaCol {
                name: Arc::from(column.name.as_str()),
                ty: ColType::new(
                    parse_sql_type(&column.data_type, dialect),
                    Nullability::Unknown,
                ),
                unique: false,
                proven_type: true,
            });
        }
    }
    for entry in yaml {
        if seen.insert(entry.name.to_lowercase()) {
            let ty = entry
                .data_type
                .as_deref()
                .map_or(SqlType::Unknown, |raw| parse_sql_type(raw, dialect));
            columns.push(with_tests(
                Arc::from(entry.name.as_str()),
                ColType::new(ty, Nullability::Unknown),
                Some(entry),
            ));
        }
    }
}

/// `unique` marks the column (a strict test or a `unique` /
/// `primary_key` constraint); `not_null` makes it `NotNull`.
pub(in crate::features::plan) fn with_tests(
    name: Arc<str>,
    mut ty: ColType,
    entry: Option<&ColumnEntry>,
) -> SchemaCol {
    let unique = entry.is_some_and(|e| e.proven_by("unique"));
    if entry.is_some_and(|e| e.proven_by("not_null")) {
        ty.null = Nullability::NotNull;
    }
    SchemaCol {
        name,
        ty,
        unique,
        proven_type: false,
    }
}

fn catalog_index(columns: &[CatalogColumn]) -> HashMap<String, &CatalogColumn> {
    columns.iter().map(|c| (c.name.to_lowercase(), c)).collect()
}

fn yaml_index(yaml: &[ColumnEntry]) -> HashMap<String, &ColumnEntry> {
    yaml.iter().map(|e| (e.name.to_lowercase(), e)).collect()
}

fn finish(mut columns: Vec<SchemaCol>, completeness: Completeness, cap: usize) -> BuiltSchema {
    let capped = columns.len() > cap;
    if capped {
        columns.truncate(cap);
    }
    let completeness = if capped {
        Completeness::Open
    } else {
        completeness
    };
    let mut hasher = DefaultHasher::new();
    columns.hash(&mut hasher);
    completeness.hash(&mut hasher);
    BuiltSchema {
        schema: ModelSchema {
            columns: Arc::from(columns),
            completeness,
            fingerprint: hasher.finish(),
            unique_sets: Vec::new(),
        },
        capped,
    }
}

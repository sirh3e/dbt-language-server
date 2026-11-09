//! `SchemaEnv` from direct upstream columns. The project-wide topological
//! job is Wave 4, so every schema filled here stays `Open`: a YAML or
//! catalog listing is evidence of these columns, not proof of no others.

use std::collections::BTreeSet;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use cogitator_ir::hir::{FromKind, Hir, RelTarget};
use cogitator_ir::schema::{Completeness, ModelSchema, ProjectSchemas, SchemaCol};
use cogitator_ir::{ColType, Nullability, SqlType};
use cogitator_schemas::ColumnEntry;
use cogitator_sql_front::{parse_sql_type, SchemaEnv};

use crate::artifacts::catalog::CatalogTable;

use super::PlanInput;

/// Named relations the query reads directly.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Upstreams {
    models: BTreeSet<String>,
    sources: BTreeSet<(String, String)>,
    this: bool,
    /// A hard-coded table or dynamic Jinja relation: no facts can exist.
    unnamed: bool,
}

impl Upstreams {
    /// Walks every FROM item; a parse hole contributes nothing.
    pub(super) fn of(hir: &Hir) -> Self {
        let mut out = Upstreams::default();
        for (_, from) in hir.from.iter() {
            let FromKind::Base { target, .. } = &from.kind else {
                continue;
            };
            match target {
                RelTarget::Model { name, .. } => {
                    out.models.insert(hir.syms.text(*name).to_string());
                }
                RelTarget::Source { source, table } => {
                    out.sources.insert((
                        hir.syms.text(*source).to_string(),
                        hir.syms.text(*table).to_string(),
                    ));
                }
                RelTarget::This => out.this = true,
                RelTarget::Physical(_) | RelTarget::Jinja(_) => out.unnamed = true,
                RelTarget::Cte(_) => {}
            }
        }
        out
    }

    fn count(&self) -> usize {
        self.models.len() + self.sources.len() + usize::from(self.this)
    }
}

/// The environment built, and what it can honestly claim.
#[derive(Debug, Default)]
pub(super) struct BuiltEnv {
    pub(super) env: SchemaEnv,
    /// Every direct upstream has a schema, so `Need::Schema` holds.
    pub(super) schema_known: bool,
    /// At least one column has a stated type, so `Need::Types` holds.
    pub(super) types_known: bool,
}

impl BuiltEnv {
    pub(super) fn is_empty(&self) -> bool {
        self.env.schemas.models.is_empty() && self.env.schemas.sources.is_empty()
    }
}

/// An environment naming only the model: used for the first HIR pass.
pub(super) fn bare_env(input: &PlanInput<'_>) -> SchemaEnv {
    SchemaEnv {
        schemas: Arc::new(ProjectSchemas {
            revision: input.schema_revision,
            ..ProjectSchemas::default()
        }),
        model: input.model.map(Arc::from),
    }
}

/// Fills schemas for `upstreams` from YAML and the catalog. A relation
/// with neither is left out, so the binder keeps it open.
pub(super) fn fill(upstreams: &Upstreams, input: &PlanInput<'_>) -> BuiltEnv {
    let mut schemas = ProjectSchemas {
        revision: input.schema_revision,
        ..ProjectSchemas::default()
    };
    let mut found = 0;
    let mut types_known = false;

    let self_name = input.model.filter(|_| upstreams.this);
    for name in upstreams.models.iter().map(String::as_str).chain(self_name) {
        let yaml = input
            .models
            .get(name)
            .map_or(&[][..], |d| d.columns.as_slice());
        let catalog = input.catalog.and_then(|c| c.models.get(name));
        if let Some(schema) = schema_of(yaml, catalog, input) {
            found += 1;
            types_known |= has_types(&schema);
            schemas.models.insert(Arc::from(name), Arc::new(schema));
        }
    }
    for (source, table) in &upstreams.sources {
        let yaml = input
            .sources
            .get(source)
            .and_then(|s| s.tables.get(table))
            .map_or(&[][..], |t| t.columns.as_slice());
        let catalog = input
            .catalog
            // boundary: the catalog's key is an owned tuple
            .and_then(|c| c.sources.get(&(source.clone(), table.clone())));
        if let Some(schema) = schema_of(yaml, catalog, input) {
            found += 1;
            types_known |= has_types(&schema);
            schemas.sources.insert(
                (Arc::from(source.as_str()), Arc::from(table.as_str())),
                Arc::new(schema),
            );
        }
    }
    BuiltEnv {
        schema_known: upstreams.count() > 0 && !upstreams.unnamed && found == upstreams.count(),
        types_known,
        env: SchemaEnv {
            schemas: Arc::new(schemas),
            model: input.model.map(Arc::from),
        },
    }
}

fn has_types(schema: &ModelSchema) -> bool {
    schema.columns.iter().any(|c| c.ty.ty != SqlType::Unknown)
}

/// Catalog columns first, in warehouse order, with their warehouse type;
/// YAML-only columns after, typed from `data_type` when stated.
fn schema_of(
    yaml: &[ColumnEntry],
    catalog: Option<&CatalogTable>,
    input: &PlanInput<'_>,
) -> Option<ModelSchema> {
    if yaml.is_empty() && catalog.is_none_or(|c| c.columns.is_empty()) {
        return None;
    }
    let mut columns: Vec<SchemaCol> = Vec::new();
    for column in catalog.map_or(&[][..], |c| c.columns.as_slice()) {
        columns.push(col(&column.name, Some(&column.data_type), input));
    }
    let typed_in_catalog = columns.len();
    for entry in yaml {
        let known = columns
            .iter()
            .take(typed_in_catalog)
            .position(|c| c.name.eq_ignore_ascii_case(&entry.name));
        match known {
            Some(i) if columns[i].ty.ty == SqlType::Unknown => {
                columns[i] = col(&entry.name, entry.data_type.as_deref(), input);
            }
            Some(_) => {}
            None => columns.push(col(&entry.name, entry.data_type.as_deref(), input)),
        }
    }
    let mut hasher = DefaultHasher::new();
    columns.hash(&mut hasher);
    Some(ModelSchema {
        columns: Arc::from(columns),
        completeness: Completeness::Open,
        fingerprint: hasher.finish(),
    })
}

fn col(name: &str, raw_type: Option<&str>, input: &PlanInput<'_>) -> SchemaCol {
    let ty = raw_type.map_or(SqlType::Unknown, |raw| parse_sql_type(raw, input.dialect));
    SchemaCol {
        name: Arc::from(name),
        ty: ColType::new(ty, Nullability::Unknown),
        unique: false,
    }
}

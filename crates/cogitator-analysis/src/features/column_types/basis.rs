//! How much an inferred type is worth: the upstream columns it rests on
//! are traced to their own evidence. A type is `Proven` when every column
//! it was derived from is typed by SQL inference, the catalog, or an
//! enforced contract; one that rests on a YAML `data_type` nobody enforces
//! is only `Declared`, and is shown but never diagnosed.

use cogitator_dialects::Dialect;
use cogitator_ir::schema::ModelSchema;
use cogitator_ir::{Nullability, SqlType};
use cogitator_schemas::ColumnEntry;
use cogitator_sql_front::parse_sql_type;
use serde::Serialize;

use super::compare::{describe, render};
use super::provenance::{is_stated, Provenance, RawColumn, Relation, UpstreamColumn};
use crate::features::plan::schemas::SchemaSnapshot;
use crate::DbtContext;

/// Bounds the walk up the model graph.
const MAX_HOPS: usize = 16;

/// How far a column's type can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Basis {
    /// No type is claimed.
    Unknown,
    /// Rests on a YAML `data_type` nothing enforces.
    Declared,
    /// SQL inference over facts that hold.
    Proven,
}

/// Where an upstream column's type was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// The upstream model's own SQL, itself proven.
    Inferred,
    Catalog,
    /// A YAML `data_type` under `contract: {enforced: true}`.
    Contract,
    /// A YAML `data_type` without an enforced contract.
    Yaml,
    Unknown,
}

impl Origin {
    fn basis(self) -> Basis {
        match self {
            Origin::Inferred | Origin::Catalog | Origin::Contract => Basis::Proven,
            Origin::Yaml => Basis::Declared,
            Origin::Unknown => Basis::Unknown,
        }
    }

    fn words(self) -> &'static str {
        match self {
            Origin::Inferred => "inferred from its SQL",
            Origin::Catalog => "from the catalog",
            Origin::Contract => "its enforced contract",
            Origin::Yaml => "its YAML data_type, not enforced",
            Origin::Unknown => "untyped",
        }
    }
}

/// An upstream column with the evidence behind its type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Feed {
    #[serde(flatten)]
    pub column: UpstreamColumn,
    pub origin: Origin,
}

/// One output column of a model, typed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnType {
    pub name: String,
    /// The lattice type; `Unknown` when nothing is claimed.
    #[serde(skip)]
    pub sql_type: SqlType,
    /// The dialect's spelling, set only when the type is pinned exactly.
    pub data_type: Option<String>,
    /// For people: the spelling, or the family and what is unproven.
    pub description: String,
    /// `Some(false)` when proven not null, `Some(true)` when nullable.
    pub nullable: Option<bool>,
    pub basis: Basis,
    pub provenance: Provenance,
    pub feeds: Vec<Feed>,
    /// One line on why the type is what it is.
    pub why: String,
}

impl ColumnType {
    /// True when diagnostics may rely on this type.
    pub fn is_proven(&self) -> bool {
        self.basis == Basis::Proven && is_stated(&self.sql_type)
    }
}

/// A model's output columns, from the project schema job's cached result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelColumnTypes {
    pub model: String,
    /// The schema revision the types were read at.
    pub revision: u64,
    pub columns: Vec<ColumnType>,
}

impl ModelColumnTypes {
    /// The column named `name`, case-insensitively.
    pub fn column(&self, name: &str) -> Option<&ColumnType> {
        self.columns
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(name))
    }
}

/// The output-column types of `model`, read off the snapshot (never
/// planned again). `None` when the job has no SQL inference for it.
pub fn column_types(
    ctx: &DbtContext,
    snapshot: &SchemaSnapshot,
    model: &str,
) -> Option<ModelColumnTypes> {
    let output = snapshot.outputs.get(model)?;
    let tracer = Tracer { ctx, snapshot };
    let columns = output
        .iter()
        .map(|raw| tracer.typed(raw, &ctx.dialect, 0))
        .collect();
    Some(ModelColumnTypes {
        model: model.to_string(),
        revision: snapshot.revision,
        columns,
    })
}

struct Tracer<'a> {
    ctx: &'a DbtContext,
    snapshot: &'a SchemaSnapshot,
}

impl Tracer<'_> {
    fn typed(&self, raw: &RawColumn, dialect: &Dialect, hops: usize) -> ColumnType {
        let feeds: Vec<Feed> = raw
            .feeds
            .iter()
            .map(|f| Feed {
                column: f.clone(),
                origin: self.origin(f, hops + 1),
            })
            .collect();
        let basis = if is_stated(&raw.ty.ty) {
            feeds
                .iter()
                .map(|f| f.origin.basis())
                .min()
                .unwrap_or(Basis::Proven)
        } else {
            Basis::Unknown
        };
        ColumnType {
            name: raw.name.clone(),
            sql_type: raw.ty.ty.clone(),
            data_type: render(&raw.ty.ty, dialect),
            description: describe(&raw.ty.ty, dialect),
            nullable: match raw.ty.null {
                Nullability::NotNull => Some(false),
                Nullability::Nullable => Some(true),
                Nullability::Unknown => None,
            },
            basis,
            why: why(&raw.provenance, &feeds),
            provenance: raw.provenance.clone(),
            feeds,
        }
    }

    fn origin(&self, up: &UpstreamColumn, hops: usize) -> Origin {
        if hops > MAX_HOPS {
            return Origin::Unknown;
        }
        match &up.relation {
            Relation::Model { name } => self.model_origin(name, &up.column, hops),
            Relation::Source { source, table } => {
                let schema = self.snapshot.schemas.source(source, table);
                let yaml = self
                    .ctx
                    .source_detail_map
                    .get(source)
                    .and_then(|s| s.tables.get(table))
                    .map_or(&[][..], |t| t.columns.as_slice());
                listed_origin(
                    schema.map(|s| &**s),
                    yaml,
                    &up.column,
                    false,
                    &self.ctx.dialect,
                )
            }
            Relation::This | Relation::Other => Origin::Unknown,
        }
    }

    fn model_origin(&self, model: &str, column: &str, hops: usize) -> Origin {
        let inferred = self
            .snapshot
            .outputs
            .get(model)
            .and_then(|cols| cols.iter().find(|c| c.name.eq_ignore_ascii_case(column)))
            .filter(|c| is_stated(&c.ty.ty));
        if let Some(raw) = inferred {
            let proven = raw
                .feeds
                .iter()
                .all(|f| self.origin(f, hops + 1).basis() == Basis::Proven);
            return if proven {
                Origin::Inferred
            } else {
                Origin::Yaml
            };
        }
        let details = self.ctx.model_detail_map.get(model);
        let yaml = details.map_or(&[][..], |d| d.columns.as_slice());
        let contract = details.is_some_and(|d| d.contract_enforced);
        let schema = self.snapshot.schemas.model(model).map(|s| &**s);
        listed_origin(schema, yaml, column, contract, &self.ctx.dialect)
    }
}

/// A type the merged schema holds that no SQL inferred: from YAML when the
/// YAML spells that very type, from the catalog otherwise.
fn listed_origin(
    schema: Option<&ModelSchema>,
    yaml: &[ColumnEntry],
    column: &str,
    contract: bool,
    dialect: &Dialect,
) -> Origin {
    let Some(col) = schema.and_then(|s| {
        s.columns
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(column))
    }) else {
        return Origin::Unknown;
    };
    if !is_stated(&col.ty.ty) {
        return Origin::Unknown;
    }
    let declared = yaml
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(column))
        .and_then(|e| e.data_type.as_deref())
        .map(|raw| parse_sql_type(raw, dialect));
    match declared {
        Some(ty) if ty == col.ty.ty && contract => Origin::Contract,
        Some(ty) if ty == col.ty.ty => Origin::Yaml,
        _ => Origin::Catalog,
    }
}

fn why(provenance: &Provenance, feeds: &[Feed]) -> String {
    let from = || {
        feeds
            .iter()
            .map(|f| format!("{} ({})", feed_name(&f.column), f.origin.words()))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let over = |what: String| {
        if feeds.is_empty() {
            what
        } else {
            format!("{what} over {}", from())
        }
    };
    match provenance {
        Provenance::Cast => "an explicit cast".into(),
        Provenance::Literal => "a literal".into(),
        Provenance::Function { name } => over(format!("returned by {name}()")),
        Provenance::Aggregate { name } => over(format!("aggregate {name}()")),
        Provenance::Window { name } => over(format!("window function {name}()")),
        Provenance::Upstream { .. } => format!("from {}", from()),
        Provenance::SetOp => over("set operation arms widened".into()),
        Provenance::Expression => over("computed by an expression".into()),
        Provenance::Unknown => "not inferable".into(),
    }
}

fn feed_name(column: &UpstreamColumn) -> String {
    match &column.relation {
        Relation::Model { name } => format!("{name}.{}", column.column),
        Relation::Source { source, table } => format!("{source}.{table}.{}", column.column),
        Relation::This => format!("this.{}", column.column),
        Relation::Other => column.column.clone(),
    }
}

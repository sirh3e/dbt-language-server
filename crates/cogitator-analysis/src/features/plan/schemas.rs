//! The project-wide schema job: walks the model graph upstream-first,
//! infers each model's output columns through HIR and MIR from its saved
//! file, and overlays catalog and YAML facts.
//!
//! Pure over [`JobInput`]: the only disk access is through [`SavedFiles`],
//! which the server shell implements. Unit of invalidation is the model:
//! a result is reused while the file stamp and the fingerprints of its
//! upstream schemas are unchanged, so a rebuild that leaves a model's
//! schema the same stops there (early cut-off).
//!
//! ## When a schema is `Closed`
//!
//! `Completeness::Closed` licenses "column does not exist". It requires
//! positive knowledge of the whole column list:
//!
//! - A **model** is `Closed` when its SQL lowered and [`output_open`] is
//!   false: no star over an open input, no Jinja select item expanding to
//!   unknown columns, no unparsed select item. Open *inputs* do not matter
//!   when the model names its columns. It is `Open` when an output has no
//!   name or a name a dialect fold would alter (a quoted identifier the
//!   binder would re-fold), when the column cap truncated it, or when it
//!   has no SQL inference at all (declined, cycle, unreadable, seed).
//! - A **source** is `Closed` only from the catalog, with the catalog's
//!   age carried through `Evidence::Catalog`. A YAML-only source stays
//!   `Open`: YAML documents the columns someone chose to list.
//!
//! [`output_open`]: cogitator_rules::rules::cross::output::output_open

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::Arc;

use cogitator_dialects::Dialect;
use cogitator_ir::schema::ProjectSchemas;

use crate::artifacts::catalog::CatalogIndex;
use crate::artifacts::manifest::ManifestIndex;
use crate::context::types::{ModelDetails, SourceDetails};
use crate::graph::{NodeKind, ProjectGraph};

mod dag;
mod infer;
pub(super) mod merge;
mod reads;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_demand;
mod upstream;

pub use dag::DagIndex;
use infer::Inferred;
use upstream::Upstream;

/// Saved file access, injected so the core stays pure.
pub trait SavedFiles {
    /// A value that changes when the file does; `None` if unreadable.
    fn stamp(&self, path: &Path) -> Option<u64>;
    /// The saved text, bounded by the implementation.
    fn read(&self, path: &Path) -> Option<String>;
}

/// Counts that bound the job; passing one degrades explicitly.
#[derive(Debug, Clone, Copy)]
pub struct JobLimits {
    pub max_models: usize,
    pub max_columns: usize,
    pub max_read_bytes: u64,
}

impl Default for JobLimits {
    fn default() -> Self {
        JobLimits {
            max_models: 20_000,
            max_columns: 5_000,
            max_read_bytes: 1 << 30,
        }
    }
}

/// Why a model's schema rests on less than its SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobIssue {
    Unreadable,
    Declined,
    Cycle,
    MissingUpstream(String),
    ModelCap,
    ReadCap,
    ColumnCap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobDegradation {
    pub model: Arc<str>,
    pub issue: JobIssue,
}

impl JobDegradation {
    fn new(model: &Arc<str>, issue: JobIssue) -> Self {
        JobDegradation {
            model: model.clone(),
            issue,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JobReport {
    pub models: usize,
    pub rebuilt: usize,
    pub reused: usize,
    pub degraded: Vec<JobDegradation>,
}

/// The published result of one job.
#[derive(Debug)]
pub struct SchemaSnapshot {
    pub revision: u64,
    pub schemas: Arc<ProjectSchemas>,
    pub dag: DagIndex,
    pub report: JobReport,
    content: u64,
}

struct Cached {
    key: (Option<u64>, u64),
    inferred: Option<Arc<Inferred>>,
}

/// What survives between jobs.
#[derive(Default)]
pub struct JobState {
    entries: HashMap<Arc<str>, Cached>,
    previous: Option<Arc<SchemaSnapshot>>,
}

pub struct JobInput<'a> {
    pub dialect: &'a Dialect,
    pub models: &'a HashMap<String, ModelDetails>,
    pub sources: &'a HashMap<String, SourceDetails>,
    pub graph: &'a ProjectGraph,
    pub manifest: Option<&'a ManifestIndex>,
    pub catalog: Option<&'a CatalogIndex>,
    pub files: &'a dyn SavedFiles,
    pub limits: JobLimits,
    /// True once a newer job has been requested.
    pub stale: &'a dyn Fn() -> bool,
}

/// One run's accumulators.
struct Walk {
    schemas: ProjectSchemas,
    report: JobReport,
    read_bytes: u64,
}

/// Runs the job. `None` means a newer job superseded this one; the results
/// computed so far stay in `state` for it.
pub fn run_schema_job(input: &JobInput<'_>, state: &mut JobState) -> Option<Arc<SchemaSnapshot>> {
    let (nodes, _) = input.graph.dag();
    let models: Vec<(&str, usize)> = nodes
        .iter()
        .filter(|(n, _)| n.kind == NodeKind::Model)
        .map(|(n, d)| (n.name.as_str(), *d))
        .collect();
    let depths: HashMap<&str, usize> = models.iter().copied().collect();
    let mut walk = Walk {
        schemas: ProjectSchemas::default(),
        report: JobReport {
            models: models.len(),
            ..JobReport::default()
        },
        read_bytes: 0,
    };
    insert_sources(input, &mut walk.schemas);
    let mut inferred: HashMap<Arc<str>, Option<Arc<Inferred>>> = HashMap::new();
    let mut backed: HashSet<Arc<str>> = HashSet::new();

    for (index, &(name, _)) in models.iter().enumerate() {
        if (input.stale)() {
            return None;
        }
        let up = Upstream::of(input.graph, name, &depths);
        let model: Arc<str> = Arc::from(name);
        let result = model_result(input, state, &model, &up, index, &mut walk);
        add_schema(input, &model, result.as_deref(), &mut walk);
        if up.catalog_backed(&walk.schemas, &backed, input) {
            backed.insert(model.clone());
        }
        inferred.insert(model, result);
    }
    state
        .entries
        .retain(|name, _| inferred.contains_key(name.as_ref()));

    let order: Vec<&str> = models.iter().map(|(n, _)| *n).collect();
    let generated_at = input
        .catalog
        .and_then(|c| c.generated_at.as_deref())
        .unwrap_or("unknown");
    let index = dag::fold(&order, input, &inferred, backed, Arc::from(generated_at));
    Some(publish(state, walk.schemas, index, walk.report))
}

/// The cached or rebuilt inference for one model, with its degradations.
fn model_result(
    input: &JobInput<'_>,
    state: &mut JobState,
    model: &Arc<str>,
    up: &Upstream<'_>,
    index: usize,
    walk: &mut Walk,
) -> Option<Arc<Inferred>> {
    if up.cycle {
        walk.report
            .degraded
            .push(JobDegradation::new(model, JobIssue::Cycle));
        return None;
    }
    if let Some(missing) = up.missing(&walk.schemas, input.models) {
        walk.report.degraded.push(JobDegradation::new(
            model,
            JobIssue::MissingUpstream(missing),
        ));
    }
    let path = input
        .models
        .get(model.as_ref())
        .filter(|d| d.uri.extension().is_some_and(|e| e == "sql"))
        .map(|d| d.uri.as_path())?;
    if index >= input.limits.max_models {
        walk.report
            .degraded
            .push(JobDegradation::new(model, JobIssue::ModelCap));
        return None;
    }
    build_one(input, state, model, path, up, walk)
}

fn build_one(
    input: &JobInput<'_>,
    state: &mut JobState,
    model: &Arc<str>,
    path: &Path,
    up: &Upstream<'_>,
    walk: &mut Walk,
) -> Option<Arc<Inferred>> {
    let stamp = input.files.stamp(path);
    let (env, digest) = up.env_for(model, &walk.schemas, input);
    let key = (stamp, digest);
    if stamp.is_some() {
        if let Some(cached) = state.entries.get(model.as_ref()) {
            if cached.key == key {
                walk.report.reused += 1;
                return cached.inferred.clone();
            }
        }
    }
    if walk.read_bytes >= input.limits.max_read_bytes {
        walk.report
            .degraded
            .push(JobDegradation::new(model, JobIssue::ReadCap));
        return None;
    }
    let Some(text) = stamp.and_then(|_| input.files.read(path)) else {
        walk.report
            .degraded
            .push(JobDegradation::new(model, JobIssue::Unreadable));
        return None;
    };
    walk.read_bytes += text.len() as u64;
    walk.report.rebuilt += 1;
    let inferred = infer::infer(&text, input.dialect, &env).map(Arc::new);
    if inferred.is_none() {
        walk.report
            .degraded
            .push(JobDegradation::new(model, JobIssue::Declined));
    }
    state.entries.insert(
        model.clone(),
        Cached {
            key,
            inferred: inferred.clone(),
        },
    );
    inferred
}

fn add_schema(
    input: &JobInput<'_>,
    model: &Arc<str>,
    result: Option<&Inferred>,
    walk: &mut Walk,
) {
    let yaml = input
        .models
        .get(model.as_ref())
        .map_or(&[][..], |d| d.columns.as_slice());
    let catalog = input.catalog.and_then(|c| c.models.get(model.as_ref()));
    let built = merge::model_schema(result, yaml, catalog, input.dialect, input.limits.max_columns);
    let Some(built) = built else {
        return;
    };
    if built.capped {
        walk.report
            .degraded
            .push(JobDegradation::new(model, JobIssue::ColumnCap));
    }
    walk.schemas
        .models
        .insert(model.clone(), Arc::new(built.schema));
}

fn insert_sources(input: &JobInput<'_>, schemas: &mut ProjectSchemas) {
    for (source, details) in input.sources {
        for (table, entry) in &details.tables {
            let catalog = input
                .catalog
                // boundary: the catalog's key is an owned tuple
                .and_then(|c| c.sources.get(&(source.clone(), table.clone())));
            let built = merge::source_schema(
                &entry.columns,
                catalog,
                input.dialect,
                input.limits.max_columns,
            );
            if let Some(built) = built {
                schemas.sources.insert(
                    (Arc::from(source.as_str()), Arc::from(table.as_str())),
                    Arc::new(built.schema),
                );
            }
        }
    }
}

/// Assigns the revision: unchanged content keeps the previous one, so a
/// refresh that changed nothing does not invalidate cached plans.
fn publish(
    state: &mut JobState,
    mut schemas: ProjectSchemas,
    dag: DagIndex,
    report: JobReport,
) -> Arc<SchemaSnapshot> {
    let content = content_hash(&schemas, &dag);
    let previous = state.previous.as_ref();
    let unchanged = previous.is_some_and(|p| p.content == content);
    let revision = match previous {
        Some(p) if unchanged => p.revision,
        Some(p) => p.revision + 1,
        None => 1,
    };
    schemas.revision = revision;
    let snapshot = Arc::new(SchemaSnapshot {
        revision,
        schemas: Arc::new(schemas),
        dag,
        report,
        content,
    });
    state.previous = Some(snapshot.clone());
    snapshot
}

/// Order-independent: the maps are sorted before hashing.
fn content_hash(schemas: &ProjectSchemas, dag: &DagIndex) -> u64 {
    let mut models: Vec<_> = schemas.models.iter().collect();
    models.sort_unstable_by(|a, b| a.0.cmp(b.0));
    let mut sources: Vec<_> = schemas.sources.iter().collect();
    sources.sort_unstable_by(|a, b| a.0.cmp(b.0));
    let mut hasher = DefaultHasher::new();
    for (name, schema) in models {
        name.hash(&mut hasher);
        schema.fingerprint.hash(&mut hasher);
    }
    for (key, schema) in sources {
        key.hash(&mut hasher);
        schema.fingerprint.hash(&mut hasher);
    }
    dag.digest(&mut hasher);
    hasher.finish()
}

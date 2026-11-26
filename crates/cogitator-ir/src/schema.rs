//! Project-level column knowledge the binder and rules read. The maps
//! are lookup-only: any output built from them must sort first.

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;

use crate::types::ColType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Completeness {
    /// Every column is known; "column does not exist" may be reported.
    Closed,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SchemaCol {
    pub name: Arc<str>,
    pub ty: ColType,
    /// Backed by a `unique` test or constraint.
    pub unique: bool,
    /// The type rests on evidence that holds: SQL inference over proven
    /// inputs, the catalog, or an enforced contract — not a YAML
    /// `data_type` nothing enforces (`type.*` rules warn only on these).
    pub proven_type: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelSchema {
    pub columns: Arc<[SchemaCol]>,
    pub completeness: Completeness,
    pub fingerprint: u64,
    /// Column sets of two or more columns that are unique together
    /// (a composite primary key, a `unique_combination_of_columns`
    /// test, an incremental model's composite `unique_key`). Lower-cased;
    /// single-column keys are `SchemaCol::unique` instead.
    pub unique_sets: Vec<Vec<Arc<str>>>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectSchemas {
    pub revision: u64,
    pub models: HashMap<Arc<str>, Arc<ModelSchema>>,
    pub sources: HashMap<(Arc<str>, Arc<str>), Arc<ModelSchema>>,
}

impl ProjectSchemas {
    pub fn model(&self, name: &str) -> Option<&Arc<ModelSchema>> {
        self.models.get(name)
    }

    pub fn source(&self, source: &str, table: &str) -> Option<&Arc<ModelSchema>> {
        // boundary: the tuple key owns its parts, so a lookup builds one
        let key: (Arc<str>, Arc<str>) = (Arc::from(source), Arc::from(table));
        self.sources.get(&key)
    }
}

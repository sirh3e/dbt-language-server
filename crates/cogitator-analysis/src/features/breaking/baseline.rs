//! The last BUILT schema of a model: the warehouse's columns from
//! `target/catalog.json` (`dbt docs generate`) when it has the model,
//! else the documented columns `target/manifest.json` recorded.

use serde::Serialize;

use super::BaselineInfo;
use crate::artifacts::ArtifactSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BaselineSource {
    /// `target/catalog.json`: real warehouse columns and types.
    Catalog,
    /// `target/manifest.json`: the columns as documented at the last parse.
    Manifest,
    /// The model's file as last saved: what the open buffer's edit changes.
    Saved,
}

impl BaselineSource {
    pub fn file(self) -> &'static str {
        match self {
            BaselineSource::Catalog => "target/catalog.json",
            BaselineSource::Manifest => "target/manifest.json",
            BaselineSource::Saved => "the saved file",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BaselineColumn {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Baseline {
    pub source: BaselineSource,
    pub generated_at: Option<String>,
    pub columns: Vec<BaselineColumn>,
}

impl Baseline {
    pub(super) fn info(&self) -> BaselineInfo {
        BaselineInfo {
            source: self.source,
            generated_at: self.generated_at.clone(),
        }
    }
}

/// The model's baseline, or `None` when no artifact has columns for it.
pub fn baseline(model: &str, artifacts: &ArtifactSet) -> Option<Baseline> {
    if let Some(catalog) = artifacts.catalog.as_deref() {
        if let Some(table) = catalog.models.get(model).filter(|t| !t.columns.is_empty()) {
            return Some(Baseline {
                source: BaselineSource::Catalog,
                generated_at: catalog.generated_at.clone(),
                columns: table
                    .columns
                    .iter()
                    .map(|c| BaselineColumn {
                        name: c.name.clone(),
                        data_type: (!c.data_type.trim().is_empty()).then(|| c.data_type.clone()),
                    })
                    .collect(),
            });
        }
    }
    let node = artifacts.manifest.as_deref()?.models.get(model)?;
    if node.columns.is_empty() {
        return None;
    }
    Some(Baseline {
        source: BaselineSource::Manifest,
        generated_at: None,
        columns: node
            .columns
            .iter()
            .map(|c| BaselineColumn {
                name: c.name.clone(),
                data_type: c.data_type.clone(),
            })
            .collect(),
    })
}

/// The saved file's columns as a baseline (the edit-time comparison).
pub fn saved_baseline(columns: &[super::CurrentColumn]) -> Baseline {
    Baseline {
        source: BaselineSource::Saved,
        generated_at: None,
        columns: columns
            .iter()
            .map(|c| BaselineColumn {
                name: c.name.clone(),
                data_type: c.data_type.clone(),
            })
            .collect(),
    }
}

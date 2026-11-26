//! The lineage graph cache on disk, beside the scan cache:
//! `<root>/target/.cogitator-lineage-cache`, versioned JSON. It is an
//! optimization only: any problem loading it (missing, corrupt, another
//! format or server build) is a trace and a cold start, and a failed save
//! is a trace too.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cogitator_analysis::lineage::{ColumnLineage, LineageOrigin, ModelLineage, SourceRef};
use serde::{Deserialize, Serialize};

use super::store::Entry;

/// 1: the first shape.
const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
struct File {
    format: u32,
    server: String,
    models: Vec<Model>,
}

#[derive(Serialize, Deserialize)]
struct Model {
    name: String,
    base: u64,
    compiled: Option<u64>,
    lineage: Option<Lineage>,
}

#[derive(Serialize, Deserialize)]
struct Lineage {
    columns: Vec<Column>,
    from_compiled: bool,
}

#[derive(Serialize, Deserialize)]
struct Column {
    name: String,
    direct: bool,
    sources: Vec<Ref>,
    /// The select item's end offset and the `derived` flag of
    /// `ColumnLineage` (both off the wire), kept so an entry reads back as
    /// it was; defaulted when absent. Nothing inline reads this cache:
    /// inlay hints and hovers take the open document's lineage, computed
    /// from its text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end: Option<usize>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    derived: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Ref {
    Model {
        name: String,
        column: String,
    },
    Source {
        source: String,
        table: String,
        column: String,
    },
    Unknown {
        name: String,
        column: String,
    },
}

pub fn cache_path(root: &Path) -> PathBuf {
    root.join("target").join(".cogitator-lineage-cache")
}

/// The persisted entries, when the file is there and of this build.
pub fn load(root: &Path) -> Option<HashMap<String, Entry>> {
    let path = cache_path(root);
    let text = std::fs::read_to_string(&path).ok()?;
    let file: File = match serde_json::from_str(&text) {
        Ok(file) => file,
        Err(err) => {
            tracing::debug!(path = %path.display(), %err, "lineage cache unreadable; cold start");
            return None;
        }
    };
    if file.format != FORMAT || file.server != crate::VERSION {
        tracing::debug!(path = %path.display(), "lineage cache from another build; cold start");
        return None;
    }
    Some(
        file.models
            .into_iter()
            .map(|m| {
                let entry = Entry {
                    base: m.base,
                    compiled: m.compiled,
                    lineage: Arc::new(m.lineage.map(from_disk)),
                };
                (m.name, entry)
            })
            .collect(),
    )
}

/// Same-directory temp file and rename, as the scan cache.
pub fn save(root: &Path, entries: &[(String, Entry)]) {
    let path = cache_path(root);
    let Some(dir) = path.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let file = File {
        format: FORMAT,
        server: crate::VERSION.to_string(),
        models: entries
            .iter()
            .map(|(name, e)| Model {
                name: name.clone(),
                base: e.base,
                compiled: e.compiled,
                lineage: (*e.lineage).as_ref().map(to_disk),
            })
            .collect(),
    };
    let Ok(json) = serde_json::to_string(&file) else {
        return;
    };
    let tmp = path.with_extension("tmp");
    if let Err(err) = std::fs::write(&tmp, json).and_then(|()| std::fs::rename(&tmp, &path)) {
        tracing::debug!(path = %path.display(), %err, "lineage cache save skipped");
    }
}

pub fn remove(root: &Path) {
    let _ = std::fs::remove_file(cache_path(root));
}

fn to_disk(lineage: &ModelLineage) -> Lineage {
    Lineage {
        from_compiled: lineage.origin == Some(LineageOrigin::Compiled),
        columns: lineage
            .columns
            .iter()
            .map(|c| Column {
                name: c.name.clone(),
                direct: c.direct,
                end: c.end.map(|e| e as usize),
                derived: c.derived,
                sources: c
                    .sources
                    .iter()
                    .map(|s| match s.clone() {
                        SourceRef::Model { name, column } => Ref::Model { name, column },
                        SourceRef::Source {
                            source,
                            table,
                            column,
                        } => Ref::Source {
                            source,
                            table,
                            column,
                        },
                        SourceRef::Unknown { name, column } => Ref::Unknown { name, column },
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn from_disk(lineage: Lineage) -> ModelLineage {
    ModelLineage {
        origin: lineage.from_compiled.then_some(LineageOrigin::Compiled),
        columns: lineage
            .columns
            .into_iter()
            .map(|c| ColumnLineage {
                name: c.name,
                direct: c.direct,
                derived: c.derived,
                end: c.end.and_then(|e| u32::try_from(e).ok()),
                sources: c
                    .sources
                    .into_iter()
                    .map(|s| match s {
                        Ref::Model { name, column } => SourceRef::Model { name, column },
                        Ref::Source {
                            source,
                            table,
                            column,
                        } => SourceRef::Source {
                            source,
                            table,
                            column,
                        },
                        Ref::Unknown { name, column } => SourceRef::Unknown { name, column },
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_survive_a_round_trip() {
        let root =
            std::env::temp_dir().join(format!("cogitator-lineage-persist-{}", std::process::id()));
        let lineage = ModelLineage {
            origin: Some(LineageOrigin::Compiled),
            columns: vec![ColumnLineage {
                name: "id".into(),
                direct: true,
                derived: true,
                end: Some(42),
                sources: vec![
                    SourceRef::Model {
                        name: "stg".into(),
                        column: "id".into(),
                    },
                    SourceRef::Source {
                        source: "s".into(),
                        table: "t".into(),
                        column: "c".into(),
                    },
                ],
            }],
        };
        let entries = vec![
            (
                "a".to_string(),
                Entry {
                    base: 1,
                    compiled: Some(2),
                    lineage: Arc::new(Some(lineage)),
                },
            ),
            (
                "b".to_string(),
                Entry {
                    base: u64::MAX,
                    compiled: None,
                    lineage: Arc::new(None),
                },
            ),
        ];
        save(&root, &entries);
        let back = load(&root).expect("loads");
        assert_eq!(back.len(), 2);
        let a = &back["a"];
        assert_eq!((a.base, a.compiled), (1, Some(2)));
        let a = (*a.lineage).as_ref().unwrap();
        assert_eq!(a.origin, Some(LineageOrigin::Compiled));
        assert!(a.columns[0].direct, "direct survives, though off the wire");
        assert!(a.columns[0].derived, "derived survives too");
        assert_eq!(a.columns[0].end, Some(42));
        assert_eq!(a.columns[0].sources.len(), 2);
        assert_eq!(back["b"].base, u64::MAX);
        assert!(back["b"].lineage.is_none());

        std::fs::write(cache_path(&root), "{not json").unwrap();
        assert!(load(&root).is_none(), "a corrupt file is a cold start");
        std::fs::write(
            cache_path(&root),
            r#"{"format":999,"server":"x","models":[]}"#,
        )
        .unwrap();
        assert!(load(&root).is_none(), "another format is a cold start");
        let _ = std::fs::remove_dir_all(&root);
    }
}

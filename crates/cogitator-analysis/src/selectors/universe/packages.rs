//! The root project and its installed packages: where each lives and its
//! resource paths, to place a file in its package and fqn.

use std::path::{Path, PathBuf};

use super::build::{Builder, Read};
use crate::DbtContext;

/// One package's root and resource paths.
pub(crate) struct Package {
    pub name: String,
    pub root: PathBuf,
    pub paths: Vec<String>,
}

pub(super) fn packages(ctx: &DbtContext, root: &Path, read: &Read<'_>) -> Vec<Package> {
    let p = &ctx.project;
    let own = [
        &p.model_paths,
        &p.seed_paths,
        &p.snapshot_paths,
        &p.test_paths,
        &p.analysis_paths,
    ];
    let mut out = vec![Package {
        name: p.name.clone(),
        root: root.to_path_buf(),
        paths: own.into_iter().flatten().cloned().collect(),
    }];
    for pkg in &ctx.package_variables {
        let path = pkg.root.join("dbt_project.yml");
        let file = read(&path)
            .and_then(|text| cogitator_schemas::project::parse_dbt_project(&text, &path).ok());
        let mut paths: Vec<String> = file
            .map(|f| {
                [
                    f.model_paths,
                    f.seed_paths,
                    f.snapshot_paths,
                    f.test_paths,
                    f.analysis_paths,
                ]
                .into_iter()
                .flatten()
                .collect()
            })
            .unwrap_or_default();
        if paths.is_empty() {
            paths = ["models", "seeds", "snapshots", "tests", "analyses"]
                .map(String::from)
                .to_vec();
        }
        out.push(Package {
            name: pkg.name.clone(),
            root: pkg.root.clone(),
            paths,
        });
    }
    out
}

impl Builder<'_> {
    /// The package a file belongs to: the deepest package root above it.
    pub fn package_of(&self, uri: &Path) -> &Package {
        self.packages[1..]
            .iter()
            .filter(|p| uri.starts_with(&p.root))
            .max_by_key(|p| p.root.components().count())
            .unwrap_or(&self.packages[0])
    }

    /// (package, path relative to it, directories under its resource
    /// path) of a file.
    pub fn locate(&self, uri: &Path) -> (String, String, Vec<String>) {
        let pkg = self.package_of(uri);
        let rel: Vec<String> = uri
            .strip_prefix(&pkg.root)
            .map(|r| {
                r.components()
                    .map(|c| c.as_os_str().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();
        let dirs = &rel[..rel.len().saturating_sub(1)];
        let skip = pkg
            .paths
            .iter()
            .map(|p| p.trim_end_matches('/').split('/').collect::<Vec<_>>())
            .filter(|parts| dirs.len() >= parts.len() && dirs[..parts.len()] == parts[..])
            .map(|parts| parts.len())
            .max()
            .unwrap_or(0);
        (pkg.name.clone(), rel.join("/"), dirs[skip..].to_vec())
    }
}

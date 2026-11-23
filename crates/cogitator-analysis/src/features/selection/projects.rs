//! The projects whose nodes the universe holds — the root and every
//! installed package (`dbt_packages/*`, symlinks of local packages
//! included) — their properties files, and the folder configs a node
//! takes, the root's `models: <package>:` over a package's own.

use std::path::{Path, PathBuf};

use cogitator_schemas::project::ProjectFile;

use super::folder_config::{self, FolderConfig};
use crate::DbtContext;

/// A project whose nodes the universe holds: the root, or a package.
pub(super) struct Project {
    pub root: PathBuf,
    pub file: ProjectFile,
    pub is_root: bool,
}

/// The root project and its installed packages.
pub(super) fn projects(ctx: &DbtContext, root: &Path) -> Vec<Project> {
    let mut out = vec![Project {
        root: root.to_path_buf(),
        file: ctx.project.clone(),
        is_root: true,
    }];
    let install = if ctx.project.packages_install_path.is_empty() {
        "dbt_packages"
    } else {
        ctx.project.packages_install_path.as_str()
    };
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root.join(install))
        .map(|d| {
            d.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    for dir in dirs {
        let file = crate::context::load_project_file_at(&dir);
        if !file.name.is_empty() {
            out.push(Project {
                root: dir,
                file,
                is_root: false,
            });
        }
    }
    out
}

/// Each project's properties files, read once for every builder.
pub(super) struct YamlFile {
    pub path: PathBuf,
    pub text: String,
    /// The index of its project in `projects`.
    pub project: usize,
}

pub(super) fn read_yaml(projects: &[Project]) -> Vec<YamlFile> {
    let mut out = Vec::new();
    for (i, p) in projects.iter().enumerate() {
        let f = &p.file;
        let mut files: Vec<PathBuf> = f
            .model_paths
            .iter()
            .chain(&f.seed_paths)
            .chain(&f.snapshot_paths)
            .chain(&f.macro_paths)
            .flat_map(|rel| {
                let dir = p.root.join(rel);
                let mut files = crate::util::walk_filepath(&dir, ".yml");
                files.extend(crate::util::walk_filepath(&dir, ".yaml"));
                files
            })
            .collect();
        files.sort();
        files.dedup();
        out.extend(files.into_iter().filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            Some(YamlFile {
                path,
                text,
                project: i,
            })
        }));
    }
    out
}

/// A file's path inside its own project.
pub(super) fn path_in(project: &Project, file: &Path) -> String {
    let rel = file.strip_prefix(&project.root).unwrap_or(file);
    rel.iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The folder configs of a node of `package` in `folders`: the package's
/// own dbt_project.yml, then the root's over it.
pub(super) fn folder_of(
    projects: &[Project],
    package: &str,
    tree_key: &str,
    folders: &[String],
) -> FolderConfig {
    let dirs: Vec<&str> = folders.iter().map(String::as_str).collect();
    let mut config = FolderConfig::default();
    let own = projects
        .iter()
        .find(|p| p.file.name == package && !p.is_root);
    for p in own.into_iter().chain(projects.iter().filter(|p| p.is_root)) {
        let layer = folder_config::resolve(p.file.resource_configs.get(tree_key), package, &dirs);
        if layer.materialized.is_some() {
            config.materialized = layer.materialized;
        }
        for tag in layer.tags {
            if !config.tags.contains(&tag) {
                config.tags.push(tag);
            }
        }
    }
    config
}

//! Installed packages under the install path (03 §8).

use std::path::Path;

use codex::project::ProjectFile;

use super::{load_project_file, ProjectDetails};

/// Installed packages (03 §8): direct subdirectories of the install path,
/// each with its own parsed+defaulted dbt_project.yml.
pub(super) fn package_details(root: &Path, project: &ProjectFile) -> Vec<ProjectDetails> {
    if project.packages_install_path.is_empty() {
        return Vec::new();
    }
    let install = root.join(&project.packages_install_path);
    std::fs::read_dir(install)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| {
                    let pkg_root = e.path();
                    let file = load_project_file(&pkg_root);
                    ProjectDetails {
                        root: pkg_root,
                        file,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

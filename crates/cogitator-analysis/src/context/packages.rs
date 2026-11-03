//! Installed packages under the install path (03 §8).

use std::path::Path;

use cogitator_schemas::project::ProjectFile;

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

/// Declared packages from `<root>/packages.yml` (or the dbt ≥1.6
/// `dependencies.yml`), reduced to their installable names: the hub
/// id's last segment, a git url's repo stem, or a local path's dir
/// name. Lenient on purpose (08 §4).
pub(super) fn declared_packages(root: &Path) -> Vec<String> {
    use cogitator_schemas::yaml::{as_str, get, items};
    use saphyr::{LoadableYamlNode, MarkedYaml};

    let text = ["packages.yml", "dependencies.yml"]
        .iter()
        .find_map(|name| std::fs::read_to_string(root.join(name)).ok());
    let Some(text) = text else {
        return Vec::new();
    };
    let Ok(docs) = MarkedYaml::load_from_str(&text) else {
        return Vec::new();
    };
    let Some(top) = docs.first() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in get(top, "packages").into_iter().flat_map(items) {
        let name = get(entry, "package")
            .and_then(as_str)
            .map(|hub| hub.rsplit('/').next().unwrap_or(hub).to_string())
            .or_else(|| {
                get(entry, "git").and_then(as_str).map(|url| {
                    url.trim_end_matches(".git")
                        .rsplit('/')
                        .next()
                        .unwrap_or(url)
                        .to_string()
                })
            })
            .or_else(|| {
                get(entry, "local").and_then(as_str).map(|path| {
                    std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(path)
                        .to_string()
                })
            });
        if let Some(name) = name {
            out.push(name);
        }
    }
    out
}

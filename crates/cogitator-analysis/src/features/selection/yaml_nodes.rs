//! The nodes properties files define: snapshots (dbt 1.9's YAML
//! `snapshots:`), generic and unit tests, and the versions of models.

use std::collections::HashMap;

use super::deps::parent_dep;
use super::projects::{folder_of, path_in, Project, YamlFile};
use super::universe::{folders, uri, Dep, Draft, Node};
use super::universe_nodes::fqn;
use super::universe_yaml::{model_versions, yaml_snapshots as read_snapshots, ModelVersion};
use crate::features::quality::test_nodes::{yaml_test_nodes, TestParent};
use crate::DbtContext;

/// Every version of every model, by the file stem it is defined in.
pub(super) fn versions(yaml: &[YamlFile]) -> HashMap<String, ModelVersion> {
    yaml.iter()
        .flat_map(|f| model_versions(&f.text))
        .map(|v| (v.file.clone(), v))
        .collect()
}

/// Snapshots a YAML `snapshots:` entry defines (no `{% snapshot %}` file).
pub(super) fn yaml_snapshots(
    ctx: &DbtContext,
    projects: &[Project],
    yaml: &[YamlFile],
    out: &mut Vec<Draft>,
) {
    for file in yaml {
        let project = &projects[file.project];
        for snap in read_snapshots(&file.text) {
            if ctx.model_detail_map.contains_key(&snap.name) {
                continue;
            }
            let path = path_in(project, &file.path);
            let name = &project.file.name;
            let mut node = Node::new("snapshot", &snap.name, name, path.clone());
            node.fqn = fqn(
                name,
                folders(&path, &project.file.snapshot_paths),
                &snap.name,
            );
            let folder = folder_of(
                projects,
                name,
                "snapshots",
                &folders(&path, &project.file.snapshot_paths),
            );
            node.tags = snap.tags.clone();
            for tag in folder.tags {
                if !node.tags.contains(&tag) {
                    node.tags.push(tag);
                }
            }
            node.materialized = Some("snapshot".to_string());
            node.uri = uri(&file.path);
            node.range = Some(snap.range);
            node.patch_path = Some(path);
            let deps = snap.parents.iter().map(parent_dep).collect();
            out.push(Draft {
                node,
                deps,
                aliases: Vec::new(),
            });
        }
    }
}

/// Generic and unit tests of every project's properties files. A unit
/// test of a versioned model is one test per version, `<name>_v<N>`, as
/// dbt makes them.
pub(super) fn yaml_tests(
    projects: &[Project],
    yaml: &[YamlFile],
    versions: &HashMap<String, ModelVersion>,
    out: &mut Vec<Draft>,
) {
    for file in yaml {
        let project = &projects[file.project];
        let package = &project.file.name;
        let path = path_in(project, &file.path);
        let dirs = folders(&path, &project.file.model_paths);
        for test in yaml_test_nodes(&file.text) {
            let model = match test.parents.first() {
                Some(TestParent::Node(m)) => Some(m.clone()),
                _ => None,
            };
            let model_versions: Vec<&ModelVersion> = match (&model, test.unit) {
                (Some(m), true) => versions.values().filter(|v| &v.model == m).collect(),
                _ => Vec::new(),
            };
            let instances: Vec<(String, Vec<Dep>)> = if model_versions.is_empty() {
                vec![(
                    test.name.clone(),
                    test.parents.iter().map(parent_dep).collect(),
                )]
            } else {
                model_versions
                    .iter()
                    .map(|v| {
                        (
                            format!("{}_v{}", test.name, v.version),
                            vec![Dep::Model(format!("{}_v{}", v.model, v.version))],
                        )
                    })
                    .collect()
            };
            for (name, deps) in instances {
                let resource_type = if test.unit { "unit_test" } else { "test" };
                let mut node = Node::new(resource_type, &name, package, path.clone());
                node.fqn = fqn(package, dirs.clone(), &name);
                if test.unit {
                    if let Some(m) = &model {
                        node.unique_id = format!("unit_test.{package}.{m}.{name}");
                    }
                    node.test_type = Some("unit");
                } else {
                    node.materialized = Some("test".to_string());
                    node.test_type = Some("generic");
                    node.test_name = Some(test.test.clone());
                }
                node.uri = uri(&file.path);
                node.range = Some(test.range);
                out.push(Draft {
                    node,
                    deps,
                    aliases: Vec::new(),
                });
            }
        }
    }
}

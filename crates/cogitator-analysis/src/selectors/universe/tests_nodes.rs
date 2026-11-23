//! Tests (generic, unit, singular), analyses, exposures, metrics and
//! semantic models as selectable nodes, each linked to what it reads.

use super::build::Builder;
use super::sql_deps::sql_deps;
use super::{Node, ResourceType, TestInfo};
use crate::context::exposures::{CitizenDetails, RefTarget};
use crate::features::quality::test_scope::Owner;
use crate::graph::NodeKind;

impl Builder<'_> {
    pub(super) fn tests(&mut self) {
        self.generic_tests();
        self.unit_tests();
        for (kind, resource) in [
            (NodeKind::Test, ResourceType::Test),
            (NodeKind::Analysis, ResourceType::Analysis),
        ] {
            let mut files: Vec<_> = self
                .ctx
                .graph
                .nodes()
                .filter(|n| n.kind == kind && !n.uri.as_os_str().is_empty())
                .map(|n| (n.name.clone(), n.uri.clone()))
                .collect();
            files.sort();
            for (name, uri) in files {
                let (package, path, dirs) = self.locate(&uri);
                let mut node = Node::new(resource, &name, &package);
                node.fqn = std::iter::once(package)
                    .chain(dirs)
                    .chain(std::iter::once(name.clone()))
                    .collect();
                let section = if resource == ResourceType::Test {
                    "data_tests"
                } else {
                    "analyses"
                };
                node.config = self.config(section, &node.fqn, resource);
                node.tags = super::config::tag_list(node.config.get("tags"));
                node.path = path;
                node.uri = uri.clone();
                if resource == ResourceType::Test {
                    node.test = Some(TestInfo {
                        test_type: "singular",
                        test_name: None,
                    });
                }
                let idx = self.u.push(node);
                if let Some(sql) = (self.read)(&uri) {
                    self.deps.push((idx, sql_deps(&sql, &self.ctx.dialect)));
                }
            }
        }
    }

    fn generic_tests(&mut self) {
        let mut pending = Vec::new();
        for (path, props) in &self.properties {
            for test in &props.tests {
                let owner = match &test.owner {
                    Owner::Node { name, version } => self.u.nodes.iter().position(|n| {
                        n.name == *name
                            && n.resource_type != ResourceType::Source
                            && n.version_number == *version
                    }),
                    Owner::Source { source, table } => self.u.find_source(source, table),
                };
                let (_, rel, dirs) = self.locate(path);
                let package = owner
                    .map(|o| self.u.nodes[o].package.clone())
                    .unwrap_or_else(|| self.u.root_package.clone());
                let mut node = Node::new(ResourceType::Test, &test.name, &package);
                node.fqn = std::iter::once(package)
                    .chain(dirs)
                    .chain(std::iter::once(test.name.clone()))
                    .collect();
                node.config = self.config("data_tests", &node.fqn, ResourceType::Test);
                node.tags = super::config::tag_list(node.config.get("tags"));
                node.path = rel;
                node.uri = path.clone();
                node.range = test.range;
                node.test = Some(TestInfo {
                    test_type: "generic",
                    test_name: Some(test.test_name.clone()),
                });
                let refs: Vec<usize> = test
                    .also_refs
                    .iter()
                    .filter_map(|r| self.by_ref.get(r).copied())
                    .collect();
                pending.push((node, owner, refs));
            }
        }
        for (node, owner, refs) in pending {
            let idx = self.u.push(node);
            for parent in owner.into_iter().chain(refs) {
                self.u.link(idx, parent);
            }
        }
    }

    fn unit_tests(&mut self) {
        let mut pending = Vec::new();
        for (path, props) in &self.properties {
            for unit in &props.unit_tests {
                let model = unit
                    .model
                    .as_ref()
                    .and_then(|m| self.by_ref.get(m).copied());
                let package = model
                    .map(|m| self.u.nodes[m].package.clone())
                    .unwrap_or_else(|| self.u.root_package.clone());
                let (_, rel, dirs) = self.locate(path);
                let mut node = Node::new(ResourceType::UnitTest, &unit.name, &package);
                node.fqn = std::iter::once(package)
                    .chain(dirs)
                    .chain(unit.model.clone())
                    .chain(std::iter::once(unit.name.clone()))
                    .collect();
                node.config = self.config("unit_tests", &node.fqn, ResourceType::UnitTest);
                node.path = rel;
                node.uri = path.clone();
                node.range = unit.range;
                node.test = Some(TestInfo {
                    test_type: "unit",
                    test_name: None,
                });
                pending.push((node, model));
            }
        }
        for (node, model) in pending {
            let idx = self.u.push(node);
            if let Some(model) = model {
                self.u.link(idx, model);
            }
        }
    }

    pub(super) fn citizens(&mut self) {
        let maps = [
            (
                &self.ctx.exposure_detail_map,
                ResourceType::Exposure,
                "exposures",
            ),
            (&self.ctx.metric_detail_map, ResourceType::Metric, "metrics"),
            (
                &self.ctx.semantic_model_detail_map,
                ResourceType::SemanticModel,
                "semantic-models",
            ),
        ];
        for (map, resource, section) in maps {
            let mut citizens: Vec<&CitizenDetails> = map.values().collect();
            citizens.sort_by(|a, b| a.name.cmp(&b.name));
            for citizen in citizens {
                let (package, path, dirs) = self.locate(&citizen.uri);
                let mut node = Node::new(resource, &citizen.name, &package);
                node.fqn = std::iter::once(package)
                    .chain(dirs)
                    .chain(std::iter::once(citizen.name.clone()))
                    .collect();
                node.config = self.config(section, &node.fqn, resource);
                node.tags = super::config::tag_list(node.config.get("tags"));
                node.path = path;
                node.uri = citizen.uri.clone();
                node.range = citizen.range;
                let idx = self.u.push(node);
                for target in &citizen.depends {
                    let parent = match target {
                        RefTarget::Model(model) => self.by_ref.get(model).copied(),
                        RefTarget::Source(source, table) => self.u.find_source(source, table),
                    };
                    if let Some(parent) = parent {
                        self.u.link(idx, parent);
                    }
                }
            }
        }
    }
}

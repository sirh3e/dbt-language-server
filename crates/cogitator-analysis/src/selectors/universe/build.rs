//! Building the [`Universe`] from the context: models, seeds and
//! snapshots here, tests and the other resources in `tests_nodes`.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::config::{self, Config};
use super::packages::Package;
use super::properties::{self, Properties};
use super::sql_deps::{sql_deps, SqlDeps};
use super::{Node, ResourceType, Universe, VersionKind};
use crate::context::ModelKind;
use crate::DbtContext;

/// The shell's file access: a file's text, `None` when unreadable.
pub type Read<'a> = dyn Fn(&Path) -> Option<String> + 'a;

pub(super) struct Builder<'a> {
    pub ctx: &'a DbtContext,
    pub read: &'a Read<'a>,
    pub u: Universe,
    pub project: Value,
    pub packages: Vec<Package>,
    /// ref name → node (a versioned model's base name is its latest).
    pub by_ref: HashMap<String, usize>,
    /// Each node's SQL dependencies, linked once every node exists.
    pub deps: Vec<(usize, SqlDeps)>,
    pub properties: Vec<(PathBuf, Properties)>,
}

/// Every selectable node of the project and its installed packages.
pub fn build(ctx: &DbtContext, read: &Read<'_>) -> Universe {
    let Some(root) = ctx.project_root.clone() else {
        return Universe::default();
    };
    let project = read(&root.join("dbt_project.yml"))
        .and_then(|text| yaml_json(&crate::util::resolve_env_vars(&text)))
        .unwrap_or(Value::Null);
    let mut b = Builder {
        ctx,
        read,
        u: Universe {
            root_package: ctx.project.name.clone(),
            ..Universe::default()
        },
        project,
        packages: super::packages::packages(ctx, &root, read),
        by_ref: HashMap::new(),
        deps: Vec::new(),
        properties: Vec::new(),
    };
    b.read_properties();
    b.models();
    b.sources();
    b.tests();
    b.citizens();
    b.link_sql();
    b.u
}

fn yaml_json(text: &str) -> Option<Value> {
    use saphyr::{LoadableYamlNode, MarkedYaml};
    let docs = MarkedYaml::load_from_str(text).ok()?;
    docs.first().map(cogitator_schemas::yaml::to_json)
}

impl Builder<'_> {
    /// The resolved config of a node from its project section on.
    pub fn config(&self, section: &str, fqn: &[String], resource: ResourceType) -> Config {
        let mut out = config::defaults(resource);
        let project = config::from_project(config::section(&self.project, section), fqn);
        config::layer(&mut out, &project);
        out
    }

    fn read_properties(&mut self) {
        let mut files: BTreeSet<PathBuf> = self
            .ctx
            .model_detail_map
            .values()
            .filter_map(|m| m.schema_uri.clone())
            .collect();
        files.extend(self.ctx.source_detail_map.values().map(|s| s.uri.clone()));
        self.properties = files
            .into_iter()
            .filter_map(|path| Some((path.clone(), properties::read(&(self.read)(&path)?))))
            .collect();
    }

    fn models(&mut self) {
        let mut names: Vec<&String> = self.ctx.model_detail_map.keys().collect();
        names.sort();
        // a version's file → (base name, v, kind)
        let mut versions: HashMap<String, (String, String, VersionKind)> = HashMap::new();
        for (_, props) in &self.properties {
            for entry in props.entries.iter().filter(|e| !e.versions.is_empty()) {
                let latest = entry.latest_version.clone().unwrap_or_else(|| {
                    entry
                        .versions
                        .iter()
                        .map(|(v, _)| v.clone())
                        .max_by(cmp_version)
                        .unwrap_or_default()
                });
                for (v, defined_in) in &entry.versions {
                    let mut file = defined_in.clone().unwrap_or(format!("{}_v{v}", entry.name));
                    if !self.ctx.model_detail_map.contains_key(&file) && *v == latest {
                        file = entry.name.clone();
                    }
                    let kind = match cmp_version(v, &latest) {
                        std::cmp::Ordering::Equal => VersionKind::Latest,
                        std::cmp::Ordering::Greater => VersionKind::Prerelease,
                        std::cmp::Ordering::Less => VersionKind::Old,
                    };
                    versions.insert(file, (entry.name.clone(), v.clone(), kind));
                }
            }
        }
        for file_name in names {
            let details = &self.ctx.model_detail_map[file_name];
            let (resource, section) = match details.kind {
                ModelKind::Model => (ResourceType::Model, "models"),
                ModelKind::Seed => (ResourceType::Seed, "seeds"),
                ModelKind::Snapshot => (ResourceType::Snapshot, "snapshots"),
            };
            let (_, path, dirs) = self.locate(&details.uri);
            let package = details.project_name.clone();
            let version = versions.get(file_name);
            let name = version.map_or(file_name.clone(), |(base, _, _)| base.clone());
            let mut node = Node::new(resource, &name, &package);
            node.fqn = std::iter::once(package.clone())
                .chain(dirs)
                .chain(std::iter::once(name.clone()))
                .chain(version.map(|(_, v, _)| format!("v{v}")))
                .collect();
            let mut config = self.config(section, &node.fqn, resource);
            if let Some(entry) = self
                .properties
                .iter()
                .flat_map(|(_, p)| &p.entries)
                .find(|e| e.name == name)
            {
                config::layer(&mut config, &entry.config);
            }
            let sql = (details.kind != ModelKind::Seed)
                .then(|| (self.read)(&details.uri))
                .flatten();
            if let Some(materialized) = sql
                .as_deref()
                .and_then(crate::features::config_materialized)
            {
                config.insert("materialized".into(), Value::from(materialized));
            }
            let mut tags = config::tag_list(config.get("tags"));
            for tag in &details.tags {
                if !tags.contains(tag) {
                    tags.push(tag.clone());
                }
            }
            config.insert("tags".into(), Value::from(tags.clone()));
            node.tags = tags;
            node.group = details.group.clone().or_else(|| str_of(&config, "group"));
            node.access = details.access.clone().or_else(|| str_of(&config, "access"));
            if resource == ResourceType::Model && node.access.is_none() {
                node.access = Some("protected".into());
            }
            node.config = config;
            node.path = path;
            node.uri = details.uri.clone();
            if let Some((_, v, kind)) = version {
                node.version = Some(*kind);
                node.version_number = Some(v.clone());
            }
            let latest = version.is_none_or(|(_, _, k)| *k == VersionKind::Latest);
            let idx = self.u.push(node);
            self.by_ref.insert(file_name.clone(), idx);
            if latest {
                self.by_ref.insert(name, idx);
            }
            if let Some(sql) = sql {
                self.deps.push((idx, sql_deps(&sql, &self.ctx.dialect)));
            }
        }
    }

    fn sources(&mut self) {
        let mut out = Vec::new();
        for (path, props) in &self.properties {
            for table in &props.sources {
                let (package, rel, dirs) = self.locate(path);
                let mut node = Node::new(ResourceType::Source, &table.table, &package);
                node.fqn = std::iter::once(package)
                    .chain(dirs)
                    .chain([table.source.clone(), table.table.clone()])
                    .collect();
                let mut config = self.config("sources", &node.fqn, ResourceType::Source);
                config::layer(&mut config, &table.config);
                node.tags = config::tag_list(config.get("tags"));
                node.config = config;
                node.path = rel;
                node.uri = path.clone();
                node.range = table.range;
                node.source = Some((table.source.clone(), table.table.clone()));
                out.push(node);
            }
        }
        for node in out {
            self.u.push(node);
        }
    }

    /// Link every node to what its SQL refs and sources.
    fn link_sql(&mut self) {
        for (child, deps) in std::mem::take(&mut self.deps) {
            for name in &deps.refs {
                if let Some(&parent) = self.by_ref.get(name) {
                    self.u.link(child, parent);
                }
            }
            for (source, table) in &deps.sources {
                if let Some(parent) = self.u.find_source(source, table) {
                    self.u.link(child, parent);
                }
            }
        }
    }
}

fn str_of(config: &Config, key: &str) -> Option<String> {
    config.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Versions compare as numbers when both are, else as text.
fn cmp_version(a: &String, b: &String) -> std::cmp::Ordering {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        _ => a.cmp(b),
    }
}

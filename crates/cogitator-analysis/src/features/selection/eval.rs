//! Evaluate a parsed selection over the universe the way dbt-core's
//! NodeSelector does (graph/selector.py, graph/selector_methods.py):
//! each criterion picks its nodes by method, widens them by its graph
//! operators, then takes in the tests directly on them (indirect
//! selection, "eager"); a group intersects its criteria, the groups are
//! unioned, and the exclusion — evaluated the same way — is subtracted.

use std::collections::{BTreeSet, HashMap};

use super::methods::{matches, RESOURCE_TYPES};
use super::parse::{parse, Criterion, Depth, Selection, SelectionError};
use super::universe::Universe;
use crate::context::SelectorDetails;

pub type Set = BTreeSet<usize>;

pub struct Eval<'a> {
    pub universe: &'a Universe,
    pub selectors: &'a HashMap<String, SelectorDetails>,
    pub root_package: &'a str,
}

impl Eval<'_> {
    /// The nodes of `selection`, without those of `exclude`.
    pub fn select(
        &self,
        selection: &Selection,
        exclude: Option<&Selection>,
    ) -> Result<Set, SelectionError> {
        let mut out = if selection.groups.is_empty() {
            // no selection at all: everything, as `dbt ls` lists it
            (0..self.universe.nodes.len()).collect()
        } else {
            self.union(selection, 0)?
        };
        if let Some(exclude) = exclude {
            for i in self.union(exclude, 0)? {
                out.remove(&i);
            }
        }
        Ok(out)
    }

    /// What an exclusion takes out (nothing for an empty one).
    pub fn excluded(&self, selection: &Selection) -> Result<Set, SelectionError> {
        self.union(selection, 0)
    }

    fn union(&self, selection: &Selection, nesting: usize) -> Result<Set, SelectionError> {
        let mut out = Set::new();
        for group in &selection.groups {
            let mut acc: Option<Set> = None;
            for criterion in group {
                let set = self.criterion(criterion, nesting)?;
                acc = Some(match acc {
                    None => set,
                    Some(prev) => prev.intersection(&set).copied().collect(),
                });
            }
            out.extend(acc.unwrap_or_default());
        }
        Ok(out)
    }

    fn criterion(&self, c: &Criterion, nesting: usize) -> Result<Set, SelectionError> {
        let base = self.method(c, nesting)?;
        let mut out = base.clone();
        if c.childrens_parents {
            let descendants = self.walk(&base, Depth::All, true);
            out.extend(&descendants);
            let mut with_descendants = base.clone();
            with_descendants.extend(descendants);
            out.extend(self.walk(&with_descendants, Depth::All, false));
        }
        if let Some(depth) = c.parents {
            out.extend(self.walk(&base, depth, false));
        }
        if let Some(depth) = c.children {
            out.extend(self.walk(&base, depth, true));
        }
        // eager indirect selection: the tests directly on what is selected;
        // a selector's definition applied it at its own leaves already
        // (its `exclude` would be undone otherwise)
        if c.method == "selector" {
            return Ok(out);
        }
        let tests: Vec<usize> = out
            .iter()
            .flat_map(|&i| self.universe.children(i).iter().copied())
            .filter(|&t| matches!(self.universe.nodes[t].resource_type, "test" | "unit_test"))
            .collect();
        out.extend(tests);
        Ok(out)
    }

    /// Breadth-first ancestors (`down` false) or descendants, `depth` levels.
    fn walk(&self, from: &Set, depth: Depth, down: bool) -> Set {
        let mut seen: Set = from.clone();
        let mut out = Set::new();
        let mut frontier: Vec<usize> = from.iter().copied().collect();
        let mut level = 0u32;
        while !frontier.is_empty() {
            if let Depth::Levels(n) = depth {
                if level >= n {
                    break;
                }
            }
            level += 1;
            let mut next = Vec::new();
            for i in frontier {
                let edges = if down {
                    self.universe.children(i)
                } else {
                    self.universe.parents(i)
                };
                for &j in edges {
                    if seen.insert(j) {
                        out.insert(j);
                        next.push(j);
                    }
                }
            }
            frontier = next;
        }
        out
    }

    fn method(&self, c: &Criterion, nesting: usize) -> Result<Set, SelectionError> {
        let v = c.value.as_str();
        let err = |message: String| SelectionError {
            message,
            start: c.start,
            end: c.end,
        };
        if c.method == "selector" {
            return self.selector(v, nesting).map_err(|e| match e {
                Nested::Unknown => err(format!("selectors.yml has no selector `{v}`")),
                Nested::Cycle => err(format!("selector `{v}` refers back to itself")),
                Nested::Inner(inner) => err(format!("in selector `{v}`: {}", inner.message)),
            });
        }
        if c.method == "version" {
            return Err(err(
                "`version:` is not evaluated in the editor yet".to_string()
            ));
        }
        if c.method == "config" {
            let key = c.args.join(".");
            if !["materialized", "tags", "access", "group"].contains(&key.as_str()) {
                return Err(err(format!(
                    "`config.{key}:` is not known to the editor: config.materialized, config.tags, config.access and config.group are"
                )));
            }
        }
        if c.method == "resource_type" && !RESOURCE_TYPES.contains(&v) {
            return Err(err(format!(
                "`{v}` is no resource type: {}",
                RESOURCE_TYPES.join(", ")
            )));
        }
        if c.method == "test_type" && !["generic", "singular", "unit", "data"].contains(&v) {
            return Err(err(format!(
                "`{v}` is no test type: generic, singular, unit or data"
            )));
        }
        let root = self.root_package;
        Ok(self
            .universe
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| matches(c, n, root))
            .map(|(i, _)| i)
            .collect())
    }

    fn selector(&self, name: &str, nesting: usize) -> Result<Set, Nested> {
        if nesting > 16 {
            return Err(Nested::Cycle);
        }
        let details = self.selectors.get(name).ok_or(Nested::Unknown)?;
        let definition: serde_json::Value =
            serde_json::from_str(&details.definition_text).map_err(|_| Nested::Unknown)?;
        self.definition(&definition, nesting + 1)
    }

    /// A selectors.yml `definition`: a CLI string, a `{method, value}`
    /// mapping (or the `{tag: x}` shorthand), or a union/intersection of
    /// definitions, any of whose items may be an `exclude`.
    fn definition(&self, def: &serde_json::Value, nesting: usize) -> Result<Set, Nested> {
        use serde_json::Value;
        match def {
            Value::String(text) => {
                let sel = parse(text).map_err(Nested::Inner)?;
                self.union(&sel, nesting).map_err(Nested::Inner)
            }
            Value::Object(map) => {
                for (key, combine) in [("union", true), ("intersection", false)] {
                    if let Some(Value::Array(items)) = map.get(key) {
                        let mut acc: Option<Set> = None;
                        let mut excluded = Set::new();
                        for item in items {
                            if let Some(ex) = item.get("exclude") {
                                let list =
                                    ex.as_array().cloned().unwrap_or_else(|| vec![ex.clone()]);
                                for e in &list {
                                    excluded.extend(self.definition(e, nesting)?);
                                }
                                continue;
                            }
                            let set = self.definition(item, nesting)?;
                            acc = Some(match acc {
                                None => set,
                                Some(prev) if combine => prev.union(&set).copied().collect(),
                                Some(prev) => prev.intersection(&set).copied().collect(),
                            });
                        }
                        let mut out = acc.unwrap_or_default();
                        out.retain(|i| !excluded.contains(i));
                        return Ok(out);
                    }
                }
                let (method, value) = match (map.get("method"), map.get("value")) {
                    (Some(m), Some(v)) => (scalar(m), scalar(v)),
                    _ => match map.iter().next() {
                        // `- tag: nightly`
                        Some((m, v)) => (m.clone(), scalar(v)),
                        None => return Ok(Set::new()),
                    },
                };
                let flag = |k: &str| map.get(k).and_then(Value::as_bool).unwrap_or(false);
                let depth_of = |k: &str| {
                    map.get(k)
                        .and_then(Value::as_u64)
                        .map(|n| Depth::Levels(n as u32))
                };
                let mut text = String::new();
                if flag("childrens_parents") {
                    text.push('@');
                }
                let parents = depth_of("parents_depth").or(flag("parents").then_some(Depth::All));
                let children =
                    depth_of("children_depth").or(flag("children").then_some(Depth::All));
                text.push_str(&format!("{method}:{value}"));
                let mut criterion = super::parse::parse(&text)
                    .map_err(Nested::Inner)?
                    .groups
                    .into_iter()
                    .flatten()
                    .next()
                    .ok_or(Nested::Unknown)?;
                criterion.parents = parents;
                criterion.children = children;
                self.criterion(&criterion, nesting).map_err(Nested::Inner)
            }
            _ => Ok(Set::new()),
        }
    }
}

enum Nested {
    Unknown,
    Cycle,
    Inner(SelectionError),
}

fn scalar(v: &serde_json::Value) -> String {
    v.as_str()
        .map(str::to_string)
        .unwrap_or_else(|| v.to_string())
}

//! Project variables (03 §4): the root project's, each installed
//! package's, and overrides, by dbt's variable precedence.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use cogitator_lsp::Range;
use cogitator_schemas::project::ProjectFile;

use super::{PackageVars, ProjectDetails, VarOverride, VarSource, VariableDetails};

type Vars = HashMap<String, VariableDetails>;

/// The root project's vars and each package's, every map already in
/// precedence order (03 §4): package global < package scoped < root
/// global < root `vars.<scope>` < overrides. A top-level key naming the
/// root project or an installed package, with a mapping value, is a
/// scope block, never a var; null values in a scope block are skipped.
pub(super) fn build_variables(
    root: &Path,
    project: &ProjectFile,
    packages: &[ProjectDetails],
    overrides: &[VarOverride],
) -> (Vars, Vec<PackageVars>) {
    let scopes: HashSet<&str> = std::iter::once(project.name.as_str())
        .chain(packages.iter().map(|p| p.file.name.as_str()))
        .filter(|n| !n.is_empty())
        .collect();
    let root_file = root.join("dbt_project.yml");

    let mut own = Vars::new();
    globals(&mut own, project, &root_file, &scopes, VarSource::Project);
    scoped(
        &mut own,
        project,
        &project.name,
        &root_file,
        VarSource::Scoped(project.name.clone()),
    );
    overriding(&mut own, overrides);

    let package_vars = packages
        .iter()
        .filter(|p| !p.file.name.is_empty())
        .map(|p| {
            let name = p.file.name.clone();
            let file = p.root.join("dbt_project.yml");
            let mut vars = Vars::new();
            let pkg_scopes: HashSet<&str> = scopes.clone();
            globals(
                &mut vars,
                &p.file,
                &file,
                &pkg_scopes,
                VarSource::Package(name.clone()),
            );
            scoped(
                &mut vars,
                &p.file,
                &name,
                &file,
                VarSource::Package(name.clone()),
            );
            globals(&mut vars, project, &root_file, &scopes, VarSource::Project);
            scoped(
                &mut vars,
                project,
                &name,
                &root_file,
                VarSource::Scoped(name.clone()),
            );
            overriding(&mut vars, overrides);
            PackageVars {
                name,
                root: p.root.clone(),
                vars,
            }
        })
        .collect();
    (own, package_vars)
}

fn globals(
    out: &mut Vars,
    file: &ProjectFile,
    path: &Path,
    scopes: &HashSet<&str>,
    source: VarSource,
) {
    for (key, value) in &file.vars {
        if value.is_object() && (scopes.contains(key.as_str()) || key == &file.name) {
            continue; // a scope block
        }
        let range = file.var_ranges.get(key).copied().unwrap_or_default();
        insert(out, key, value, path, range, source.clone());
    }
}

fn scoped(out: &mut Vars, file: &ProjectFile, scope: &str, path: &Path, source: VarSource) {
    let Some(serde_json::Value::Object(block)) = file.vars.get(scope) else {
        return;
    };
    let ranges = file.nested_var_ranges.get(scope);
    for (key, value) in block {
        if value.is_null() {
            continue;
        }
        let range = ranges.and_then(|m| m.get(key)).copied().unwrap_or_default();
        insert(out, key, value, path, range, source.clone());
    }
}

fn overriding(out: &mut Vars, overrides: &[VarOverride]) {
    for o in overrides {
        insert(out, &o.name, &o.value, &o.uri, o.range, VarSource::Override);
    }
}

fn insert(
    out: &mut Vars,
    key: &str,
    value: &serde_json::Value,
    path: &Path,
    range: Range,
    source: VarSource,
) {
    out.insert(
        key.to_string(),
        VariableDetails {
            name: key.to_string(),
            value: value.clone(),
            uri: path.to_path_buf(),
            range,
            source,
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn file(name: &str, vars: serde_json::Value) -> ProjectFile {
        ProjectFile {
            name: name.into(),
            vars: vars.as_object().cloned().unwrap_or_default(),
            ..Default::default()
        }
    }

    fn value(vars: &Vars, name: &str) -> Option<(serde_json::Value, VarSource)> {
        vars.get(name).map(|v| (v.value.clone(), v.source.clone()))
    }

    #[test]
    fn precedence_follows_dbt() {
        let root = file(
            "shop",
            json!({
                "a": 1, "b": 1, "c": 1,
                "shop": {"b": 2, "n": null},
                "utils": {"c": 3, "d": 3},
            }),
        );
        let pkg = ProjectDetails {
            root: "/p/dbt_packages/utils".into(),
            file: file("utils", json!({"c": 0, "d": 0, "e": 0, "utils": {"e": 4}})),
        };
        let overrides = [VarOverride {
            name: "a".into(),
            value: json!(9),
            ..Default::default()
        }];
        let (own, packages) = build_variables(Path::new("/p"), &root, &[pkg], &overrides);

        // root scope
        assert_eq!(value(&own, "a"), Some((json!(9), VarSource::Override)));
        assert_eq!(
            value(&own, "b"),
            Some((json!(2), VarSource::Scoped("shop".into())))
        );
        assert_eq!(value(&own, "c"), Some((json!(1), VarSource::Project)));
        assert_eq!(value(&own, "n"), None, "a null in a scope block is skipped");
        assert_eq!(value(&own, "shop"), None, "a scope block is no var");
        assert_eq!(value(&own, "utils"), None, "a package's block is no var");
        assert_eq!(value(&own, "d"), None, "another scope's vars stay there");

        // package scope
        let utils = &packages[0];
        assert_eq!(utils.name, "utils");
        let v = &utils.vars;
        assert_eq!(value(v, "a"), Some((json!(9), VarSource::Override)));
        assert_eq!(
            value(v, "c"),
            Some((json!(3), VarSource::Scoped("utils".into())))
        );
        assert_eq!(
            value(v, "d"),
            Some((json!(3), VarSource::Scoped("utils".into())))
        );
        assert_eq!(
            value(v, "e"),
            Some((json!(4), VarSource::Package("utils".into())))
        );
        assert_eq!(
            value(v, "b"),
            Some((json!(1), VarSource::Project)),
            "root global, not shop's block"
        );
        assert_eq!(
            v["e"].uri,
            Path::new("/p/dbt_packages/utils/dbt_project.yml"),
            "defined in the package's own file"
        );
    }

    #[test]
    fn an_object_var_that_names_no_scope_is_a_var() {
        let root = file("shop", json!({"config": {"x": 1}}));
        let (own, _) = build_variables(Path::new("/p"), &root, &[], &[]);
        assert_eq!(
            value(&own, "config"),
            Some((json!({"x": 1}), VarSource::Project))
        );
    }
}

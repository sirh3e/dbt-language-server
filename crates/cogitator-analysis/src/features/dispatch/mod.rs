//! Macro dispatch as dbt resolves it: `adapter.dispatch('x', 'pkg')`
//! looks, package by package in the search order, for
//! `<adapter>__x`, then each parent adapter's, then `default__x`
//! (dbt-core's `BaseDatabaseWrapper.dispatch`). The search order is the
//! `dispatch:` entry of dbt_project.yml for that namespace, else the
//! root project then the namespace; without a namespace, the root project
//! then every installed package. dbt's internal macros (`dbt`) are not
//! scanned, so what only they define is not known here.

pub(crate) mod implementation;
pub(crate) mod no_default;
#[cfg(test)]
pub(crate) mod testkit;

use cogitator_parser::macro_body::DispatchCall;

use super::DbtContext;
use crate::context::MacroDetails;

/// dbt's internal namespace: always last, never scanned.
pub(crate) const DBT_NAMESPACE: &str = "dbt";

/// The packages `dispatch` looks in, in order.
pub fn search_packages(ctx: &DbtContext, namespace: Option<&str>) -> Vec<String> {
    let root = ctx.project.name.clone();
    let mut out = match namespace {
        Some(ns) => match ctx
            .project
            .dispatch
            .iter()
            .find(|d| d.macro_namespace == ns)
        {
            Some(entry) => entry.search_order.clone(),
            None => vec![root, ns.to_string()],
        },
        None => {
            let mut packages: Vec<String> = ctx
                .macro_detail_map
                .keys()
                .filter(|p| **p != root)
                .cloned()
                .collect();
            packages.sort();
            std::iter::once(root).chain(packages).collect()
        }
    };
    out.push(DBT_NAMESPACE.to_string());
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

/// The prefixes dbt tries for one adapter type: itself, the adapters it
/// builds on, then `default`. An unknown adapter tries `default` alone.
pub fn adapter_prefixes(adapter: &str) -> Vec<&str> {
    let parents: &[&str] = match adapter {
        "redshift" => &["postgres"],
        "databricks" => &["spark"],
        "materialize" => &["postgres"],
        _ => &[],
    };
    let mut out: Vec<&str> = Vec::new();
    if !adapter.is_empty() && adapter != "default" {
        out.push(adapter);
    }
    out.extend(parents);
    out.push("default");
    out
}

/// One implementation of a dispatched macro.
#[derive(Debug, Clone, Copy)]
pub struct Implementation<'a> {
    pub package: &'a str,
    /// The `<prefix>` of `<prefix>__name`.
    pub prefix: &'a str,
    pub details: &'a MacroDetails,
}

/// Every `<prefix>__name` of every scanned package, by package then name.
pub fn implementations<'a>(ctx: &'a DbtContext, name: &str) -> Vec<Implementation<'a>> {
    let suffix = format!("__{name}");
    let mut out: Vec<Implementation<'a>> = ctx
        .macro_detail_map
        .iter()
        .flat_map(|(package, macros)| {
            let suffix = suffix.clone();
            macros.iter().filter_map(move |(macro_name, details)| {
                let prefix = macro_name.strip_suffix(suffix.as_str())?;
                (!prefix.is_empty() && !prefix.contains("__")).then_some(Implementation {
                    package: package.as_str(),
                    prefix,
                    details,
                })
            })
        })
        .collect();
    out.sort_by(|a, b| (a.package, &a.details.name).cmp(&(b.package, &b.details.name)));
    out
}

/// The implementation dispatch picks for the active adapter
/// (`ctx.dialect`, from the active profiles target), if one is scanned.
pub fn active_implementation<'a>(
    ctx: &'a DbtContext,
    call: &DispatchCall,
) -> Option<Implementation<'a>> {
    let adapter = ctx.dialect.as_str();
    for package in search_packages(ctx, call.namespace.as_deref()) {
        let Some((package, macros)) = ctx.macro_detail_map.get_key_value(&package) else {
            continue;
        };
        for prefix in adapter_prefixes(adapter) {
            if let Some(details) = macros.get(&format!("{prefix}__{}", call.name)) {
                let prefix = &details.name[..prefix.len()];
                return Some(Implementation {
                    package: package.as_str(),
                    prefix,
                    details,
                });
            }
        }
    }
    None
}

/// One hover line naming what dispatch picks for the active adapter.
pub fn active_line(ctx: &DbtContext, call: &DispatchCall) -> String {
    let adapter = match ctx.dialect.as_str() {
        "" => "default",
        a => a,
    };
    match active_implementation(ctx, call) {
        Some(i) => format!(
            "Dispatches to `{}.{}` for adapter `{adapter}`.",
            i.package, i.details.name
        ),
        None => format!(
            "No scanned implementation of `{}` for adapter `{adapter}` \
             (dbt's internal macros are not scanned).",
            call.name
        ),
    }
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;

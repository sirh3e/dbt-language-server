//! What dbt declares about how each input grows, for the incrementality
//! analysis (`cogitator_rules::canon::incrementality`, specs/ir-pipeline/10):
//!
//! - a snapshot's rows change (dbt closes and reopens them);
//! - an incremental model built by `append` or `microbatch` only grows;
//!   one built by `merge`, `delete+insert` or `insert_overwrite` changes;
//! - `event_time` (manifest or YAML) and a source's `loaded_at_field`
//!   name the column that only grows.
//!
//! Inputs dbt says nothing about are left out; the analysis then reads
//! their columns, and marks what it infers.

use std::sync::Arc;

use cogitator_rules::canon::incrementality::{IncrementalInputs, InputFacts, InputGrowth};

use super::PlanInput;
use crate::context::ModelKind;

pub(super) fn incremental_inputs(input: &PlanInput<'_>) -> IncrementalInputs {
    let mut out = IncrementalInputs::default();
    for (name, details) in input.models {
        let manifest = input
            .manifest
            .and_then(|m| m.models.get(name))
            .map(|n| &n.config);
        let event_time = manifest
            .and_then(|c| c.event_time.clone())
            .or_else(|| details.event_time.clone());
        let growth = if details.kind == ModelKind::Snapshot {
            InputGrowth::Changing
        } else {
            match manifest.map(|c| (c.materialized.as_deref(), c.incremental_strategy.as_deref())) {
                Some((Some("incremental"), Some("append" | "microbatch"))) => InputGrowth::Append,
                Some((Some("incremental"), Some(_))) => InputGrowth::Changing,
                _ => InputGrowth::Unknown,
            }
        };
        if growth == InputGrowth::Unknown && event_time.is_none() {
            continue;
        }
        out.by_name.insert(
            Arc::from(name.as_str()),
            InputFacts {
                growth,
                event_time: event_time.map(Arc::from),
                inferred: false,
            },
        );
    }
    for source in input.sources.values() {
        for table in source.tables.values() {
            let Some(event_time) = table.event_time.as_deref() else {
                continue;
            };
            out.by_name.insert(
                Arc::from(format!("{}.{}", source.name, table.name)),
                InputFacts {
                    growth: InputGrowth::Unknown,
                    event_time: Some(Arc::from(event_time)),
                    inferred: false,
                },
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::tests::Facts;
    use super::*;
    use crate::artifacts::manifest::ManifestConfig;

    fn manifest(
        materialized: &str,
        strategy: Option<&str>,
        event_time: Option<&str>,
    ) -> ManifestConfig {
        ManifestConfig {
            materialized: Some(materialized.into()),
            incremental_strategy: strategy.map(str::to_string),
            event_time: event_time.map(str::to_string),
            ..ManifestConfig::default()
        }
    }

    #[test]
    fn what_dbt_declares_becomes_input_facts() {
        let facts = Facts::new("duckdb")
            .model("snap", ModelKind::Snapshot, None)
            .model("events", ModelKind::Model, None)
            .manifest(
                "events",
                manifest("incremental", Some("append"), Some("ts")),
            )
            .model("orders", ModelKind::Model, None)
            .manifest("orders", manifest("incremental", Some("merge"), None))
            .model("clicks", ModelKind::Model, Some("clicked_at"))
            .model("users", ModelKind::Model, None)
            .source("raw", "payments", Some("_loaded_at"))
            .source("raw", "plans", None);
        let inputs = incremental_inputs(&facts.input("select 1", Some("m")));
        let fact = |name: &str| inputs.by_name.get(name).cloned();
        let growth = |name: &str| fact(name).map(|f| f.growth);
        let column = |name: &str| fact(name).and_then(|f| f.event_time).map(|c| c.to_string());

        assert_eq!(growth("snap"), Some(InputGrowth::Changing));
        assert_eq!(growth("events"), Some(InputGrowth::Append));
        assert_eq!(column("events").as_deref(), Some("ts"));
        assert_eq!(growth("orders"), Some(InputGrowth::Changing));
        assert_eq!(growth("clicks"), Some(InputGrowth::Unknown));
        assert_eq!(column("clicks").as_deref(), Some("clicked_at"));
        assert_eq!(column("raw.payments").as_deref(), Some("_loaded_at"));
        // dbt says nothing of these: the analysis reads their columns
        assert_eq!(fact("users"), None);
        assert_eq!(fact("raw.plans"), None);
    }
}

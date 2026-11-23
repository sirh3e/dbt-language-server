//! The temp-dir dbt projects the planner's tests run on: the context,
//! the schema job and typed lineage as the shell gathers them.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::*;
use crate::artifacts::ArtifactSet;
use crate::features::column_types;
use crate::features::plan::schemas::{
    run_schema_job, JobInput, JobLimits, JobState, SavedFiles, SchemaSnapshot,
};
use crate::features::plan::PlanInput;
use crate::{build_context, lineage};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A dbt project in a temp directory, removed on drop.
pub(super) struct Project {
    pub root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct Disk;

impl SavedFiles for Disk {
    fn stamp(&self, path: &Path) -> Option<u64> {
        path.is_file().then_some(1)
    }
    fn read(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }
}

impl Project {
    pub fn new(files: &[(&str, &str)]) -> Self {
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("cogitator_sync_{}_{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let project = Project { root };
        project.write(
            "dbt_project.yml",
            "name: p\nversion: '1.0'\nprofile: p\nmodel-paths: ['models']\n",
        );
        for (path, text) in files {
            project.write(path, text);
        }
        project
    }

    pub fn write(&self, rel: &str, text: &str) {
        let path = self.root.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        std::fs::write(path, text).expect("write");
    }

    pub fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.root.join(rel)).unwrap_or_default()
    }

    /// The plan over the project as it is on disk now.
    pub fn plan(&self, options: &SyncOptions) -> SyncPlan {
        let ctx = build_context(self.root.to_str().expect("utf-8"));
        let never = || false;
        let snapshot: Arc<SchemaSnapshot> = run_schema_job(
            &JobInput {
                dialect: &ctx.dialect,
                models: &ctx.model_detail_map,
                sources: &ctx.source_detail_map,
                graph: &ctx.graph,
                manifest: None,
                catalog: None,
                files: &Disk,
                limits: JobLimits::default(),
                stale: &never,
            },
            &mut JobState::default(),
        )
        .expect("schema job");
        let artifacts = ArtifactSet::default();
        let lineage = |model: &str| {
            let details = ctx.model_detail_map.get(model)?;
            let text = std::fs::read_to_string(&details.uri).ok()?;
            let doc = cogitator_parser::pipeline::parse_document(&text, &ctx.dialect).ok()?;
            let input =
                PlanInput::from_context(&ctx, &text, Some(model), &artifacts, snapshot.revision)
                    .with_project(Some(&snapshot));
            lineage::typed_column_lineage(&doc, &input)
        };
        let types = |model: &str| column_types::column_types(&ctx, &snapshot, model);
        let read = |path: &Path| std::fs::read_to_string(path).ok();
        plan_sync(
            &SyncInputs {
                ctx: &ctx,
                lineage: &lineage,
                types: &types,
                read: &read,
            },
            options,
        )
    }

    /// Plan and write the result, as `cogitator yaml-sync` does.
    pub fn sync(&self, options: &SyncOptions) -> SyncPlan {
        let plan = self.plan(options);
        for file in &plan.files {
            std::fs::write(&file.path, &file.new_text).expect("write");
        }
        plan
    }
}

pub(super) const SOURCES: &str = "\
version: 2
sources:
  - name: raw
    tables:
      - name: orders
        columns:
          - name: id
            description: The order key.
          - name: amount
            description: Order amount in cents.
          - name: customer
            description: Who ordered.
";

pub(super) fn base() -> Vec<(&'static str, &'static str)> {
    vec![
        ("models/staging/sources.yml", SOURCES),
        (
            "models/staging/stg_orders.sql",
            "select id, amount, status, customer from {{ source('raw', 'orders') }}\n",
        ),
        (
            "models/staging/stg_orders.yml",
            "version: 2\n\nmodels:\n  - name: stg_orders\n    columns:\n      - name: id\n      - name: status\n        description: Where the order stands.\n",
        ),
        (
            "models/marts/orders.sql",
            "with o as (select * from {{ ref('stg_orders') }}),\n\
             final as (select id, amount + 1 as amount, status as state, customer from o)\n\
             select * from final\n",
        ),
    ]
}

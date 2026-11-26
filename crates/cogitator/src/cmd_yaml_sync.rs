//! `cogitator yaml-sync`: the schema-YAML sync from the command line —
//! the same planner as `dbt.syncYaml` (`features::yaml_sync`), over the
//! project on disk. Writes the files, or with `--check` only says what
//! would change and exits 1 when anything would (for CI).

use std::path::PathBuf;

use cogitator_analysis::features::yaml_sync::{ModelSummary, NewEntries, SyncOptions, SyncPlan};
use cogitator_analysis::lineage::SourceRef;

use crate::server::yaml_sync::{offline_plan, summary_line, Offline};

/// Bring the models' schema YAML in line with their SQL: add missing
/// entries and columns, inherit descriptions down the column lineage.
#[derive(clap::Args, Debug)]
pub struct Args {
    /// The dbt project (the directory holding dbt_project.yml).
    #[arg(long = "project-dir", default_value = ".")]
    project_dir: PathBuf,
    /// Write nothing; exit 1 when the YAML is not in sync.
    #[arg(long)]
    check: bool,
    /// Remove documented columns the models no longer produce.
    #[arg(long)]
    prune: bool,
    /// Fill missing data_type from proven inferred types.
    #[arg(long = "inherit-types")]
    inherit_types: bool,
    /// Order column entries as the SQL outputs them.
    #[arg(long)]
    reorder: bool,
    /// Put new entries in one `_<folder>__models.yml` per folder instead of
    /// `<model>.yml` beside each model.
    #[arg(long = "per-folder")]
    per_folder: bool,
    /// Print the plan as JSON (the dbt.syncYaml result) instead of the report.
    #[arg(long)]
    json: bool,
    /// Only these models (default: every model of the project).
    models: Vec<String>,
}

/// Runs the command; the process exit code.
pub fn run(args: &Args, profiles_dir: Option<&str>, target: Option<&str>) -> i32 {
    let options = SyncOptions {
        models: args.models.clone(),
        prune: args.prune,
        inherit_types: args.inherit_types,
        reorder: args.reorder,
        new_entries: if args.per_folder {
            NewEntries::PerFolder
        } else {
            NewEntries::PerModel
        },
    };
    let root =
        std::fs::canonicalize(&args.project_dir).unwrap_or_else(|_| args.project_dir.clone());
    let offline = Offline {
        profiles_dir: profiles_dir.map(str::to_string),
        target: target.map(str::to_string),
    };
    let plan = match offline_plan(&root, &offline, &options) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("cogitator yaml-sync: {}", error.0);
            return 2;
        }
    };
    if args.json {
        match serde_json::to_string_pretty(&plan) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("cogitator yaml-sync: cannot print the plan: {error}");
                return 2;
            }
        }
    } else {
        print!("{}", report(&plan, args.check));
    }
    if args.check {
        return i32::from(plan.changes > 0);
    }
    for file in &plan.files {
        if file.created && file.path.exists() {
            eprintln!(
                "cogitator yaml-sync: {} appeared while planning.\nSolution: run yaml-sync again.",
                file.path.display()
            );
            return 2;
        }
        if let Some(parent) = file.path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "cogitator yaml-sync: cannot create {}: {error}",
                    parent.display()
                );
                return 2;
            }
        }
        if let Err(error) = std::fs::write(&file.path, &file.new_text) {
            eprintln!(
                "cogitator yaml-sync: cannot write {}: {error}\nSolution: check the file's permissions.",
                file.path.display()
            );
            return 2;
        }
    }
    if !args.json {
        println!("{}", summary_line(&plan, true));
    }
    0
}

/// What the plan holds, a block per model that changes or was skipped.
fn report(plan: &SyncPlan, check: bool) -> String {
    let mut out = String::new();
    for model in &plan.models {
        let lines = model_lines(model);
        if lines.is_empty() {
            continue;
        }
        out.push_str(&format!("{}\n", model.model));
        for line in lines {
            out.push_str(&format!("  {line}\n"));
        }
    }
    if check {
        out.push_str(&summary_line(plan, false));
        out.push('\n');
    }
    out
}

fn model_lines(model: &ModelSummary) -> Vec<String> {
    let mut lines = Vec::new();
    if model.created_entry {
        lines.push("+ entry".to_string());
    }
    for column in &model.added {
        lines.push(format!("+ column {column}"));
    }
    for inherited in &model.inherited {
        let from = match &inherited.from {
            source @ SourceRef::Source { .. } => format!("source {}", source.label()),
            other => other.label(),
        };
        lines.push(format!("~ {} description from {from}", inherited.column));
    }
    for typed in &model.typed {
        lines.push(format!("~ {} data_type {}", typed.column, typed.data_type));
    }
    for column in &model.pruned {
        lines.push(format!("- column {column}"));
    }
    if model.reordered {
        lines.push("~ columns reordered".to_string());
    }
    for column in model.stale.iter().filter(|c| !model.pruned.contains(c)) {
        lines.push(format!("! stale column {column} (--prune removes it)"));
    }
    // one line per reason, naming the columns it holds for
    let mut reasons: Vec<(&str, Vec<&str>)> = Vec::new();
    for skip in &model.skipped {
        let Some(column) = skip.column.as_deref() else {
            lines.push(format!("? skipped: {}", skip.reason));
            continue;
        };
        match reasons.iter_mut().find(|(r, _)| *r == skip.reason) {
            Some((_, columns)) => columns.push(column),
            None => reasons.push((&skip.reason, vec![column])),
        }
    }
    for (reason, columns) in reasons {
        lines.push(format!("? {}: {reason}", columns.join(", ")));
    }
    lines
}

//! `cogitator lineage-site`: the column-lineage dashboard of a dbt project
//! as one self-contained `index.html`, from static analysis alone (no dbt
//! run, no warehouse). The analysis is the server's (`dbt.lineageSite`,
//! `server::handlers::lineage_site`).

use std::path::PathBuf;
use std::time::Instant;

use clap::Args;

use crate::server::handlers::lineage_site::{headless, write_site, SITE_DIR};

/// The default limits of the server (cogitator.yml `limits:`).
const ARTIFACT_CAP: u64 = 64 * 1024 * 1024;
const COMPILED_CAP: u64 = 2 * 1024 * 1024;

#[derive(Args)]
pub struct LineageSiteArgs {
    /// The dbt project (the directory with dbt_project.yml)
    #[arg(long = "project-dir", default_value = ".")]
    project_dir: PathBuf,
    /// Where to write index.html and lineage.json
    /// [default: <project-dir>/target/cogitator-lineage]
    #[arg(long)]
    out: Option<PathBuf>,
    /// Open the page in the default browser when it is written
    #[arg(long)]
    open: bool,
}

/// Exit status: 0 written, 1 not.
pub fn run(args: &LineageSiteArgs) -> i32 {
    let started = Instant::now();
    let root =
        cogitator_analysis::util::canonical(&args.project_dir).unwrap_or(args.project_dir.clone());
    let site = match headless(&root, ARTIFACT_CAP, COMPILED_CAP) {
        Ok(site) => site,
        Err(guidance) => {
            eprintln!("{}", guidance.to_message());
            return 1;
        }
    };
    let out = args.out.clone().unwrap_or_else(|| root.join(SITE_DIR));
    let page = match write_site(&site, &out) {
        Ok(page) => page,
        Err(guidance) => {
            eprintln!("{}", guidance.to_message());
            return 1;
        }
    };
    let c = &site.counts;
    eprintln!(
        "cogitator: wrote {} — {} nodes, {} edges; {} of {} columns traced, {} undetermined; \
         {} models traced, {} declined ({} ms)",
        page.display(),
        c.nodes,
        c.edges,
        c.columns_traced,
        c.columns,
        c.columns_undetermined,
        c.models_traced,
        c.models_declined,
        started.elapsed().as_millis()
    );
    println!("{}", page.display());
    if args.open {
        if let Err(err) = open_in_browser(&page) {
            eprintln!(
                "{}",
                cogitator_error::guidance::Guidance::new("The browser did not open.")
                    .at(page.display().to_string())
                    .because(err.to_string())
                    .fix("open the file yourself; it needs no server.")
                    .to_message()
            );
        }
    }
    0
}

fn open_in_browser(page: &std::path::Path) -> std::io::Result<()> {
    let mut command = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else {
        std::process::Command::new("xdg-open")
    };
    command
        .arg(page)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(|_| ())
}

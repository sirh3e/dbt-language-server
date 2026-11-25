//! The shell of the lineage site, shared by `dbt.lineageSite` and the
//! `cogitator lineage-site` subcommand: it reads the model files, the
//! compiled SQL of templates with control flow and the artifacts, and
//! hands the pure export values (`features::lineage_site`).

use std::path::{Path, PathBuf};

use cogitator_analysis::artifacts::ArtifactSet;
use cogitator_analysis::features::lineage_site::{
    self, page_html, trace_model, LineageSite, ModelText, SiteInput, Traced,
};
use cogitator_analysis::features::plan::schemas::{
    run_schema_job, JobInput, JobLimits, JobState, SchemaSnapshot,
};
use cogitator_analysis::features::plan::CompiledInput;
use cogitator_analysis::DbtContext;
use cogitator_error::guidance::Guidance;
use serde::Serialize;

use super::super::derived_cache::{DerivedCache, Disk};
use super::super::reply::Reply;
use super::plan_compiled::CompiledProbe;

/// Where the editors' copy of the site goes, under the project root.
pub const SITE_DIR: &str = "target/cogitator-lineage";

/// The site of `ctx`, every model traced from its saved file.
pub fn site(
    ctx: &DbtContext,
    artifacts: &ArtifactSet,
    project: Option<&SchemaSnapshot>,
    compiled_cap: u64,
) -> LineageSite {
    let read = |p: &Path| std::fs::read_to_string(p).ok();
    let universe = cogitator_analysis::selectors::universe(ctx, &read);
    let traced = |model: &str| -> Traced {
        let Some(details) = ctx.model_detail_map.get(model) else {
            return Traced::Unreadable;
        };
        let text = std::fs::read_to_string(&details.uri).ok();
        let uri = cogitator_lsp::fs_path_to_uri(&details.uri);
        // Only a template with Jinja statements can need the compiled file.
        let probe = text
            .as_deref()
            .filter(|t| t.contains("{%"))
            .map(|_| CompiledProbe::new(ctx, &uri).read(compiled_cap));
        trace_model(
            ctx,
            artifacts,
            project,
            ModelText {
                model,
                text: text.as_deref(),
                compiled: probe
                    .as_ref()
                    .map_or(CompiledInput::NotChecked, |file| file.input()),
            },
        )
    };
    let generator = format!("cogitator {}", crate::VERSION);
    lineage_site::export(&SiteInput {
        ctx,
        universe: &universe,
        schemas: project,
        traced: &traced,
        generator: &generator,
    })
}

/// The project at `root`, analysed as the server would: context, the
/// artifacts under `target/`, and one schema job run to completion.
pub fn headless(
    root: &Path,
    artifact_cap: u64,
    compiled_cap: u64,
) -> Result<LineageSite, Guidance> {
    if !root.join("dbt_project.yml").is_file() {
        return Err(Guidance::new("No dbt project here.")
            .at(root.display().to_string())
            .because("the directory has no dbt_project.yml")
            .fix("pass --project-dir pointing at the directory that holds dbt_project.yml."));
    }
    let Some(root_text) = root.to_str() else {
        return Err(Guidance::new("The project path is not UTF-8.")
            .at(root.display().to_string())
            .fix("move the project to a UTF-8 path."));
    };
    let ctx = cogitator_analysis::build_context(root_text);
    let artifacts = DerivedCache::default().artifacts(root, artifact_cap);
    let never = || false;
    let project = run_schema_job(
        &JobInput {
            dialect: &ctx.dialect,
            models: &ctx.model_detail_map,
            sources: &ctx.source_detail_map,
            graph: &ctx.graph,
            manifest: artifacts.manifest.as_deref(),
            catalog: artifacts.catalog.as_deref(),
            files: &Disk,
            limits: JobLimits::default(),
            stale: &never,
        },
        &mut JobState::default(),
    );
    Ok(site(&ctx, &artifacts, project.as_deref(), compiled_cap))
}

/// Writes `index.html` (and `lineage.json`) into `dir`, made if missing;
/// returns the page's path.
pub fn write_site(site: &LineageSite, dir: &Path) -> Result<PathBuf, Guidance> {
    let unwritable = |err: std::io::Error| {
        Guidance::new("The lineage site could not be written.")
            .at(dir.display().to_string())
            .because(err.to_string())
            .fix("pass an --out directory you can write to.")
    };
    std::fs::create_dir_all(dir).map_err(unwritable)?;
    let page = dir.join("index.html");
    std::fs::write(&page, page_html(site)).map_err(unwritable)?;
    let json = serde_json::to_string_pretty(site).unwrap_or_default();
    std::fs::write(dir.join("lineage.json"), json).map_err(unwritable)?;
    Ok(page)
}

#[derive(Serialize)]
struct Assets {
    script: String,
    style: &'static str,
}

#[derive(Serialize)]
struct Written {
    path: String,
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Answer<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    site: Option<&'a LineageSite>,
    #[serde(skip_serializing_if = "Option::is_none")]
    assets: Option<Assets>,
    #[serde(skip_serializing_if = "Option::is_none")]
    written: Option<Written>,
    counts: &'a lineage_site::Counts,
}

/// What `dbt.lineageSite` asks for besides the data.
pub struct Request {
    /// Answer the page's script and stylesheet too (the VS Code webview).
    pub assets: bool,
    /// Write the page under `<root>/target/cogitator-lineage/` and answer
    /// its path instead of the data (Neovim, Emacs).
    pub write: bool,
}

/// `dbt.lineageSite`. Offloaded.
pub fn command(
    ctx: &DbtContext,
    derived: &DerivedCache,
    artifact_cap: u64,
    compiled_cap: u64,
    request: Request,
) -> Reply {
    let Some(root) = ctx.project_root.as_deref() else {
        return Reply::null();
    };
    let artifacts = derived.artifacts(root, artifact_cap);
    let project = derived.project_schemas(root);
    let site = site(ctx, &artifacts, project.as_deref(), compiled_cap);
    let written = if request.write {
        match write_site(&site, &root.join(SITE_DIR)) {
            Ok(page) => Some(Written {
                uri: cogitator_lsp::fs_path_to_uri(&page),
                path: page.display().to_string(),
            }),
            Err(guidance) => {
                return Reply::error(
                    cogitator_lsp::error_codes::INTERNAL_ERROR,
                    guidance.to_message(),
                )
            }
        }
    } else {
        None
    };
    Reply::result(&Answer {
        site: (!request.write).then_some(&site),
        assets: request.assets.then(|| Assets {
            script: lineage_site::page_script(),
            style: lineage_site::page_style(),
        }),
        written,
        counts: &site.counts,
    })
}

//! What the project-wide subcommands (`cogitator lint`, `cogitator
//! format`) share: finding the dbt project, its cogitator.yml, the
//! context, the files a run covers, and how a path is shown. Results go
//! to stdout, problems to stderr; exit code 2 is a usage or project error.

use std::path::{Path, PathBuf};

use cogitator_analysis::context::ContextBuilder;
use cogitator_analysis::DbtContext;
use cogitator_error::guidance::Guidance;

use crate::server::config_file::{self, FileConfig};

/// Exit code of a usage or project error.
pub const USAGE: i32 = 2;

/// A loaded project: its root, cogitator.yml and context.
pub struct Project {
    pub root: PathBuf,
    pub config: FileConfig,
    pub ctx: DbtContext,
}

/// The dbt project at or above `dir`, its cogitator.yml (when present)
/// and the context built the way the server builds it. `profiles_dir`
/// and `target` are the global `--profiles-dir` / `--target` flags.
pub fn load_project(
    dir: &Path,
    profiles_dir: Option<&str>,
    target: Option<&str>,
) -> Result<Project, Guidance> {
    let start = dir.canonicalize().map_err(|err| {
        Guidance::new("the project directory does not exist.")
            .at(dir.display().to_string())
            .because(err.to_string())
            .fix("pass --project-dir with the directory that holds dbt_project.yml.")
    })?;
    let root =
        cogitator_analysis::util::get_project_root("dbt_project.yml", &start).ok_or_else(|| {
            Guidance::new("no dbt project found.")
                .at(start.display().to_string())
                .because("neither this directory nor a parent holds dbt_project.yml.")
                .fix("run inside a dbt project, or pass --project-dir.")
        })?;
    let config = read_config(&root)?;
    let mut builder = ContextBuilder::default();
    let dir = profiles_dir
        .map(str::to_string)
        .or_else(|| config.profiles.dir.clone());
    if let Some(dir) = dir {
        builder.set_profiles_dir(dir);
    }
    if let Some(profile) = config.profiles.profile.clone() {
        builder.set_profile_override(profile);
    }
    let target = target
        .map(str::to_string)
        .or_else(|| config.profiles.target.clone());
    if let Some(target) = target {
        builder.set_target_override(target);
    }
    builder.set_var_overrides(config.var_overrides.clone());
    let ctx = builder.build(&root.display().to_string());
    if ctx.project_root.is_none() {
        return Err(Guidance::new("the dbt project could not be read.")
            .at(root.display().to_string())
            .because("dbt_project.yml was found but no project context was built.")
            .fix("check that dbt_project.yml is valid YAML with a `name:`."));
    }
    Ok(Project { root, config, ctx })
}

/// `<root>/cogitator.yml` (or `.cogitator.yml`); absent means defaults.
fn read_config(root: &Path) -> Result<FileConfig, Guidance> {
    for name in ["cogitator.yml", ".cogitator.yml"] {
        let path = root.join(name);
        if let Ok(text) = std::fs::read_to_string(&path) {
            return config_file::parse(&text, &path);
        }
    }
    Ok(FileConfig::default())
}

/// `candidates` narrowed to the given paths (files, or directories that
/// contain them); explicit files outside the candidates are kept when
/// `keep` accepts them. No paths: every candidate.
pub fn select(
    candidates: Vec<PathBuf>,
    paths: &[PathBuf],
    keep: impl Fn(&Path) -> bool,
) -> Result<Vec<PathBuf>, Guidance> {
    if paths.is_empty() {
        return Ok(candidates);
    }
    let mut wanted = Vec::new();
    for path in paths {
        let full = path.canonicalize().map_err(|err| {
            Guidance::new("a path does not exist.")
                .at(path.display().to_string())
                .because(err.to_string())
                .fix("pass files or directories inside the dbt project.")
        })?;
        wanted.push(full);
    }
    let mut out: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|c| wanted.iter().any(|w| c.starts_with(w)))
        .collect();
    for w in wanted.iter().filter(|w| w.is_file() && keep(w)) {
        if !out.contains(w) {
            out.push(w.clone());
        }
    }
    out.sort();
    Ok(out)
}

/// A path as the user reads it: relative to the working directory
/// when below it, absolute otherwise.
pub fn shown(path: &Path) -> String {
    let cwd = std::env::current_dir()
        .ok()
        .and_then(|d| d.canonicalize().ok());
    cwd.and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}

/// Print a Guidance to stderr and hand back the usage exit code.
pub fn fail(guidance: &Guidance) -> i32 {
    eprintln!("cogitator: {}", guidance.to_message());
    USAGE
}

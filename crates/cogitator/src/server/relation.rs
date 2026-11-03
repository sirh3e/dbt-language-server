//! dbt.relation shell: resolve the active profiles target's relation
//! coordinates from the inventory (same resolution walk as the auth
//! warning), leaving the actual relation assembly to the pure feature.

use cogitator_analysis::features::TargetRelation;

use super::derived_cache::DerivedCache;
use super::ProfilesConfig;

/// "Switch target" code-action choices when `uri` IS the active
/// profiles.yml: every allowed profile×target, the active pair marked.
/// `None` for any other uri — the pure feature then skips the branch.
pub fn profiles_target_choices(
    cfg: &ProfilesConfig,
    root: &str,
    project_profile: &str,
    uri: &str,
    derived: &DerivedCache,
) -> Option<Vec<cogitator_analysis::features::TargetChoice>> {
    let path = cogitator_lsp::uri_to_fs_path(uri)?;
    let active_file = cogitator_analysis::util::profiles_candidates(&cfg.dir_override, root)
        .into_iter()
        .find(|p| p.is_file())?;
    // Path equality compares components, so `/` and `\` separators match
    if active_file.as_path() != std::path::Path::new(path.as_str()) {
        return None;
    }
    let inventory = derived.profiles_inventory(&active_file, project_profile, &cfg.allow);
    let active_profile = if cfg.profile.is_empty() {
        project_profile.to_string()
    } else {
        cfg.profile.clone()
    };
    let active_target = if cfg.target.is_empty() {
        inventory
            .profile(&active_profile)
            .map(|p| p.default_target.clone())
            .unwrap_or_default()
    } else {
        cfg.target.clone()
    };
    let choices: Vec<_> = inventory
        .profiles
        .iter()
        .filter(|p| p.allowed)
        .flat_map(|p| {
            p.targets
                .iter()
                .map(|t| cogitator_analysis::features::TargetChoice {
                    profile: p.name.clone(),
                    target: t.name.clone(),
                    active: p.name == active_profile && t.name == active_target,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    Some(choices)
}

/// The active target's database/schema, or `None` when no profiles.yml,
/// profile or target resolves — dbt.relation then answers with nulls.
pub fn active_target_relation(
    cfg: &ProfilesConfig,
    root: &str,
    project_profile: &str,
    derived: &DerivedCache,
) -> Option<TargetRelation> {
    if project_profile.is_empty() && cfg.profile.is_empty() {
        return None;
    }
    let path = cogitator_analysis::util::profiles_candidates(&cfg.dir_override, root)
        .into_iter()
        .find(|p| p.is_file())?;
    let inventory = derived.profiles_inventory(&path, project_profile, &cfg.allow);
    let profile_name = if cfg.profile.is_empty() {
        project_profile
    } else {
        cfg.profile.as_str()
    };
    let profile = inventory.profile(profile_name)?;
    let target_name = if cfg.target.is_empty() {
        profile.default_target.as_str()
    } else {
        cfg.target.as_str()
    };
    let target = profile.targets.iter().find(|t| t.name == target_name)?;
    Some(TargetRelation {
        database: target.database.clone(),
        schema: target.schema.clone(),
    })
}

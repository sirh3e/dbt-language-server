//! dbt.relation shell: resolve the active profiles target's relation
//! coordinates from the inventory (same resolution walk as the auth
//! warning), leaving the actual relation assembly to the pure feature.

use cogitator_analysis::features::TargetRelation;
use cogitator_analysis::profiles::{load_inventory, ProfileScope};

use super::ProfilesConfig;

/// The active target's database/schema, or `None` when no profiles.yml,
/// profile or target resolves — dbt.relation then answers with nulls.
pub fn active_target_relation(
    cfg: &ProfilesConfig,
    root: &str,
    project_profile: &str,
) -> Option<TargetRelation> {
    if project_profile.is_empty() && cfg.profile.is_empty() {
        return None;
    }
    let path = cogitator_analysis::util::profiles_candidates(&cfg.dir_override, root)
        .into_iter()
        .find(|p| p.is_file())?;
    let scope = ProfileScope::Allowed {
        project: project_profile,
        extra: &cfg.allow,
    };
    let inventory = load_inventory(&path, &scope);
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

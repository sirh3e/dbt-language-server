//! Profiles-flavored handlers: dbt.profiles / dbt.setTarget /
//! dbt.setProfilesDir / dbt.relation — pure over the snapshot plus the
//! derived cache. State writes are returned as VERDICT values (`apply`)
//! for the shell: effects are data, mutations stay in the shell. The
//! formerly 3×-duplicated active-profile/target resolution and the
//! profiles-file lookup live here once.

use std::path::PathBuf;

use cogitator_analysis::features;
use cogitator_analysis::profiles::ProfilesInventory;
use cogitator_lsp::{message_type, Position};

use super::super::derived_cache::DerivedCache;
use super::super::reply::{Effect, Reply};
use super::super::snapshot::Snapshot;
use super::super::status::{SetProfilesDirResult, SetTargetResult};
use super::super::ProfilesConfig;
use super::navigation::short_uri;

/// The first existing profiles.yml candidate (duplicated three times
/// before this module).
pub fn find_profiles_file(cfg: &ProfilesConfig, root: &str) -> Option<PathBuf> {
    cogitator_analysis::util::profiles_candidates(&cfg.dir_override, root)
        .into_iter()
        .find(|p| p.is_file())
}

/// The active (profile, target) pair under today's overrides.
pub fn active_profile_target(
    cfg: &ProfilesConfig,
    project_profile: &str,
    inventory: &ProfilesInventory,
) -> (String, String) {
    let profile = if cfg.profile.is_empty() {
        project_profile.to_string()
    } else {
        cfg.profile.clone()
    };
    let target = if cfg.target.is_empty() {
        inventory
            .profile(&profile)
            .map(|p| p.default_target.clone())
            .unwrap_or_default()
    } else {
        cfg.target.clone()
    };
    (profile, target)
}

fn error(text: impl Into<String>) -> Effect {
    Effect::ShowMessage {
        level: message_type::ERROR,
        text: text.into(),
    }
}

/// `dbt.profiles`: the redacted inventory report.
#[tracing::instrument(skip_all)]
pub fn report(snap: &Snapshot, derived: &DerivedCache) -> Reply {
    let candidates =
        cogitator_analysis::util::profiles_candidates(&snap.profiles.dir_override, &snap.root_path);
    let searched: Vec<String> = candidates.iter().map(|p| p.display().to_string()).collect();
    let Some(path) = candidates.into_iter().find(|p| p.is_file()) else {
        return Reply::result(&cogitator_analysis::profiles::ProfilesReport::not_found(
            searched.clone(),
        ))
        .with(Effect::ShowMessage {
            level: message_type::WARNING,
            text: format!(
                "No profiles.yml found.\n\
                 Searched: {}.\n\
                 Solution: create one, or point --profiles-dir, \
                 initializationOptions.profilesDir or DBT_PROFILES_DIR at it.",
                searched.join(", ")
            ),
        });
    };
    let inventory =
        derived.profiles_inventory(&path, &snap.ctx.project.profile, &snap.profiles.allow);
    let (active_profile, active_target) =
        active_profile_target(&snap.profiles, &snap.ctx.project.profile, &inventory);
    let mut report = inventory.report();
    report.searched = searched;
    report.active_profile = Some(active_profile);
    report.active_target = Some(active_target);
    Reply::result(&report)
}

/// What the shell must write into the live ProfilesConfig on success.
pub struct SetTargetOutcome {
    pub reply: Reply,
    /// `(profile_override_or_None, target)` — `None` keeps the reply only.
    pub apply: Option<(Option<String>, String)>,
}

/// `dbt.setTarget`: validate against the inventory; the WRITE is the
/// shell's (returned as the verdict).
#[tracing::instrument(skip_all, fields(target = %target))]
pub fn set_target(
    snap: &Snapshot,
    derived: &DerivedCache,
    target: String,
    profile: Option<String>,
) -> SetTargetOutcome {
    let fail = |reply: Reply| SetTargetOutcome { reply, apply: None };
    if target.is_empty() {
        return fail(Reply::null().with(error(
            "dbt.setTarget needs a target name.\n\
             Solution: pass arguments: [{ \"target\": \"dev\" }], optionally \
             with \"profile\".",
        )));
    }
    let Some(path) = find_profiles_file(&snap.profiles, &snap.root_path) else {
        return fail(Reply::null().with(error(
            "Cannot switch target: no profiles.yml found.\n\
             Solution: create one, or point --profiles-dir / \
             initializationOptions.profilesDir / DBT_PROFILES_DIR at it.",
        )));
    };
    let inventory =
        derived.profiles_inventory(&path, &snap.ctx.project.profile, &snap.profiles.allow);
    let profile_given = profile.as_deref().is_some_and(|p| !p.is_empty());
    let profile_name = profile.filter(|p| !p.is_empty()).unwrap_or_else(|| {
        if snap.profiles.profile.is_empty() {
            snap.ctx.project.profile.clone()
        } else {
            snap.profiles.profile.clone()
        }
    });
    let Some(info) = inventory.profile(&profile_name) else {
        let guidance = cogitator_error::guidance::Guidance::new(format!(
            "Profile '{profile_name}' not found."
        ))
        .at(inventory.path.display().to_string())
        .because(format!(
            "the file defines: {}.",
            inventory
                .profiles
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .suggest(cogitator_error::guidance::closest_matches(
            &profile_name,
            inventory.profiles.iter().map(|p| p.name.as_str()),
        ))
        .fix("pass one of those as {\"profile\": …}, or add the profile to profiles.yml.");
        return fail(Reply::null().with(error(guidance.to_message())));
    };
    if !info.allowed {
        return fail(Reply::null().with(error(format!(
            "Profile '{profile_name}' is not readable.\n\
             Reason: by default only the project's own profile is; \
             '{profile_name}' is not on the allowlist.\n\
             Solution: add it via --profiles-allow {profile_name} or \
             initializationOptions.profilesAllow."
        ))));
    }
    let Some(chosen) = info.targets.iter().find(|t| t.name == target) else {
        let available: Vec<&str> = info.targets.iter().map(|t| t.name.as_str()).collect();
        let guidance = cogitator_error::guidance::Guidance::new(format!(
            "Target '{target}' not found in profile '{profile_name}'."
        ))
        .at(inventory.path.display().to_string())
        .because(format!("available targets: {}.", available.join(", ")))
        .suggest(cogitator_error::guidance::closest_matches(
            &target,
            available.iter().copied(),
        ))
        .fix("run dbt.setTarget with one of the available targets.");
        return fail(Reply::null().with(error(guidance.to_message())));
    };
    let mut message = format!(
        "Active target: {profile_name}/{target} ({}, auth: {}).",
        chosen.adapter_type,
        chosen.auth.method.as_str()
    );
    let level = if chosen.auth.issues.is_empty() {
        message_type::INFO
    } else {
        message.push_str("\nAuth problems:");
        for issue in &chosen.auth.issues {
            message.push_str("\n• ");
            message.push_str(&issue.message);
        }
        message_type::WARNING
    };
    let reply = Reply::result(&SetTargetResult {
        profile: profile_name.clone(),
        target: target.clone(),
    })
    .with(Effect::Refresh)
    .with(Effect::ShowMessage {
        level,
        text: message,
    });
    SetTargetOutcome {
        reply,
        apply: Some((profile_given.then_some(profile_name), target)),
    }
}

/// What the shell writes on a profiles-dir switch.
pub struct SetProfilesDirOutcome {
    pub reply: Reply,
    pub apply: Option<String>,
}

/// `dbt.setProfilesDir`: live directory switch; overrides reset.
#[tracing::instrument(skip_all)]
pub fn set_profiles_dir(dir: String) -> SetProfilesDirOutcome {
    if dir.is_empty() {
        return SetProfilesDirOutcome {
            reply: Reply::null().with(error(
                "No profiles directory given.\n\
                 Solution: call dbt.setProfilesDir with {\"dir\": \"/path/to/directory\"}.",
            )),
            apply: None,
        };
    }
    let reply = Reply::result(&SetProfilesDirResult { dir: dir.clone() })
        .with(Effect::Refresh)
        .with(Effect::ShowMessage {
            level: message_type::INFO,
            text: format!("Profiles directory: {dir} (profile/target overrides reset)."),
        });
    SetProfilesDirOutcome {
        reply,
        apply: Some(dir),
    }
}

/// `dbt.relation`: the warehouse coordinates under the cursor.
#[tracing::instrument(skip_all, fields(uri = %short_uri(uri)))]
pub fn relation(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
    pos: Option<Position>,
) -> Reply {
    let target = active_target_relation(snap, derived);
    let result = features::relation_info(&snap.ctx, snap.doc.as_deref(), uri, pos, target.as_ref());
    Reply::result(&result)
}

/// The active target's database/schema for dbt.relation.
fn active_target_relation(
    snap: &Snapshot,
    derived: &DerivedCache,
) -> Option<features::TargetRelation> {
    if snap.ctx.project.profile.is_empty() && snap.profiles.profile.is_empty() {
        return None;
    }
    let path = find_profiles_file(&snap.profiles, &snap.root_path)?;
    let inventory =
        derived.profiles_inventory(&path, &snap.ctx.project.profile, &snap.profiles.allow);
    let (profile_name, target_name) =
        active_profile_target(&snap.profiles, &snap.ctx.project.profile, &inventory);
    let profile = inventory.profile(&profile_name)?;
    let target = profile.targets.iter().find(|t| t.name == target_name)?;
    Some(features::TargetRelation {
        database: target.database.clone(),
        schema: target.schema.clone(),
    })
}

/// "Switch target" code-action choices when `uri` IS the active
/// profiles.yml (every allowed profile×target, active pair marked).
pub fn target_choices(
    snap: &Snapshot,
    derived: &DerivedCache,
    uri: &str,
) -> Option<Vec<features::TargetChoice>> {
    let path = cogitator_lsp::uri_to_fs_path(uri)?;
    let active_file = find_profiles_file(&snap.profiles, &snap.root_path)?;
    // Path equality compares components, so `/` and `\` separators match
    if active_file.as_path() != std::path::Path::new(path.as_str()) {
        return None;
    }
    let inventory = derived.profiles_inventory(
        &active_file,
        &snap.ctx.project.profile,
        &snap.profiles.allow,
    );
    let (active_profile, active_target) =
        active_profile_target(&snap.profiles, &snap.ctx.project.profile, &inventory);
    let choices: Vec<_> = inventory
        .profiles
        .iter()
        .filter(|p| p.allowed)
        .flat_map(|p| {
            p.targets
                .iter()
                .map(|t| features::TargetChoice {
                    profile: p.name.clone(),
                    target: t.name.clone(),
                    active: p.name == active_profile && t.name == active_target,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    Some(choices)
}

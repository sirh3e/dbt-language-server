//! The redacted `dbt.profiles` wire report: names, targets, adapter types,
//! auth methods and issue messages — never a credential value, and no YAML
//! positions. The dispatcher fills `searched` and the active pair in.

use serde::Serialize;

use super::ProfilesInventory;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilesReport {
    /// `None` only on the not-found report → `"path": null`.
    pub path: Option<String>,
    pub searched: Vec<String>,
    pub profiles: Vec<ProfileReport>,
    /// Absent (not null) when no profiles.yml was found — the two only
    /// omitted-when-missing keys in the server's command responses.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_target: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileReport {
    pub name: String,
    pub default_target: String,
    pub allowed: bool,
    pub targets: Vec<TargetReport>,
}

#[derive(Debug, Serialize)]
pub struct TargetReport {
    pub name: String,
    #[serde(rename = "type")]
    pub adapter_type: String,
    /// Relation coordinates (database/schema names only, never secrets);
    /// explicit `null` when the target does not define them.
    pub database: Option<String>,
    pub schema: Option<String>,
    pub auth: AuthReport,
}

#[derive(Debug, Serialize)]
pub struct AuthReport {
    pub method: &'static str,
    pub issues: Vec<String>,
}

impl ProfilesInventory {
    /// The redacted report for this inventory; `searched` and the active
    /// profile/target start empty for the caller to fill.
    pub fn report(&self) -> ProfilesReport {
        ProfilesReport {
            path: Some(self.path.display().to_string()),
            searched: Vec::new(),
            profiles: self
                .profiles
                .iter()
                .map(|p| ProfileReport {
                    name: p.name.clone(),
                    default_target: p.default_target.clone(),
                    allowed: p.allowed,
                    targets: p
                        .targets
                        .iter()
                        .map(|t| TargetReport {
                            name: t.name.clone(),
                            adapter_type: t.adapter_type.clone(),
                            database: t.database.clone(),
                            schema: t.schema.clone(),
                            auth: AuthReport {
                                method: t.auth.method.as_str(),
                                issues: t.auth.issues.iter().map(|i| i.message.clone()).collect(),
                            },
                        })
                        .collect(),
                })
                .collect(),
            active_profile: None,
            active_target: None,
        }
    }
}

impl ProfilesReport {
    /// The `dbt.profiles` answer when no profiles.yml exists anywhere in
    /// the search order.
    pub fn not_found(searched: Vec<String>) -> Self {
        ProfilesReport {
            path: None,
            searched,
            profiles: Vec::new(),
            active_profile: None,
            active_target: None,
        }
    }
}

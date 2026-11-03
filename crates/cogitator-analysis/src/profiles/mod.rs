//! profiles.yml inventory: connection-auth classification and validation
//! (Snowflake first-class), behind a read scope so only allowed profiles
//! are parsed and exposed. Secrets are never stored or reported — only
//! which auth method a target uses and what is missing for it to work.

use std::path::{Path, PathBuf};

mod auth;
mod report;

pub use report::{AuthReport, ProfileReport, ProfilesReport, TargetReport};

use cogitator_lsp::Position;
use cogitator_schemas::yaml::{as_str, entries, get, position_of};
use saphyr::{LoadableYamlNode, MarkedYaml};

use crate::util;

use auth::classify_and_validate;

/// Which profiles may be parsed and exposed.
pub enum ProfileScope<'a> {
    /// Everything in the file (profiles.yml diagnostics in the editor).
    All,
    /// The project's own profile plus an explicit allowlist (the default
    /// for `dbt.profiles` / `dbt.setTarget`).
    Allowed {
        project: &'a str,
        extra: &'a [String],
    },
}

impl ProfileScope<'_> {
    fn allows(&self, name: &str) -> bool {
        match self {
            ProfileScope::All => true,
            ProfileScope::Allowed { project, extra } => {
                (!project.is_empty() && name == *project) || extra.iter().any(|e| e == name)
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct ProfilesInventory {
    pub path: PathBuf,
    pub profiles: Vec<ProfileInfo>,
}

#[derive(Debug)]
pub struct ProfileInfo {
    pub name: String,
    pub default_target: String,
    pub allowed: bool,
    pub targets: Vec<TargetInfo>,
}

#[derive(Debug)]
pub struct TargetInfo {
    pub name: String,
    pub adapter_type: String,
    /// Relation coordinates (never secrets): `database`/`dbname`, or for
    /// duckdb the `path` file stem; `schema`, defaulting to `main` on duckdb.
    pub database: Option<String>,
    pub schema: Option<String>,
    pub auth: AuthInfo,
    pub position: Position,
}

#[derive(Debug)]
pub struct AuthInfo {
    pub method: AuthMethod,
    pub issues: Vec<AuthIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    Password,
    KeyPair,
    /// SSO through the browser (`authenticator: externalbrowser`).
    ExternalBrowser,
    Oauth,
    UsernamePasswordMfa,
    Jwt,
    Unspecified,
}

impl AuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuthMethod::Password => "password",
            AuthMethod::KeyPair => "key-pair",
            AuthMethod::ExternalBrowser => "sso (externalbrowser)",
            AuthMethod::Oauth => "oauth",
            AuthMethod::UsernamePasswordMfa => "username+password+mfa",
            AuthMethod::Jwt => "jwt",
            AuthMethod::Unspecified => "unspecified",
        }
    }
}

#[derive(Debug)]
pub struct AuthIssue {
    pub message: String,
    pub position: Position,
}

impl ProfilesInventory {
    pub fn profile(&self, name: &str) -> Option<&ProfileInfo> {
        self.profiles.iter().find(|p| p.name == name)
    }
}

/// The target's relation coordinates — names only, never credentials.
/// `database` = `database` | `dbname`, or for duckdb the `path` file stem
/// (`:memory:` and absent yield `None`); `schema` defaults to `main` on
/// duckdb, matching dbt-duckdb.
fn relation_coordinates(
    target: &MarkedYaml<'_>,
    adapter_type: &str,
) -> (Option<String>, Option<String>) {
    let value = |key: &str| {
        get(target, key)
            .and_then(as_str)
            .map(util::resolve_env_vars)
    };
    let mut database = value("database").or_else(|| value("dbname"));
    let mut schema = value("schema");
    if adapter_type == "duckdb" {
        database = database.or_else(|| {
            let path = value("path")?;
            if path == ":memory:" {
                return None;
            }
            std::path::Path::new(&path)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        });
        schema = schema.or_else(|| Some("main".to_string()));
    }
    (database, schema)
}

/// Parse `text` (the content of `path`) under `scope`. Profiles outside the
/// scope appear by name only (`allowed: false`, no targets).
pub fn inventory(path: &Path, text: &str, scope: &ProfileScope<'_>) -> ProfilesInventory {
    let mut out = ProfilesInventory {
        path: path.to_path_buf(),
        profiles: Vec::new(),
    };
    let Ok(docs) = MarkedYaml::load_from_str(text) else {
        return out;
    };
    let Some(root) = docs.first() else {
        return out;
    };
    for (key, profile) in entries(root) {
        let Some(name) = as_str(key) else { continue };
        if name == "config" {
            continue; // profiles.yml global config block, not a profile
        }
        if !scope.allows(name) {
            out.profiles.push(ProfileInfo {
                name: name.to_string(),
                default_target: String::new(),
                allowed: false,
                targets: Vec::new(),
            });
            continue;
        }
        let default_target = get(profile, "target")
            .and_then(as_str)
            .map(util::resolve_env_vars)
            .unwrap_or_default();
        let mut targets = Vec::new();
        if let Some(outputs) = get(profile, "outputs") {
            for (tkey, tnode) in entries(outputs) {
                let Some(tname) = as_str(tkey) else { continue };
                let adapter_type = get(tnode, "type")
                    .and_then(as_str)
                    .map(util::resolve_env_vars)
                    .unwrap_or_default();
                let (database, schema) = relation_coordinates(tnode, &adapter_type);
                targets.push(TargetInfo {
                    name: tname.to_string(),
                    adapter_type: adapter_type.clone(),
                    database,
                    schema,
                    auth: classify_and_validate(tnode, &adapter_type, position_of(tkey)),
                    position: position_of(tkey),
                });
            }
        }
        out.profiles.push(ProfileInfo {
            name: name.to_string(),
            default_target,
            allowed: true,
            targets,
        });
    }
    out
}

/// Read `path` and build the inventory; a missing/unreadable file yields an
/// empty inventory carrying the path.
pub fn load_inventory(path: &Path, scope: &ProfileScope<'_>) -> ProfilesInventory {
    match std::fs::read_to_string(path) {
        Ok(text) => inventory(path, &text, scope),
        Err(_) => ProfilesInventory {
            path: path.to_path_buf(),
            profiles: Vec::new(),
        },
    }
}

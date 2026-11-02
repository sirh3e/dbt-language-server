//! profiles.yml inventory: connection-auth classification and validation
//! (Snowflake first-class), behind a read scope so only allowed profiles
//! are parsed and exposed. Secrets are never stored or reported — only
//! which auth method a target uses and what is missing for it to work.

use std::path::{Path, PathBuf};

use cogitator_lsp::Position;
use cogitator_schemas::yaml::{as_str, entries, get, position_of};
use saphyr::{LoadableYamlNode, MarkedYaml};
use serde_json::{json, Value};

use crate::util;

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

    /// Redacted JSON for `dbt.profiles`: names, targets, adapter types,
    /// auth methods and issues — never a credential value.
    pub fn to_json(&self) -> Value {
        json!({
            "path": self.path.display().to_string(),
            "profiles": self.profiles.iter().map(|p| json!({
                "name": p.name,
                "defaultTarget": p.default_target,
                "allowed": p.allowed,
                "targets": p.targets.iter().map(|t| json!({
                    "name": t.name,
                    "type": t.adapter_type,
                    "auth": {
                        "method": t.auth.method.as_str(),
                        "issues": t.auth.issues.iter()
                            .map(|i| i.message.clone())
                            .collect::<Vec<_>>(),
                    },
                })).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })
    }
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
                targets.push(TargetInfo {
                    name: tname.to_string(),
                    adapter_type: adapter_type.clone(),
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

// ---------------------------------------------------------------------------
// classification + validation
// ---------------------------------------------------------------------------

/// A target field as auth validation sees it: never the value itself.
/// (Unresolved env_var() references are reported separately for every
/// field by `classify_and_validate`; here they count as present.)
enum Field {
    Missing,
    UnresolvedEnv,
    Present,
}

fn field(target: &MarkedYaml<'_>, key: &str) -> Field {
    let Some(node) = get(target, key) else {
        return Field::Missing;
    };
    let Some(raw) = as_str(node) else {
        return Field::Present; // numbers, booleans
    };
    if !util::unresolved_env_vars(raw).is_empty() {
        return Field::UnresolvedEnv;
    }
    if util::resolve_env_vars(raw).trim().is_empty() {
        return Field::Missing;
    }
    Field::Present
}

fn classify_and_validate(target: &MarkedYaml<'_>, adapter: &str, at: Position) -> AuthInfo {
    let mut issues = Vec::new();

    // unresolved env_var() anywhere in the target is an issue regardless of
    // adapter — this is the most common broken-auth cause
    for (key, value) in entries(target) {
        if let (Some(key), Some(raw)) = (as_str(key), as_str(value)) {
            for name in util::unresolved_env_vars(raw) {
                issues.push(AuthIssue {
                    message: format!(
                        "`{key}` references env var {name}, which is not set and has no default"
                    ),
                    position: position_of(value),
                });
            }
        }
    }

    if adapter != "snowflake" {
        let method = match field(target, "password") {
            Field::Missing => AuthMethod::Unspecified,
            _ => AuthMethod::Password,
        };
        return AuthInfo { method, issues };
    }

    let authenticator = get(target, "authenticator")
        .and_then(as_str)
        .map(util::resolve_env_vars)
        .map(|a| a.to_lowercase());
    let method = match authenticator.as_deref() {
        Some("externalbrowser") => AuthMethod::ExternalBrowser,
        Some("oauth") => AuthMethod::Oauth,
        Some("username_password_mfa") => AuthMethod::UsernamePasswordMfa,
        Some("snowflake_jwt") | Some("jwt") => AuthMethod::Jwt,
        Some(other) => {
            issues.push(AuthIssue {
                message: format!(
                    "unknown authenticator `{other}` — expected externalbrowser, oauth, \
                     username_password_mfa or snowflake_jwt"
                ),
                position: at,
            });
            AuthMethod::Unspecified
        }
        None => match (
            field(target, "private_key_path"),
            field(target, "private_key"),
        ) {
            (Field::Missing, Field::Missing) => match field(target, "password") {
                Field::Missing => {
                    issues.push(AuthIssue {
                        message: "no auth method configured: set `password`, \
                                  `private_key_path`/`private_key`, or `authenticator`"
                            .to_string(),
                        position: at,
                    });
                    AuthMethod::Unspecified
                }
                _ => AuthMethod::Password,
            },
            _ => AuthMethod::KeyPair,
        },
    };

    let require = |issues: &mut Vec<AuthIssue>, key: &str, hint: &str| {
        if matches!(field(target, key), Field::Missing) {
            issues.push(AuthIssue {
                message: format!("`{key}` is missing ({hint})"),
                position: at,
            });
        }
    };
    require(
        &mut issues,
        "account",
        "required for every snowflake target",
    );
    match method {
        AuthMethod::Password => {
            require(&mut issues, "user", "password auth needs user + password");
            require(
                &mut issues,
                "password",
                "set it directly or via {{ env_var('…') }}",
            );
        }
        AuthMethod::KeyPair => {
            require(&mut issues, "user", "key-pair auth needs the user");
            if let Some(node) = get(target, "private_key_path") {
                if let Some(raw) = as_str(node) {
                    let resolved = util::resolve_env_vars(raw);
                    let path = expand_home_path(&resolved);
                    if util::unresolved_env_vars(raw).is_empty() && !path.is_file() {
                        issues.push(AuthIssue {
                            message: format!("private key file not found at {}", path.display()),
                            position: position_of(node),
                        });
                    }
                }
            }
        }
        AuthMethod::Oauth => {
            require(&mut issues, "token", "oauth auth needs a `token`");
        }
        AuthMethod::ExternalBrowser | AuthMethod::UsernamePasswordMfa | AuthMethod::Jwt => {
            require(&mut issues, "user", "this authenticator needs the user");
        }
        AuthMethod::Unspecified => {}
    }

    AuthInfo { method, issues }
}

fn expand_home_path(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = util::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

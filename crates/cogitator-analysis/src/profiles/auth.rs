//! Connection-auth classification and validation (Snowflake first-class).
//! Secrets never leave this module: only methods and issue messages.

use std::path::PathBuf;

use cogitator_lsp::Position;
use cogitator_schemas::yaml::{as_str, entries, get, position_of};
use saphyr::MarkedYaml;

use crate::util;

use super::{AuthInfo, AuthIssue, AuthMethod};

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

pub(super) fn classify_and_validate(
    target: &MarkedYaml<'_>,
    adapter: &str,
    at: Position,
) -> AuthInfo {
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

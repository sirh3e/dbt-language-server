//! `env_var('NAME'[, default])`: the value inline, a hover saying where
//! it comes from, definition into `.env` (else the profiles.yml use)
//! and a warning when dbt would fail on it — unset in the server's
//! environment with no default. Secret-looking names are masked
//! everywhere (`context::env::is_secret`).
//!
//! "Set" means set in the server's process environment, which is what
//! dbt — run by the server or from the same shell — inherits. dbt does
//! not read `.env`, so a value only there is shown but does not count.

use cogitator_error::guidance::Guidance;
use cogitator_lsp::{diagnostic_severity, inlay_hint_kind, Diagnostic, InlayHint, Location};
use cogitator_lsp::{Position, Range};
use cogitator_parser::{Token, TokenKind};

use super::{byte_position, file_uri, token_range, DbtContext, Document};
use crate::context::env::{shown_value, EnvValue, EnvVars};

/// Longest value a hint shows before `…`.
const HINT_CHARS: usize = 40;

/// The default argument's text of the env_var call at token `i`.
fn default_of(doc: &Document, i: usize) -> Option<&str> {
    doc.parsed.var_default(i)
}

/// A hint's label and tooltip for `name` (pure, for the case table).
pub(crate) fn env_hint_label(env: &EnvVars, name: &str, default: Option<&str>) -> (String, String) {
    match env.lookup(name) {
        EnvValue::Process(v) => {
            let shown = shown_value(name, v);
            (
                format!("= '{}'", shorten(&shown)),
                format!("{name} = {shown}\n\nfrom the server's environment"),
            )
        }
        EnvValue::DotenvOnly(e) => {
            let shown = shown_value(name, &e.value);
            let (label, then) = match default {
                Some(d) => (
                    format!("= {} (default)", shorten(d)),
                    format!("the default {d} applies"),
                ),
                None => ("= ? unset".to_string(), "dbt fails".to_string()),
            };
            (
                label,
                format!(
                    "{name} is only in .env (line {}: {shown}); dbt does not read .env, so {then}",
                    e.line + 1
                ),
            )
        }
        EnvValue::Unset => match default {
            Some(d) => (
                format!("= {} (default)", shorten(d)),
                format!("{name} is unset; the default {d} applies"),
            ),
            None => (
                "= ? unset".to_string(),
                format!("{name} is unset and has no default: dbt fails"),
            ),
        },
    }
}

fn shorten(text: &str) -> String {
    if text.chars().count() <= HINT_CHARS {
        return text.to_string();
    }
    let cut: String = text.chars().take(HINT_CHARS - 1).collect();
    format!("{cut}…")
}

/// The inlay hint after an `env_var()` call (token `i` is its name).
pub(crate) fn env_hint(ctx: &DbtContext, doc: &Document, i: usize, token: &Token) -> InlayHint {
    let parsed = &doc.parsed;
    let call = parsed.var_call(i);
    let default = default_of(doc, i);
    let position = match (default, call.close) {
        (Some(_), Some(close)) => {
            let t = &parsed.tokens[close];
            Position {
                line: t.line,
                character: t.column + t.len(),
            }
        }
        _ => Position {
            line: token.line,
            character: token.column + token.len() + 1,
        },
    };
    let (label, tooltip) = env_hint_label(&ctx.env, parsed.literal(token), default);
    InlayHint {
        position,
        label,
        kind: Some(inlay_hint_kind::TYPE),
        padding_left: true,
        padding_right: false,
        tooltip: Some(tooltip),
    }
}

/// Hover text of an `env_var()` name.
pub(crate) fn env_hover(ctx: &DbtContext, doc: &Document, idx: usize) -> String {
    let name = doc.parsed.literal(&doc.parsed.tokens[idx]);
    let default = default_of(doc, idx);
    let env = &ctx.env;
    let mut out = match env.lookup(name) {
        EnvValue::Process(v) => format!(
            "env_var {name}: {}\n\nset in the server's environment",
            shown_value(name, v)
        ),
        EnvValue::DotenvOnly(e) => format!(
            "env_var {name}: unset\n\n.env line {} has {}, but dbt does not read .env — \
             export it (direnv, `set -a; . ./.env`) for dbt to see it",
            e.line + 1,
            shown_value(name, &e.value)
        ),
        EnvValue::Unset => format!("env_var {name}: unset"),
    };
    if !matches!(env.lookup(name), EnvValue::Process(_)) {
        out.push_str(&match default {
            Some(d) => format!("\n\nthe default {d} applies"),
            None => "\n\nno default: dbt fails to compile this".to_string(),
        });
    }
    if let (EnvValue::Process(_), Some(e)) = (env.lookup(name), env.dotenv_entry(name)) {
        out.push_str(&format!("\n\nalso in .env, line {}", e.line + 1));
    }
    out
}

/// Definition of an `env_var()` name: its `.env` line, else its use in
/// the active profiles.yml (when that is not where the cursor is), else
/// the default argument.
pub(crate) fn env_definition(
    ctx: &DbtContext,
    doc: &Document,
    uri: &str,
    idx: usize,
) -> Option<Location> {
    let parsed = &doc.parsed;
    let name = parsed.literal(&parsed.tokens[idx]);
    let env = &ctx.env;
    if let (Some(path), Some(e)) = (&env.dotenv_path, env.dotenv_entry(name)) {
        let start = Position {
            line: e.line,
            character: 0,
        };
        return Some(Location {
            uri: file_uri(path),
            range: Range {
                start,
                end: Position {
                    line: e.line,
                    character: e.name.len() as u32,
                },
            },
        });
    }
    let here = token_range(&parsed.tokens[idx]);
    if let Some(path) = &env.profiles_path {
        let profiles = file_uri(path);
        let elsewhere = env
            .profile_uses
            .iter()
            .find(|(n, r)| n == name && !(profiles == uri && *r == here));
        if let Some((_, range)) = elsewhere {
            return Some(Location {
                uri: profiles,
                range: *range,
            });
        }
    }
    parsed.var_call(idx).default.map(|(s, e)| Location {
        uri: uri.to_string(),
        range: Range {
            start: byte_position(&parsed.text, s),
            end: byte_position(&parsed.text, e),
        },
    })
}

/// `env-var-unset` warnings: an `env_var()` without a default whose
/// variable the server's environment does not have.
pub fn env_var_diagnostics(ctx: &DbtContext, doc: &Document) -> Vec<Diagnostic> {
    let parsed = &doc.parsed;
    parsed
        .tokens
        .iter()
        .enumerate()
        .filter(|(i, t)| t.kind == TokenKind::EnvVar && parsed.var_default(*i).is_none())
        .filter_map(|(_, t)| {
            let name = parsed.literal(t);
            let guidance = match ctx.env.lookup(name) {
                EnvValue::Process(_) => return None,
                EnvValue::DotenvOnly(e) => Guidance::new(format!(
                    "env var '{name}' is not set in the server's environment and has no default"
                ))
                .because(format!(
                    ".env line {} defines it, but dbt does not read .env",
                    e.line + 1
                ))
                .fix("export it before starting the editor (direnv, `set -a; . ./.env`), or add a default"),
                EnvValue::Unset => Guidance::new(format!(
                    "env var '{name}' is not set in the server's environment and has no default"
                ))
                .fix(format!(
                    "export {name} before starting the editor, or add a default: env_var('{name}', '…')"
                )),
            };
            Some(Diagnostic::plain(
                token_range(t),
                guidance.to_diagnostic(),
                diagnostic_severity::WARNING,
                "env-var-unset",
                "cogitator",
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::env::parse_dotenv;
    use std::collections::HashMap;

    fn env() -> EnvVars {
        EnvVars {
            process: HashMap::from([
                ("DBT_SCHEMA".into(), "analytics".into()),
                ("DB_PASSWORD".into(), "hunter2".into()),
            ]),
            dotenv: parse_dotenv("ONLY_DOT=x\nDB_TOKEN=t\n"),
            ..Default::default()
        }
    }

    #[test]
    fn hint_labels() {
        let e = env();
        let cases = [
            ("DBT_SCHEMA", None, "= 'analytics'"),
            ("DB_PASSWORD", None, "= '••••••'"),
            ("ONLY_DOT", None, "= ? unset"),
            ("ONLY_DOT", Some("'d'"), "= 'd' (default)"),
            ("NOPE", None, "= ? unset"),
            ("NOPE", Some("'dev'"), "= 'dev' (default)"),
        ];
        for (name, default, want) in cases {
            assert_eq!(env_hint_label(&e, name, default).0, want, "{name}");
        }
        let (_, tip) = env_hint_label(&e, "DB_TOKEN", None);
        assert!(tip.contains("••••••") && !tip.contains(": t"), "{tip}");
        assert!(tip.contains("does not read .env"), "{tip}");
        let (_, tip) = env_hint_label(&e, "DB_PASSWORD", None);
        assert!(!tip.contains("hunter2"), "{tip}");
    }
}

//! The environment `env_var()` reads (dbt docs: env_var): the server's
//! process environment — what dbt, launched by the server or from the
//! same shell, inherits — plus `<root>/.env` and the env_var() uses of
//! the active profiles.yml, captured at context build so the features
//! stay pure.
//!
//! dbt itself never reads `.env`: a value written only there is shown
//! (and is a definition target), but does not count as set.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cogitator_lsp::Range;
use cogitator_parser::{DocumentParser, JinjaSqlParser, TokenKind};

/// One `NAME=value` line of a `.env` file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DotenvEntry {
    pub name: String,
    pub value: String,
    /// 0-based line of the entry.
    pub line: u32,
}

/// What `env_var()` can see (captured at context build).
#[derive(Debug, Clone, Default)]
pub struct EnvVars {
    /// The server's process environment.
    pub process: HashMap<String, String>,
    /// `<root>/.env`, when it exists.
    pub dotenv_path: Option<PathBuf>,
    pub dotenv: Vec<DotenvEntry>,
    /// The profiles.yml the project's profile is read from.
    pub profiles_path: Option<PathBuf>,
    /// `env_var('NAME')` uses in that profiles.yml: name and range.
    pub profile_uses: Vec<(String, Range)>,
}

/// Where an `env_var()` name's value comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvValue<'a> {
    /// Set in the server's environment (and so in dbt's).
    Process(&'a str),
    /// Only in `.env`, which dbt does not read.
    DotenvOnly(&'a DotenvEntry),
    /// Unset everywhere.
    Unset,
}

impl EnvVars {
    pub fn lookup(&self, name: &str) -> EnvValue<'_> {
        if let Some(v) = self.process.get(name) {
            return EnvValue::Process(v);
        }
        match self.dotenv_entry(name) {
            Some(e) => EnvValue::DotenvOnly(e),
            None => EnvValue::Unset,
        }
    }

    /// The last `.env` entry of `name` (later lines win, like shells).
    pub fn dotenv_entry(&self, name: &str) -> Option<&DotenvEntry> {
        self.dotenv.iter().rev().find(|e| e.name == name)
    }
}

/// Names whose values never appear in hints or hovers: dbt's own
/// secret prefix, and anything that reads like a credential.
pub fn is_secret(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.starts_with("DBT_ENV_SECRET_")
        || ["PASSWORD", "TOKEN", "SECRET", "KEY"]
            .iter()
            .any(|w| upper.contains(w))
}

/// A value as hints and hovers may show it.
pub fn shown_value(name: &str, value: &str) -> String {
    if is_secret(name) {
        "••••••".to_string()
    } else {
        value.to_string()
    }
}

/// `.env` lines: `NAME=value`, `export NAME=value`, optional single or
/// double quotes around the value, `#` comments and blank lines skipped.
pub fn parse_dotenv(text: &str) -> Vec<DotenvEntry> {
    text.lines()
        .enumerate()
        .filter_map(|(line, raw)| {
            let l = raw.trim();
            if l.is_empty() || l.starts_with('#') {
                return None;
            }
            let l = l.strip_prefix("export ").unwrap_or(l);
            let (name, value) = l.split_once('=')?;
            let name = name.trim();
            if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return None;
            }
            let value = value.trim();
            let value = ['"', '\'']
                .iter()
                .find_map(|q| value.strip_prefix(*q)?.strip_suffix(*q))
                .unwrap_or_else(|| value.split(" #").next().unwrap_or(value).trim());
            Some(DotenvEntry {
                name: name.to_string(),
                value: value.to_string(),
                line: line as u32,
            })
        })
        .collect()
}

/// `env_var('NAME')` uses of a text, by the same parser as documents.
pub fn env_var_uses(text: &str) -> Vec<(String, Range)> {
    let Ok(parsed) = JinjaSqlParser.parse(text, &Default::default()) else {
        return Vec::new();
    };
    parsed
        .tokens
        .iter()
        .filter(|t| t.kind == TokenKind::EnvVar)
        .map(|t| {
            (
                parsed.literal(t).to_string(),
                crate::features::token_range(t),
            )
        })
        .collect()
}

/// The shell: read `.env` and the profiles.yml at build time.
pub(super) fn load_env(root: &Path, profiles_dir: &str, workspace_root: &str) -> EnvVars {
    let dotenv_path = root.join(".env");
    let (dotenv_path, dotenv) = match std::fs::read_to_string(&dotenv_path) {
        Ok(text) => (Some(dotenv_path), parse_dotenv(&text)),
        Err(_) => (None, Vec::new()),
    };
    let profiles = crate::util::profiles_candidates(profiles_dir, workspace_root)
        .into_iter()
        .find_map(|p| std::fs::read_to_string(&p).ok().map(|t| (p, t)));
    let (profiles_path, profile_uses) = match profiles {
        Some((path, text)) => (Some(path), env_var_uses(&text)),
        None => (None, Vec::new()),
    };
    EnvVars {
        process: std::env::vars().collect(),
        dotenv_path,
        dotenv,
        profiles_path,
        profile_uses,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dotenv_lines() {
        let text = "# c\nA=1\nexport B = \"two words\"\nC='x'\n\nbad line\nD=v # note\n9-x=1\n";
        let got: Vec<(String, String, u32)> = parse_dotenv(text)
            .into_iter()
            .map(|e| (e.name, e.value, e.line))
            .collect();
        assert_eq!(
            got,
            vec![
                ("A".into(), "1".into(), 1),
                ("B".into(), "two words".into(), 2),
                ("C".into(), "x".into(), 3),
                ("D".into(), "v".into(), 6),
            ]
        );
    }

    #[test]
    fn secrets_are_recognised() {
        for (name, secret) in [
            ("DBT_ENV_SECRET_GIT", true),
            ("SNOWFLAKE_PASSWORD", true),
            ("api_token", true),
            ("AWS_SECRET_ACCESS_KEY", true),
            ("PRIVATE_KEY_PATH", true),
            ("DBT_SCHEMA", false),
            ("DBT_TARGET", false),
        ] {
            assert_eq!(is_secret(name), secret, "{name}");
        }
        assert_eq!(shown_value("X_TOKEN", "abc"), "••••••");
        assert_eq!(shown_value("SCHEMA", "abc"), "abc");
    }

    #[test]
    fn lookup_prefers_the_process() {
        let env = EnvVars {
            process: HashMap::from([("A".into(), "p".into())]),
            dotenv: parse_dotenv("A=d\nB=1\nB=2\n"),
            ..Default::default()
        };
        assert_eq!(env.lookup("A"), EnvValue::Process("p"));
        assert!(matches!(env.lookup("B"), EnvValue::DotenvOnly(e) if e.value == "2"));
        assert_eq!(env.lookup("C"), EnvValue::Unset);
    }

    #[test]
    fn uses_in_profiles_text() {
        let uses = env_var_uses(
            "p:\n  outputs:\n    dev:\n      schema: \"{{ env_var('DBT_SCHEMA', 'x') }}\"\n",
        );
        assert_eq!(uses.len(), 1);
        assert_eq!(uses[0].0, "DBT_SCHEMA");
        assert_eq!(uses[0].1.start.line, 3);
    }
}

//! The formatting rules. Every key is optional and has the default below;
//! the names follow sqlfluff's (`.sqlfluff`), and each rule says which of
//! sqlfluff's rules it plays the part of.
//!
//! A standalone run reads `cogitator-format.toml` (or `.cogitator-format.toml`),
//! the nearest one at or above the file; the language server also takes the
//! same keys under `format:` in `cogitator.yml`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The file names a standalone run looks for, nearest first.
pub const CONFIG_FILES: [&str; 2] = ["cogitator-format.toml", ".cogitator-format.toml"];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// sqlfluff's dialect name (`duckdb`, `snowflake`, `bigquery`, …).
    pub dialect: String,
    /// The longest line the layout aims for (LT05).
    pub max_line_length: usize,
    /// `space` or `tab` (LT02's `indent_unit`).
    pub indent_unit: IndentUnit,
    /// Spaces per indent, and a tab's width when measuring (`tab_space_size`).
    pub tab_space_size: usize,
    /// Blank lines kept where the source had more (LT08 keeps one).
    pub max_blank_lines: usize,
    /// End the file with exactly one newline (LT12).
    pub trailing_newline: bool,
    pub case: CaseRules,
    pub layout: LayoutRules,
    pub jinja: JinjaRules,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            dialect: "ansi".into(),
            max_line_length: 80,
            indent_unit: IndentUnit::Space,
            tab_space_size: 4,
            max_blank_lines: 1,
            trailing_newline: true,
            case: CaseRules::default(),
            layout: LayoutRules::default(),
            jinja: JinjaRules::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndentUnit {
    Space,
    Tab,
}

/// How a word is written: sqlfluff's `capitalisation_policy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Case {
    Upper,
    Lower,
    /// First letter upper, the rest lower.
    Capitalise,
    /// As written.
    Preserve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CaseRules {
    /// Keywords (CP01).
    pub keywords: Case,
    /// Unquoted identifiers (CP02); quoted ones are never changed.
    pub identifiers: Case,
    /// Function names (CP03).
    pub functions: Case,
    /// `null`, `true` and `false` (CP04).
    pub literals: Case,
    /// Data type names (CP05).
    pub types: Case,
}

impl Default for CaseRules {
    fn default() -> Self {
        CaseRules {
            keywords: Case::Lower,
            identifiers: Case::Preserve,
            functions: Case::Lower,
            literals: Case::Lower,
            types: Case::Lower,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Commas {
    /// `a,\n    b` (LT04's `trailing`).
    Trailing,
    /// `a\n    , b` (LT04's `leading`).
    Leading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectTargets {
    /// More than one target: each on a line of its own (LT09).
    OnePerLine,
    /// On one line while they fit.
    Fit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayoutRules {
    pub commas: Commas,
    pub select_targets: SelectTargets,
    /// `join` indented under `from` (sqlfluff's `indented_joins`).
    pub indented_joins: bool,
    /// A blank line after each CTE's closing bracket (LT08).
    pub blank_line_after_cte: bool,
}

impl Default for LayoutRules {
    fn default() -> Self {
        LayoutRules {
            commas: Commas::Trailing,
            select_targets: SelectTargets::OnePerLine,
            indented_joins: false,
            blank_line_after_cte: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JinjaRules {
    /// One space inside the delimiters: `{{ x }}`, `{% if x %}`.
    pub delimiter_spacing: bool,
    /// Lay a tag's inside out again from minijinja's tokens:
    /// `ref( 'x' )` → `ref('x')`, `a|b` → `a | b`. A tag over several
    /// lines is kept as written.
    pub normalize_tags: bool,
    /// Spaces around a filter's `|`.
    pub filter_spacing: bool,
    /// Indent the lines inside `{% if %}`, `{% for %}`, `{% macro %}` …
    /// one level (sqlfluff's `template_blocks_indent`).
    pub indent_blocks: bool,
}

impl Default for JinjaRules {
    fn default() -> Self {
        JinjaRules {
            delimiter_spacing: true,
            normalize_tags: true,
            filter_spacing: true,
            indent_blocks: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Parse {
        path: PathBuf,
        source: Box<toml::de::Error>,
    },
    #[error("unknown dialect `{0}`; one of: {names}", names = crate::sql::dialect_names().join(", "))]
    Dialect(String),
}

impl Config {
    /// The rules of a TOML text; `path` only names it in errors.
    pub fn from_toml(text: &str, path: &Path) -> Result<Config, ConfigError> {
        toml::from_str::<Config>(text)
            .map_err(|source| ConfigError::Parse {
                path: path.to_path_buf(),
                source: Box::new(source),
            })
            .and_then(Config::checked)
    }

    /// Refuses what cannot be formatted with: an unknown dialect.
    pub fn checked(self) -> Result<Config, ConfigError> {
        crate::sql::dialect(&self.dialect)
            .map(|_| self.clone())
            .ok_or(ConfigError::Dialect(self.dialect))
    }

    /// The width of one indent, in columns.
    pub fn indent_width(&self) -> usize {
        self.tab_space_size.max(1)
    }

    /// The text of `levels` indents.
    pub fn indent(&self, levels: usize) -> String {
        match self.indent_unit {
            IndentUnit::Tab => "\t".repeat(levels),
            IndentUnit::Space => " ".repeat(levels * self.indent_width()),
        }
    }
}

/// The nearest config file at or above `start` (a file or a directory).
pub fn find_config(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .flat_map(|dir| CONFIG_FILES.iter().map(move |name| dir.join(name)))
        .find(|candidate| candidate.is_file())
}

/// The rules for a file at `start`: the nearest config file's, or the defaults.
pub fn load_for(start: &Path) -> Result<Config, ConfigError> {
    find_config(start).map_or(Ok(Config::default()), |path| {
        std::fs::read_to_string(&path)
            .map_err(|source| ConfigError::Read {
                path: path.clone(),
                source,
            })
            .and_then(|text| Config::from_toml(&text, &path))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_is_optional() {
        let config = Config::from_toml("", Path::new("x.toml")).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn keys_read_as_sqlfluff_names_them() {
        let text = r#"
            dialect = "duckdb"
            max_line_length = 100
            indent_unit = "tab"
            [case]
            keywords = "upper"
            [layout]
            commas = "leading"
            [jinja]
            indent_blocks = false
        "#;
        let config = Config::from_toml(text, Path::new("x.toml")).unwrap();
        assert_eq!(config.dialect, "duckdb");
        assert_eq!(config.max_line_length, 100);
        assert_eq!(config.indent(2), "\t\t");
        assert_eq!(config.case.keywords, Case::Upper);
        assert_eq!(config.layout.commas, Commas::Leading);
        assert!(!config.jinja.indent_blocks);
    }

    #[test]
    fn unknown_keys_and_dialects_are_refused() {
        let err = Config::from_toml("max_lenght = 1", Path::new("x.toml")).unwrap_err();
        assert!(err.to_string().contains("max_lenght"), "{err}");
        let err = Config::from_toml("dialect = \"nope\"", Path::new("x.toml")).unwrap_err();
        assert!(err.to_string().contains("unknown dialect `nope`"), "{err}");
    }

    #[test]
    fn the_nearest_file_wins() {
        let dir = tempfile::tempdir().unwrap();
        let inner = dir.path().join("models/staging");
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(dir.path().join(CONFIG_FILES[0]), "max_line_length = 1\n").unwrap();
        std::fs::write(dir.path().join("models").join(CONFIG_FILES[1]), "").unwrap();
        let found = find_config(&inner.join("m.sql")).unwrap();
        assert_eq!(found, dir.path().join("models").join(CONFIG_FILES[1]));
    }
}

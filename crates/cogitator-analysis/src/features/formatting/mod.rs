//! textDocument/formatting (05 §9) on the `cogitator-format` crate: SQL
//! laid out on sqlfluff's parse tree, Jinja on minijinja's tokens, by the
//! rules of `cogitator.yml`'s `format:` or the nearest
//! `cogitator-format.toml`. A model the formatter cannot keep intact is
//! declined: no edits, never a corrupted file.

use cogitator_format::{Config, Declined, IndentUnit};
use cogitator_lsp::{Position, Range, TextEdit};

/// What a request formats by.
#[derive(Debug, Clone, Default)]
pub struct FormatOptions {
    /// The editor's indent, used when no rules were configured.
    pub tab_size: u8,
    pub insert_spaces: bool,
    /// The configured rules, if any.
    pub rules: Option<Config>,
    /// The project's dbt adapter (`duckdb`, `spark`): the dialect where the
    /// rules name none.
    pub adapter: String,
}

impl FormatOptions {
    /// The rules this request formats by: the configured ones, else the
    /// defaults with the editor's indent; the adapter's dialect where they
    /// name none.
    pub fn config(&self) -> Config {
        let rules = self.rules.clone().unwrap_or_else(|| Config {
            indent_unit: if self.insert_spaces {
                IndentUnit::Space
            } else {
                IndentUnit::Tab
            },
            tab_space_size: usize::from(self.tab_size.max(1)),
            ..Config::default()
        });
        rules.with_adapter(&self.adapter)
    }
}

/// The rules of the nearest cogitator-format.toml at or above `path`, if
/// there is one.
pub fn rules_near(path: &std::path::Path) -> Result<Option<Config>, cogitator_format::ConfigError> {
    cogitator_format::config::find_config(path)
        .map(|file| {
            std::fs::read_to_string(&file)
                .map_err(|source| cogitator_format::ConfigError::Read {
                    path: file.clone(),
                    source,
                })
                .and_then(|text| Config::from_toml(&text, &file))
        })
        .transpose()
}

/// The formatted text, or why the document is left as it is.
pub fn format_text(text: &str, options: &FormatOptions) -> Result<String, Declined> {
    cogitator_format::format(text, &options.config())
}

/// The whole-document edit, `Some(vec![])` when already formatted,
/// `None` when the document is not safely formattable.
pub fn format_document(text: &str, options: &FormatOptions) -> Option<Vec<TextEdit>> {
    match format_text(text, options) {
        Ok(out) if out == text => Some(Vec::new()),
        Ok(out) => Some(vec![whole_document(text, out)]),
        Err(declined) => {
            tracing::debug!(%declined, "formatting declined");
            None
        }
    }
}

fn whole_document(text: &str, new_text: String) -> TextEdit {
    let end_line = text.lines().count() as u32 + 1;
    TextEdit {
        range: Range {
            start: Position::default(),
            end: Position {
                line: end_line,
                character: 0,
            },
        },
        new_text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> FormatOptions {
        FormatOptions {
            tab_size: 4,
            insert_spaces: true,
            rules: None,
            adapter: "duckdb".into(),
        }
    }

    fn fmt(text: &str) -> Option<String> {
        format_document(text, &options()).map(|edits| {
            edits
                .first()
                .map(|e| e.new_text.clone())
                .unwrap_or_else(|| text.to_string())
        })
    }

    #[test]
    fn crlf_endings_are_no_change_and_are_kept() {
        let lf = "select a,b from {{ ref('x') }} where a>1\n";
        let formatted = fmt(lf).unwrap();
        let crlf = formatted.replace('\n', "\r\n");
        assert_eq!(format_document(&crlf, &options()), Some(Vec::new()));
        let edit = fmt(&lf.replace('\n', "\r\n")).unwrap();
        assert_eq!(edit, crlf);
    }

    #[test]
    fn formats_and_is_idempotent_with_jinja_preserved() {
        let input =
            "{{ config(materialized='table') }}\nselect a,b from {{ ref('stg_x') }} where a>1\n";
        let once = fmt(input).expect("formats");
        assert!(once.contains("from {{ ref('stg_x') }}"), "{once}");
        assert!(
            once.starts_with("{{ config(materialized='table') }}\n"),
            "{once}"
        );
        assert_eq!(fmt(&once).as_deref(), Some(once.as_str()), "idempotent");
    }

    #[test]
    fn statements_keep_their_lines_and_indent_their_blocks() {
        let input = "{% set x = 1 %}\nselect {{ x }} as v\n{% if flag %}\nunion all\nselect 2 as w\n{% endif %}\n";
        let formatted = fmt(input).expect("formats");
        assert!(formatted.contains("{% set x = 1 %}\n"), "{formatted}");
        assert!(
            formatted.contains("{% if flag %}\n    union all\n"),
            "{formatted}"
        );
        assert!(formatted.ends_with("{% endif %}\n"), "{formatted}");
    }

    #[test]
    fn inline_statements_stay_inline() {
        let formatted = fmt("select 1 {% if x %} ,2 {% endif %} from t\n").expect("formats");
        assert!(formatted.contains("{% if x %}"), "{formatted}");
        assert!(formatted.contains("{% endif %}"), "{formatted}");
    }

    #[test]
    fn what_cannot_be_kept_intact_is_declined() {
        assert_eq!(fmt("select {{ unclosed\n"), None);
    }

    #[test]
    fn the_editors_indent_without_rules_and_the_rules_over_it() {
        let tabs = FormatOptions {
            insert_spaces: false,
            ..options()
        };
        let out = format_text("select a, b from t", &tabs).unwrap();
        assert!(out.contains("\n\ta,\n"), "{out:?}");
        let ruled = FormatOptions {
            rules: Some(Config {
                tab_space_size: 2,
                ..Config::default()
            }),
            ..tabs
        };
        let out = format_text("select a, b from t", &ruled).unwrap();
        assert!(out.contains("\n  a,\n"), "{out:?}");
    }

    #[test]
    fn the_adapter_names_the_dialect() {
        let config = FormatOptions {
            adapter: "spark".into(),
            ..options()
        }
        .config();
        assert_eq!(config.dialect_name(), "sparksql");
    }
}

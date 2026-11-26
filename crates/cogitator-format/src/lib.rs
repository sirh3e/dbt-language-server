//! A formatter for dbt models: SQL with Jinja in it.
//!
//! - **SQL** is laid out on the parse tree of sqlfluff's Rust parser
//!   (sqlfluffrs, vendored in `vendor/sqlfluffrs`), in any of its dialects,
//!   at sqlfluff's indent points, by the rules of a [`Config`].
//! - **Jinja** is read with minijinja's own tokenizer: each tag's inside is
//!   laid out again (`{{ref( 'x' )}}` → `{{ ref('x') }}`), and the lines
//!   inside `{% if %}`, `{% for %}`, `{% macro %}` … are indented.
//!
//! It never changes what a model means. Only whitespace and the case of
//! keywords and names move: the output is lexed again and must hold the
//! same tokens in the same order, every tag must come back exactly once,
//! and SQL the parser cannot read is declined ([`Declined`]) rather than
//! guessed at.
//!
//! The crate stands alone (`cogitator-format`, the CLI in `main.rs`); the
//! language server embeds the library for `textDocument/formatting`.

pub mod config;
pub mod jinja;
mod mask;
pub mod sql;

pub use config::{Case, CaseRules, Commas, Config, ConfigError, IndentUnit, JinjaRules};

use mask::{Mode, Unit};

/// Why a document was left as it is.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Declined {
    #[error("its Jinja cannot be read: {0}")]
    Jinja(String),
    #[error("its SQL cannot be parsed as {dialect}: {reason}")]
    Unparsable { dialect: String, reason: String },
    #[error("the layout would change its tokens (a formatter bug; please report it)")]
    Changed,
    #[error("unknown dialect `{0}`")]
    Dialect(String),
}

/// The document laid out by `config`'s rules.
///
/// Line endings are kept: a file with CRLF is formatted as its LF text and
/// written back with CRLF.
pub fn format(source: &str, config: &Config) -> Result<String, Declined> {
    if source.contains("\r\n") {
        let lf = source.replace("\r\n", "\n");
        return format_lf(&lf, config).map(|out| out.replace('\n', "\r\n"));
    }
    format_lf(source, config)
}

const MODES: [Mode; 3] = [
    Mode::Whole {
        header_as_tags: true,
    },
    Mode::Whole {
        header_as_tags: false,
    },
    Mode::Segments,
];

fn format_lf(source: &str, config: &Config) -> Result<String, Declined> {
    let attempts = attempts(source, config)?;
    let mut first_error = None;
    for (_, outcome) in attempts {
        match outcome {
            Ok(out) => return Ok(out),
            Err(Declined::Changed) => return Err(Declined::Changed),
            Err(e) => {
                first_error.get_or_insert(e);
            }
        }
    }
    Err(first_error.expect("at least one mode"))
}

/// Each way of masking the Jinja in turn, until one is laid out: what each
/// came to, for `--explain` and the server's log.
pub fn explain(source: &str, config: &Config) -> Vec<String> {
    let lf = source.replace("\r\n", "\n");
    match attempts(&lf, config) {
        Err(declined) => vec![declined.to_string()],
        Ok(attempts) => attempts
            .into_iter()
            .map(|(mode, outcome)| match outcome {
                Ok(_) => format!("{mode:?}: laid out"),
                Err(e) => format!("{mode:?}: {e}"),
            })
            .collect(),
    }
}

/// The masks tried, lazily up to the first that lays out.
fn attempts(
    source: &str,
    config: &Config,
) -> Result<Vec<(Mode, Result<String, Declined>)>, Declined> {
    let dialect =
        sql::dialect(&config.dialect).ok_or_else(|| Declined::Dialect(config.dialect.clone()))?;
    let pieces = jinja::split(source).map_err(Declined::Jinja)?;
    let written: Vec<String> = pieces
        .iter()
        .map(|p| jinja::format_tag(p, &config.jinja))
        .collect();
    let mut done = false;
    Ok(MODES
        .into_iter()
        .map_while(|mode| {
            (!done).then(|| {
                let masked = mask::mask(source, &pieces, &written, mode);
                let outcome = lay_out(&masked, dialect, config, mode == Mode::Segments)
                    .map(|out| finish(&out, config));
                done = matches!(outcome, Ok(_) | Err(Declined::Changed));
                (mode, outcome)
            })
        })
        .collect())
}

/// One way of masking laid out; `keep_unparsable`: SQL between two lone
/// tags that the parser cannot read (a macro's body is often a fragment,
/// or not SQL at all) is kept byte for byte, as sqlfluff leaves unparsable
/// sections, while the rest is laid out.
fn lay_out(
    masked: &mask::Masked,
    dialect: sqlfluffrs_dialects::Dialect,
    config: &Config,
    keep_unparsable: bool,
) -> Result<String, Declined> {
    let unparsable = |reason: String| Declined::Unparsable {
        dialect: config.dialect.clone(),
        reason: mask::unmask_all(&reason, &masked.restore),
    };
    let trees = masked
        .units
        .iter()
        .map(|unit| match unit {
            Unit::Sql(text) => match sql::parse(text, dialect) {
                Ok(tree) => Ok(Some(tree)),
                Err(_) if keep_unparsable => Ok(None),
                Err(reason) => Err(reason),
            },
            Unit::Tag { .. } => Ok(None),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(unparsable)?;
    // SQL kept as written is printed as a placeholder of its shape, which
    // the check reads instead (it may not even lex), and goes back after
    let kept: Vec<Option<(String, String)>> = masked
        .units
        .iter()
        .zip(&trees)
        .enumerate()
        .map(|(i, (unit, tree))| match (unit, tree) {
            (Unit::Sql(text), None) => Some((
                mask::kept_placeholder(&masked.tags.prefix, i, text.trim()),
                text.trim().to_string(),
            )),
            _ => None,
        })
        .collect();
    let events: Vec<_> = masked
        .units
        .iter()
        .zip(&trees)
        .zip(&kept)
        .flat_map(|((unit, tree), kept)| match (unit, tree, kept) {
            (_, Some(tree), _) => sql::events(tree, config),
            (Unit::Tag { placeholder, newlines }, None, _) => {
                vec![sql::tag_leaf(placeholder, *newlines)]
            }
            (Unit::Sql(text), None, Some((placeholder, _))) => {
                vec![sql::verbatim_leaf(placeholder, mask::leading_newlines(text))]
            }
            (Unit::Sql(_), None, None) => Vec::new(),
        })
        .collect();
    let printed = sql::print(&sql::layout(&events, config, &masked.tags), config);
    let expected = masked
        .units
        .iter()
        .zip(&kept)
        .map(|(unit, kept)| match (unit, kept) {
            (_, Some((placeholder, _))) => Ok(vec![placeholder.clone()]),
            (Unit::Sql(text), None) => sql::code_tokens(text, dialect),
            (Unit::Tag { placeholder, .. }, None) => Ok(vec![placeholder.clone()]),
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(unparsable)?
        .concat();
    let got = sql::code_tokens(&printed, dialect).map_err(unparsable)?;
    if got != expected {
        if std::env::var_os("COGITATOR_FORMAT_DEBUG").is_some() {
            let at = got.iter().zip(&expected).position(|(a, b)| a != b).unwrap_or(0);
            let around = |v: &[String]| v[at.saturating_sub(2)..(at + 3).min(v.len())].to_vec();
            eprintln!("tokens differ at {at}: {:?} for {:?}", around(&got), around(&expected));
        }
        return Err(Declined::Changed);
    }
    let restore: Vec<(String, String)> = kept
        .into_iter()
        .flatten()
        .chain(masked.restore.iter().cloned())
        .collect();
    mask::unmask(&printed, &restore).ok_or_else(|| {
        if std::env::var_os("COGITATOR_FORMAT_DEBUG").is_some() {
            let missing: Vec<_> = restore
                .iter()
                .filter(|(p, _)| printed.matches(p.as_str()).count() != 1)
                .map(|(p, _)| p)
                .collect();
            eprintln!("placeholders not there once: {missing:?}");
        }
        Declined::Changed
    })
}

/// No space at a line's end, no blank lines at the start, and the end of
/// the file as the rules say (LT12).
fn finish(out: &str, config: &Config) -> String {
    let body = out
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    let body = body.trim_matches('\n');
    if config.trailing_newline && !body.is_empty() {
        format!("{body}\n")
    } else {
        body.to_string()
    }
}

//! Application error type (specs/rust-port/05 §2).
//!
//! Every library-style crate returns [`Error`]; `anyhow` lives only at binary
//! edges. The feature layer converts specific variants into the documented
//! default LSP responses — degradation is explicit, never silent.

pub mod guidance;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("malformed LSP frame: {reason}")]
    Frame { reason: String },
    #[error("yaml error in {path}: {message}")]
    Yaml { path: PathBuf, message: String },
    #[error("jinja error in {path}: {message}")]
    Jinja { path: PathBuf, message: String },
    #[error("sql tokenizer: {0}")]
    SqlTokenizer(#[from] sqlparser::tokenizer::TokenizerError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Scan-side degradation helper (05 §2): log-and-skip with a tracing event at
/// the call site kept uniform.
pub fn ok_or_log<T>(result: Result<T>, context: &str) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(err) => {
            tracing::debug!(context, %err, "skipping after recoverable error");
            None
        }
    }
}

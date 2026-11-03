//! dbt.compiledSql shell: bounded read of the compiled file the pure
//! feature points at — never more than the wire cap over the channel.

use cogitator_analysis::features::{compiled_target, CompiledSql};
use cogitator_analysis::DbtContext;

/// Compiled files past this size answer `content: null` plus a warning;
/// editors open the `uri` directly instead.
const MAX_COMPILED_BYTES: u64 = 2 * 1024 * 1024;

/// The dbt.compiledSql answer and an optional client warning. `None` when
/// the model itself is unknown; `uri`/`content` null when dbt has not
/// compiled it (the caller shows its own hint in that case).
pub fn compiled_sql(
    ctx: &DbtContext,
    uri: &str,
    model: Option<&str>,
) -> (Option<CompiledSql>, Option<String>) {
    let Some((name, path)) = compiled_target(ctx, uri, model) else {
        return (None, None);
    };
    let not_compiled = |name: String| CompiledSql {
        model: name,
        uri: None,
        content: None,
    };
    let Ok(meta) = std::fs::metadata(&path) else {
        return (Some(not_compiled(name)), None);
    };
    let file_uri = cogitator_lsp::fs_path_to_uri(&path);
    if meta.len() > MAX_COMPILED_BYTES {
        let warning = cogitator_error::guidance::Guidance::new(format!(
            "Compiled SQL for '{name}' was not sent."
        ))
        .at(path.display().to_string())
        .because(format!(
            "the file is {:.1} MiB, over the {} MiB wire cap.",
            meta.len() as f64 / (1024.0 * 1024.0),
            MAX_COMPILED_BYTES / (1024 * 1024),
        ))
        .fix("open the compiled file directly.")
        .to_message();
        return (
            Some(CompiledSql {
                model: name,
                uri: Some(file_uri),
                content: None,
            }),
            Some(warning),
        );
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => (
            Some(CompiledSql {
                model: name,
                uri: Some(file_uri),
                content: Some(content),
            }),
            None,
        ),
        Err(_) => (Some(not_compiled(name)), None),
    }
}

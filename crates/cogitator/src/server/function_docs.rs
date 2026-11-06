//! Materialized SQL-function documentation (03 §9): definition on a
//! built-in writes the bundled markdown to a cached file under
//! target/.cogitator/docs/<dialect>/<name>.md and jumps there — the
//! stdlib-sources move, in Markdown. Static per binary, so the file
//! rewrites only when its content drifted.

use std::path::{Path, PathBuf};

use cogitator_analysis::features::FunctionDoc;

/// The cache file for `function`, created/refreshed as needed. Any
/// write failure is a trace and a None — the caller falls back to the
/// plain self-location, never an error.
pub fn materialize(project_root: &Path, function: &FunctionDoc) -> Option<PathBuf> {
    // dots are fine (bigquery's aead.encrypt); path separators are not
    let stem: String = function
        .name
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let dir = project_root
        .join("target")
        .join(".cogitator")
        .join("docs")
        .join(&function.dialect);
    let path = dir.join(format!("{stem}.md"));
    let fresh = std::fs::read_to_string(&path)
        .map(|existing| existing == function.markdown)
        .unwrap_or(false);
    if fresh {
        return Some(path);
    }
    let write =
        std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&path, function.markdown));
    match write {
        Ok(()) => Some(path),
        Err(err) => {
            tracing::debug!(path = %path.display(), %err, "function doc not materialized");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_once_and_refreshes_on_drift() {
        let root = std::env::temp_dir().join(format!("cogitator-fndocs-{}", std::process::id()));
        std::fs::remove_dir_all(&root).ok();
        let doc = FunctionDoc {
            dialect: "snowflake".into(),
            name: "sum".into(),
            markdown: "```sql\nSUM(x)\n```",
        };
        let path = materialize(&root, &doc).expect("writes");
        assert!(path.ends_with("target/.cogitator/docs/snowflake/sum.md"));
        let first = std::fs::metadata(&path).unwrap().modified().unwrap();
        // identical content: untouched
        let again = materialize(&root, &doc).expect("cached");
        assert_eq!(path, again);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), first);
        std::fs::remove_dir_all(&root).ok();
    }
}

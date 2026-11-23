//! Which SQL a preview cuts: dbt's compiled file when it still describes
//! the model, cogitator's own rendering otherwise.

use std::time::SystemTime;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PreviewSource {
    /// target/compiled/<project>/<model path>, written by dbt.
    Compiled,
    /// The model's Jinja rendered by the server (dbt.expandJinja).
    Rendered,
}

/// The compiled file wins when it exists, is not older than the model
/// file, and the editor buffer has no unsaved edits; anything else
/// renders, so a preview never runs SQL the buffer no longer says.
pub fn choose_source(
    compiled_mtime: Option<SystemTime>,
    model_mtime: Option<SystemTime>,
    buffer_matches_disk: bool,
) -> PreviewSource {
    match (compiled_mtime, model_mtime) {
        (Some(compiled), Some(model)) if compiled >= model && buffer_matches_disk => {
            PreviewSource::Compiled
        }
        (Some(_), None) if buffer_matches_disk => PreviewSource::Compiled,
        _ => PreviewSource::Rendered,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn the_compiled_file_wins_only_while_it_is_current() {
        let t0 = SystemTime::UNIX_EPOCH + Duration::from_secs(100);
        let t1 = t0 + Duration::from_secs(5);
        let cases = [
            (Some(t1), Some(t0), true, PreviewSource::Compiled),
            (Some(t0), Some(t0), true, PreviewSource::Compiled),
            (Some(t0), Some(t1), true, PreviewSource::Rendered),
            (Some(t1), Some(t0), false, PreviewSource::Rendered),
            (None, Some(t0), true, PreviewSource::Rendered),
            (Some(t0), None, true, PreviewSource::Compiled),
            (Some(t0), None, false, PreviewSource::Rendered),
            (None, None, true, PreviewSource::Rendered),
        ];
        for (compiled, model, clean, want) in cases {
            assert_eq!(
                choose_source(compiled, model, clean),
                want,
                "{compiled:?} {model:?} {clean}"
            );
        }
    }
}

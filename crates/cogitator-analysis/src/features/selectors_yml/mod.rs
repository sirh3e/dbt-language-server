//! selectors.yml intelligence on the selector engine (`crate::selectors`):
//! diagnostics, completion (also of tag values in dbt_project.yml and
//! properties files), hover and "N nodes match" lenses, and the
//! `dbt.select` answer.

mod command;
mod completion;
mod diagnostics;
mod line_context;
mod presentation;

pub use command::{
    select_command, select_nodes, split_exclude, vocabulary, SelectAnswer, SelectError,
    SelectRequest, SelectedNode, Vocabulary,
};
pub use completion::{selector_completion, selector_completion_applies};
pub use diagnostics::{selectors_diagnostics, unmatched_selector_diagnostics};
pub use presentation::{selectors_hover, selectors_lenses};

/// Whether a uri is a project's selectors.yml.
pub fn is_selectors_file(uri: &str) -> bool {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    std::path::Path::new(&path)
        .file_name()
        .is_some_and(|f| f == "selectors.yml" || f == "selectors.yaml")
}

#[cfg(test)]
mod tests {
    #[test]
    fn selectors_files_are_recognised() {
        for (uri, expected) in [
            ("file:///p/selectors.yml", true),
            ("file:///p/selectors.yaml", true),
            ("file:///p/models/schema.yml", false),
            ("file:///p/my_selectors.yml", false),
        ] {
            assert_eq!(super::is_selectors_file(uri), expected, "{uri}");
        }
    }
}

//! The one `unsafe` in the project (06 §3.2): C-ABI binding to the vendored
//! tree-sitter-jinja2 parser compiled by build.rs.

use tree_sitter_language::LanguageFn;

unsafe extern "C" {
    fn tree_sitter_jinja2() -> *const ();
}

/// SAFETY: `tree_sitter_jinja2` is the generated entry point of the vendored
/// grammar (third_party/tree-sitter-jinja2, ABI 14) linked by build.rs.
pub const LANGUAGE: LanguageFn = unsafe { LanguageFn::from_raw(tree_sitter_jinja2) };

pub fn language() -> tree_sitter::Language {
    tree_sitter::Language::new(LANGUAGE)
}

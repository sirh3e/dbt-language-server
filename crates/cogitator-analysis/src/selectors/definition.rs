//! What a selectors.yml file is read into: definitions as set
//! expressions over criteria, and the problems of how they are written.

use cogitator_lsp::Range;

use super::eval::Indirect;
use super::syntax::Criterion;

/// A definition, as a set expression.
#[derive(Debug, Clone)]
pub enum Expr {
    /// One criterion, from a mapping or from a part of a string.
    Criterion {
        criterion: Criterion,
        indirect: Option<Indirect>,
        /// Where the method is written (a string criterion: its text).
        method_range: Range,
        /// Where the value is written.
        value_range: Range,
    },
    Union(Vec<Expr>),
    Intersection(Vec<Expr>),
    Difference(Box<Expr>, Box<Expr>),
    /// What could not be read; its problem is in the file's list.
    Invalid,
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub description: String,
    pub default: bool,
    /// The `name:` value.
    pub range: Range,
    pub expr: Expr,
}

/// Something wrong with how a definition is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub range: Range,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct SelectorsFile {
    pub definitions: Vec<Definition>,
    pub problems: Vec<Problem>,
}

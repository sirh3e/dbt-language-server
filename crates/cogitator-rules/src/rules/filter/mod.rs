//! Filter rules: predicates and how a backend can use them to prune.

pub mod non_sargable;
mod predicate;
mod range;
pub(crate) mod support;

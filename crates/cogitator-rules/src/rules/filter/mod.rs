//! Filter rules: predicates and how a backend can use them to prune.

pub mod cluster_order;
pub mod constant_false;
pub mod non_sargable;
pub mod partition_or;
mod predicate;
mod range;
pub(crate) mod support;

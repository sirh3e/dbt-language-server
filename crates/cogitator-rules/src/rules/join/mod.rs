//! `join.*` rules.

pub mod cartesian;
pub mod distinct_fanout;
pub mod left_nullified;

mod left_fix;
mod null_reject;
pub(crate) mod support;
pub(crate) mod text;

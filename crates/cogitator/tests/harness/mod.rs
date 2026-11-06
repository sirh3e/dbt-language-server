//! The deterministic e2e harness (05 §7.1): a deadline-bounded framed
//! LSP client over the compiled binary, per-test fixture copies with a
//! sandboxed home, and the scripted fake dbt.
#![allow(dead_code)] // each test crate consumes a subset

pub mod client;
pub mod fake;
pub mod fixture;
pub mod wire;

#[allow(unused_imports)]
pub use client::{default_deadline, Lsp, ServerBuilder};
#[allow(unused_imports)]
pub use fixture::{boot, copy_project, fixture_root, jaffle, write_config, TestProject};

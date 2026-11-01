//! LSP protocol structs and base-protocol framing (specs/rust-port/01).
//!
//! Serialization matches the Go server byte-for-byte modulo key order: every
//! field is always present (no `skip_serializing_if`), ids are numbers or
//! null, hover contents is a bare string.

pub mod rpc;
pub mod types;

pub use types::*;

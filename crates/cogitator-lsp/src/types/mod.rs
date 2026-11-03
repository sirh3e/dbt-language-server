//! Protocol structs (specs/rust-port/01 §2–§3), split by direction:
//! core geometry, message envelopes, incoming params, outgoing results,
//! and the initialize response.

mod actions;
mod core;
mod diagnostics;
mod init;
mod inlay;
mod linked_editing;
mod links;
mod messages;
mod params;
mod results;
mod workspace;
mod workspace_diag;

pub use actions::*;
pub use core::*;
pub use diagnostics::*;
pub use init::*;
pub use inlay::*;
pub use linked_editing::*;
pub use links::*;
pub use messages::*;
pub use params::*;
pub use results::*;
pub use workspace::*;
pub use workspace_diag::*;

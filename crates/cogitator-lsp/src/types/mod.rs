//! Protocol structs (specs/rust-port/01 §2–§3), split by direction:
//! core geometry, message envelopes, incoming params, outgoing results,
//! and the initialize response.

mod core;
mod init;
mod messages;
mod params;
mod results;
mod workspace;

pub use core::*;
pub use init::*;
pub use messages::*;
pub use params::*;
pub use results::*;
pub use workspace::*;

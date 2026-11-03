//! The deterministic slow-request seam for tests (documented in spec
//! 05 §6.2): `COGITATOR_E2E_DELAY="<name>=<millis>"` makes the offload
//! shell sleep before running the named handler. Env-gated so plain
//! `cargo test` binaries carry it; parsed once; inert when unset.

use std::sync::OnceLock;
use std::time::Duration;

static DELAY: OnceLock<Option<(String, u64)>> = OnceLock::new();

/// The configured artificial delay for `name`, if any.
pub fn delay_for(name: &str) -> Option<Duration> {
    let parsed = DELAY.get_or_init(|| {
        let spec = std::env::var("COGITATOR_E2E_DELAY").ok()?;
        let (target, millis) = spec.split_once('=')?;
        Some((target.to_string(), millis.trim().parse().ok()?))
    });
    match parsed {
        Some((target, millis)) if target == name => Some(Duration::from_millis(*millis)),
        _ => None,
    }
}

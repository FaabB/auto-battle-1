//! Development tools — only included with `cargo run --features dev`.
//!
//! Debug overlays, test spawners, and inspector setup go here.
//! This module is stripped from release builds.

use bevy::prelude::*;

#[allow(clippy::missing_const_for_fn)]
pub(super) fn plugin(_app: &mut App) {
    // Future: ticket 4 adds debug enemy spawner here
    // Future: inspector, state logging, performance overlay
}

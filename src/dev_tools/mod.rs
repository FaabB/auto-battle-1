//! Development tools — only included with `cargo run --features dev`.
//!
//! Debug overlays, inspector setup, and diagnostic tools go here.
//! This module is stripped from release builds.

use bevy::prelude::*;

#[allow(clippy::missing_const_for_fn)]
pub fn plugin(_app: &mut App) {
    // Future dev tools go here.
}

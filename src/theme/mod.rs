//! Shared UI theme: color palette and reusable widget constructors.

pub(crate) mod palette;
pub(crate) mod widget;

#[allow(clippy::missing_const_for_fn)]
pub(super) fn plugin(_app: &mut bevy::prelude::App) {
    // No runtime setup needed yet.
    // Future: register interaction systems, theme resources.
}

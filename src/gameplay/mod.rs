//! Gameplay domain plugins: battlefield, buildings, and (future) units, combat, economy, waves.

pub(crate) mod battlefield;
pub(crate) mod building;
pub(crate) mod units;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((battlefield::plugin, building::plugin, units::plugin));
}

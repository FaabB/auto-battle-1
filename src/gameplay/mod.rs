//! Gameplay domain plugins: battlefield, buildings, units, combat, and (future) economy, waves.

pub(crate) mod battlefield;
pub(crate) mod building;
pub(crate) mod combat;
pub(crate) mod units;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((
        battlefield::plugin,
        building::plugin,
        combat::plugin,
        units::plugin,
    ));
}

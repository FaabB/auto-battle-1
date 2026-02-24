//! In-game HUD: bottom bar with gold, cards, reroll, elapsed time, minimap.

pub mod bottom_bar;
mod elapsed_time;

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((bottom_bar::plugin, elapsed_time::plugin));
}

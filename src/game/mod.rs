//! Core game plugin and state management.

use bevy::prelude::*;

/// Main game plugin that sets up core systems and states.
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<crate::GameState>();
    }
}

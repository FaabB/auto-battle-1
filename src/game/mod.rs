use bevy::prelude::*;

/// Core game plugin that sets up states and the global camera.
#[derive(Debug)]
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<crate::GameState>()
            .add_sub_state::<crate::InGameState>()
            .add_systems(Startup, setup_camera);
    }
}

/// Spawns the global 2D camera. Persists across all states (do NOT add `DespawnOnExit`).
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

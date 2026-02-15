//! The UI camera is a 2D camera that renders all UI elements.
//! It persists across all states so UI is always visible, even during
//! non-gameplay screens such as the main menu.

use bevy::prelude::*;

pub fn plugin(app: &mut App) {
    app.register_type::<UiCamera>();
    app.add_systems(Startup, spawn_ui_camera);
}

/// Marker for the global UI camera. Persists across all states.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct UiCamera;

fn spawn_ui_camera(mut commands: Commands) {
    commands.spawn((Name::new("UI Camera"), UiCamera, Camera2d));
}

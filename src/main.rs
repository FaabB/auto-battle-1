//! Auto-battle game entry point.

use auto_battle::game::GamePlugin;
use auto_battle::prelude::*;
use auto_battle::screens::{InGamePlugin, LoadingScreenPlugin, MainMenuPlugin, PausedPlugin};

fn main() {
    App::new()
        // Bevy default plugins with pixel-art configuration
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Auto Battle".to_string(),
                        resolution: (1920, 1080).into(),
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()), // Pixel-perfect rendering
        )
        // Game plugins
        .add_plugins((
            GamePlugin,
            LoadingScreenPlugin,
            MainMenuPlugin,
            InGamePlugin,
            PausedPlugin,
        ))
        // Startup systems
        .add_systems(Startup, setup_camera)
        .run();
}

/// Sets up the 2D camera.
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}

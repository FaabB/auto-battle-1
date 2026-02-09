//! Testing utilities for Bevy systems.

use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::state::state::FreelyMutableState;
use bevy::window::WindowPlugin;

/// Creates a minimal app for testing with essential plugins.
pub fn create_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app
}

/// Creates a test app with state support.
#[allow(dead_code)]
pub fn create_test_app_with_state<S: FreelyMutableState + Default>() -> App {
    let mut app = create_test_app();
    app.init_state::<S>();
    app
}

/// Creates a test app with both `GameState` and `InGameState` initialized,
/// already transitioned to `InGame`/`Playing` for testing gameplay systems.
#[allow(dead_code)]
pub fn create_ingame_test_app() -> App {
    let mut app = create_test_app();
    app.add_plugins(StatesPlugin);
    app.add_plugins(InputPlugin);
    app.add_plugins(WindowPlugin::default());
    app.init_state::<crate::GameState>();
    app.add_sub_state::<crate::InGameState>();
    // Spawn a camera so systems that query Camera2d work
    app.world_mut().spawn(Camera2d);
    // Transition to InGame so SubState is active
    app.world_mut()
        .resource_mut::<NextState<crate::GameState>>()
        .set(crate::GameState::InGame);
    app.update(); // Apply the transition
    app
}

/// Helper to advance the app by one frame.
pub fn tick(app: &mut App) {
    app.update();
}

/// Helper to advance the app by multiple frames.
#[allow(dead_code)]
pub fn tick_multiple(app: &mut App, count: usize) {
    for _ in 0..count {
        app.update();
    }
}

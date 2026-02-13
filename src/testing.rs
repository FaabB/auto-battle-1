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

/// Creates a base test app with states, input, window, and camera.
///
/// Does NOT transition to `InGame` — add your domain plugins first, then
/// call [`transition_to_ingame`] to trigger `OnEnter` systems.
#[allow(dead_code)]
pub fn create_base_test_app() -> App {
    let mut app = create_test_app();
    app.add_plugins(StatesPlugin);
    app.add_plugins(InputPlugin);
    app.add_plugins(WindowPlugin::default());
    app.init_state::<crate::GameState>();
    app.add_sub_state::<crate::InGameState>();
    app.world_mut().spawn(Camera2d);
    app
}

/// Same as [`create_base_test_app`] but without `InputPlugin`.
///
/// Use when testing systems that read `ButtonInput` and you need `press()`
/// to persist through to `Update` (since `InputPlugin` clears `just_pressed`
/// in `PreUpdate`). Manually `init_resource::<ButtonInput<MouseButton>>()` etc.
#[allow(dead_code)]
pub fn create_base_test_app_no_input() -> App {
    let mut app = create_test_app();
    app.add_plugins(StatesPlugin);
    app.add_plugins(WindowPlugin::default());
    app.init_state::<crate::GameState>();
    app.add_sub_state::<crate::InGameState>();
    app.world_mut().spawn(Camera2d);
    app
}

/// Transitions the app to `GameState::InGame` and runs two updates
/// (first applies the transition + `OnEnter`, second applies deferred commands).
pub fn transition_to_ingame(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<crate::GameState>>()
        .set(crate::GameState::InGame);
    app.update();
    app.update();
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

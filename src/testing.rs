//! Testing utilities for Bevy systems.

#![cfg(test)]

use bevy::prelude::*;
use bevy::state::state::FreelyMutableState;

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

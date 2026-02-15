//! Tests for game state transitions.

use auto_battle::GameState;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use pretty_assertions::assert_eq;

fn create_game_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(StatesPlugin);
    app.add_plugins(InputPlugin);
    app.add_plugins(auto_battle::plugin);
    app
}

#[test]
fn game_initializes_in_loading_state() {
    let app = create_game_app();
    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::Loading);
}

#[test]
fn can_transition_between_states() {
    let mut app = create_game_app();

    // Transition to MainMenu
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::MainMenu);
    app.update();

    let state = app.world().resource::<State<GameState>>();
    assert_eq!(*state.get(), GameState::MainMenu);
}

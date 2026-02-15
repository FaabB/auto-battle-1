//! Screen plugins and state management.

mod in_game;
mod loading;
mod main_menu;

use bevy::prelude::*;

/// Primary game states.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[states(scoped_entities)]
pub enum GameState {
    /// Initial loading state.
    #[default]
    Loading,
    /// Main menu state.
    MainMenu,
    /// Active gameplay state.
    InGame,
}

pub fn plugin(app: &mut App) {
    app.init_state::<GameState>();
    app.add_plugins((loading::plugin, main_menu::plugin, in_game::plugin));
}

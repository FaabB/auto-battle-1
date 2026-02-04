//! Auto-battle game library.

pub mod components;
pub mod game;
pub mod prelude;
pub mod resources;
pub mod screens;
pub mod systems;
#[cfg(test)]
pub mod testing;
pub mod ui;

use bevy::prelude::*;

/// Primary game states.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    /// Initial loading state.
    #[default]
    Loading,
    /// Main menu state.
    MainMenu,
    /// Active gameplay state.
    InGame,
    /// Paused state (overlay on `InGame`).
    Paused,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn game_state_default_is_loading() {
        assert_eq!(GameState::default(), GameState::Loading);
    }

    #[test]
    fn game_states_are_distinct() {
        assert_ne!(GameState::Loading, GameState::MainMenu);
        assert_ne!(GameState::MainMenu, GameState::InGame);
        assert_ne!(GameState::InGame, GameState::Paused);
    }
}

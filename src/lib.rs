//! Auto-battle game library.

pub mod battlefield;
pub mod game;
pub mod prelude;
pub mod screens;
#[cfg(test)]
pub mod testing;

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
}

/// Sub-states within `InGame`. Only exists while `GameState::InGame` is active.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(GameState = GameState::InGame)]
pub enum InGameState {
    /// Normal gameplay.
    #[default]
    Playing,
    /// Game is paused (overlay on gameplay).
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
    }

    #[test]
    fn in_game_state_default_is_playing() {
        assert_eq!(InGameState::default(), InGameState::Playing);
    }

    #[test]
    fn in_game_states_are_distinct() {
        assert_ne!(InGameState::Playing, InGameState::Paused);
    }
}

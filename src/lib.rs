//! Auto-battle game library.

pub mod battlefield;
pub mod building;
pub mod game;
pub mod prelude;
pub mod screens;
#[cfg(test)]
pub mod testing;

use bevy::prelude::*;

// === Z-Layer Constants ===
// Cross-cutting sprite ordering used by multiple domain plugins.

/// Background layer (behind everything).
pub const Z_BACKGROUND: f32 = -1.0;
/// Zone sprites (fortresses, build zone, combat zone).
pub const Z_ZONE: f32 = 0.0;
/// Grid cell sprites in the build zone.
pub const Z_GRID: f32 = 1.0;
/// Grid cursor / hover highlight.
pub const Z_GRID_CURSOR: f32 = 2.0;
/// Placed buildings.
pub const Z_BUILDING: f32 = 3.0;
/// Units (future: Ticket 3).
pub const Z_UNIT: f32 = 4.0;
/// Health bars (future: Ticket 5).
pub const Z_HEALTH_BAR: f32 = 5.0;

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

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn z_layers_are_ordered() {
        assert!(Z_BACKGROUND < Z_ZONE);
        assert!(Z_ZONE < Z_GRID);
        assert!(Z_GRID < Z_GRID_CURSOR);
        assert!(Z_GRID_CURSOR < Z_BUILDING);
        assert!(Z_BUILDING < Z_UNIT);
        assert!(Z_UNIT < Z_HEALTH_BAR);
    }
}

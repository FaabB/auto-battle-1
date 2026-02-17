//! Auto-battle game library.

#[cfg(feature = "dev")]
pub(crate) mod dev_tools;
pub(crate) mod gameplay;
pub(crate) mod menus;
pub(crate) mod screens;
#[cfg(test)]
pub(crate) mod testing;
pub(crate) mod theme;
pub(crate) mod third_party;
pub(crate) mod ui_camera;

// Re-export types needed by integration tests in `tests/`.
pub use screens::GameState;

use bevy::prelude::*;

// === Z-Layer Constants ===
// Cross-cutting sprite ordering used by multiple domain plugins.

/// Background layer (behind everything).
pub(crate) const Z_BACKGROUND: f32 = -1.0;
/// Zone backdrop sprites (fortress zones, build zone, combat zone).
pub(crate) const Z_ZONE: f32 = 0.0;
/// Fortress entities (above zone backdrops).
pub(crate) const Z_FORTRESS: f32 = 0.5;
/// Grid cell sprites in the build zone.
pub(crate) const Z_GRID: f32 = 1.0;
/// Grid cursor / hover highlight.
pub(crate) const Z_GRID_CURSOR: f32 = 2.0;
/// Placed buildings.
pub(crate) const Z_BUILDING: f32 = 3.0;
/// Units (Ticket 3).
pub(crate) const Z_UNIT: f32 = 4.0;

// === Global System Ordering ===
// Domain plugins register their Update systems in the appropriate set.
// Sets are chained so they run in order every frame.

/// Global system sets for the Update schedule.
/// Domain plugins use `.in_set(GameSet::Xxx)` to slot into the correct phase.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GameSet {
    /// Input handling: camera pan, building placement, UI interaction.
    Input,
    /// Building production: barracks spawn timers, unit creation.
    Production,
    /// AI: target finding, decision making.
    Ai,
    /// Movement: units moving toward targets.
    Movement,
    /// Combat: attack timers, damage application.
    Combat,
    /// Death: despawn dead entities, cleanup.
    Death,
    /// UI: health bars, gold display, wave counter.
    Ui,
}

/// Run condition: true when gameplay is active (`InGame` state, no menu open).
/// Use with `.run_if(gameplay_running)` on gameplay systems.
pub(crate) fn gameplay_running(
    game_state: Res<State<screens::GameState>>,
    menu: Res<State<menus::Menu>>,
) -> bool {
    game_state.get() == &screens::GameState::InGame && menu.get() == &menus::Menu::None
}

/// Composes all game plugins. Call from `main.rs`.
pub fn plugin(app: &mut App) {
    // Global system ordering
    app.configure_sets(
        Update,
        (
            GameSet::Input,
            GameSet::Production,
            GameSet::Ai,
            GameSet::Movement,
            GameSet::Combat,
            GameSet::Death,
            GameSet::Ui,
        )
            .chain(),
    );

    app.add_plugins((
        third_party::plugin,
        ui_camera::plugin,
        screens::plugin,
        menus::plugin,
        gameplay::plugin,
        theme::plugin,
    ));

    #[cfg(feature = "dev")]
    app.add_plugins(dev_tools::plugin);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    use crate::menus::Menu;
    use crate::screens::GameState;

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
    fn menu_default_is_none() {
        assert_eq!(Menu::default(), Menu::None);
    }

    #[test]
    fn menu_states_are_distinct() {
        assert_ne!(Menu::None, Menu::Main);
        assert_ne!(Menu::Main, Menu::Pause);
        assert_ne!(Menu::Pause, Menu::Victory);
        assert_ne!(Menu::Victory, Menu::Defeat);
    }

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn z_layers_are_ordered() {
        assert!(Z_BACKGROUND < Z_ZONE);
        assert!(Z_ZONE < Z_GRID);
        assert!(Z_GRID < Z_GRID_CURSOR);
        assert!(Z_GRID_CURSOR < Z_BUILDING);
        assert!(Z_BUILDING < Z_UNIT);
    }
}

//! Menu overlays that can appear on top of any screen.
//!
//! The `Menu` state is orthogonal to `GameState` — menus are overlays,
//! not screens. For example, `Menu::Pause` appears while `GameState::InGame`
//! is active, and `Menu::Main` appears while `GameState::MainMenu` is active.

mod endgame;
mod main_menu;
mod pause;

use bevy::prelude::*;

/// Menu overlay states. Orthogonal to `GameState`.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[states(scoped_entities)]
pub enum Menu {
    /// No menu overlay is active.
    #[default]
    None,
    /// Main menu (shown on the title/main-menu screen).
    Main,
    /// Pause menu (shown in-game).
    Pause,
    /// Victory overlay (enemy fortress destroyed).
    Victory,
    /// Defeat overlay (player fortress destroyed).
    Defeat,
}

pub fn plugin(app: &mut App) {
    app.init_state::<Menu>();
    app.add_plugins((main_menu::plugin, pause::plugin, endgame::plugin));
}

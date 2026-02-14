//! In-game screen plugin: opens the pause menu on ESC.
//!
//! Gameplay visuals and logic are handled by domain plugins
//! (e.g., battlefield, building). Pause UI lives in `menus::pause`.

use bevy::prelude::*;

use super::GameState;
use crate::menus::Menu;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        open_pause_menu.run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
    );
}

fn open_pause_menu(keyboard: Res<ButtonInput<KeyCode>>, mut next_menu: ResMut<NextState<Menu>>) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_menu.set(Menu::Pause);
    }
}

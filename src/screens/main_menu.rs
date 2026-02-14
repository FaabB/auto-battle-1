//! Main menu screen — opens the main menu overlay.

use bevy::prelude::*;

use super::GameState;
use crate::menus::Menu;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameState::MainMenu), open_main_menu);
}

fn open_main_menu(mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Main);
}

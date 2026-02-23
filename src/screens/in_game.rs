//! In-game screen plugin: opens the pause menu on ESC.
//!
//! Gameplay visuals and logic are handled by domain plugins
//! (e.g., battlefield, building). Pause UI lives in `menus::pause`.

use bevy::prelude::*;

use crate::menus::Menu;
use crate::{GameSet, gameplay_running};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        open_pause_menu
            .in_set(GameSet::Input)
            .run_if(gameplay_running),
    );
}

fn open_pause_menu(keyboard: Res<ButtonInput<KeyCode>>, mut next_menu: ResMut<NextState<Menu>>) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_menu.set(Menu::Pause);
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use crate::menus::Menu;

    #[test]
    fn escape_opens_pause_menu() {
        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        app.add_systems(Update, super::open_pause_menu);
        crate::testing::transition_to_ingame(&mut app);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        let next = app.world().resource::<NextState<Menu>>();
        assert!(
            matches!(*next, NextState::Pending(Menu::Pause)),
            "Expected NextState to be Menu::Pause, got {next:?}"
        );
    }
}

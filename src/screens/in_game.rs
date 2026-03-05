//! In-game screen plugin: handles ESC for pause toggle, endgame exit.
//!
//! Gameplay visuals and logic are handled by domain plugins
//! (e.g., battlefield, building). Pause UI lives in `menus::pause`.

use bevy::prelude::*;

use crate::GameSet;
use crate::menus::Menu;
use crate::screens::GameState;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        handle_escape
            .in_set(GameSet::Input)
            .run_if(in_state(GameState::InGame)),
    );
}

fn handle_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    menu: Res<State<Menu>>,
    mut next_menu: ResMut<NextState<Menu>>,
    mut next_game: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        match menu.get() {
            Menu::None => next_menu.set(Menu::Pause),
            Menu::Pause => next_menu.set(Menu::None),
            Menu::Victory | Menu::Defeat => next_game.set(GameState::MainMenu),
            Menu::Main => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use crate::menus::Menu;
    use crate::screens::GameState;

    fn create_escape_test_app() -> App {
        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        app.add_systems(Update, super::handle_escape);
        crate::testing::transition_to_ingame(&mut app);
        app
    }

    fn press_escape(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
    }

    #[test]
    fn escape_opens_pause_menu() {
        let mut app = create_escape_test_app();
        press_escape(&mut app);

        let next = app.world().resource::<NextState<Menu>>();
        assert!(
            matches!(*next, NextState::Pending(Menu::Pause)),
            "Expected NextState to be Menu::Pause, got {next:?}"
        );
    }

    #[test]
    fn escape_closes_pause_menu() {
        let mut app = create_escape_test_app();

        // Open pause
        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::Pause);
        app.update();

        press_escape(&mut app);

        let next = app.world().resource::<NextState<Menu>>();
        assert!(
            matches!(*next, NextState::Pending(Menu::None)),
            "Expected NextState to be Menu::None, got {next:?}"
        );
    }

    #[test]
    fn escape_exits_victory_to_main_menu() {
        let mut app = create_escape_test_app();

        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::Victory);
        app.update();

        press_escape(&mut app);

        let next = app.world().resource::<NextState<GameState>>();
        assert!(
            matches!(*next, NextState::Pending(GameState::MainMenu)),
            "Expected NextState to be GameState::MainMenu, got {next:?}"
        );
    }

    #[test]
    fn escape_exits_defeat_to_main_menu() {
        let mut app = create_escape_test_app();

        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::Defeat);
        app.update();

        press_escape(&mut app);

        let next = app.world().resource::<NextState<GameState>>();
        assert!(
            matches!(*next, NextState::Pending(GameState::MainMenu)),
            "Expected NextState to be GameState::MainMenu, got {next:?}"
        );
    }
}

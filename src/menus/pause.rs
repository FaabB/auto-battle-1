//! Pause menu UI: overlay, pause text, and input handling (unpause / quit).

use bevy::prelude::*;

use super::Menu;
use crate::screens::GameState;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Pause), spawn_pause_menu);
    app.add_systems(Update, handle_pause_input.run_if(in_state(Menu::Pause)));
}

fn spawn_pause_menu(mut commands: Commands) {
    // Semi-transparent overlay
    commands.spawn((crate::theme::widget::overlay(), DespawnOnExit(Menu::Pause)));

    // Pause text
    commands.spawn((
        crate::theme::widget::header("PAUSED"),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(40.0),
            ..default()
        },
        DespawnOnExit(Menu::Pause),
    ));

    // Resume prompt (24px — smaller than default label)
    commands.spawn((
        Text::new("Press ESC to Resume | Q to Quit"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(crate::theme::palette::BODY_TEXT),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(55.0),
            ..default()
        },
        DespawnOnExit(Menu::Pause),
    ));
}

fn handle_pause_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_menu: ResMut<NextState<Menu>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_menu.set(Menu::None);
    }
    if keyboard.just_pressed(KeyCode::KeyQ) {
        next_game_state.set(GameState::MainMenu);
        // Menu::Main will be set by the MainMenu screen's OnEnter system.
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use crate::menus::Menu;
    use crate::screens::GameState;

    #[test]
    fn escape_unpauses() {
        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        app.add_systems(Update, super::handle_pause_input);
        crate::testing::transition_to_ingame(&mut app);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        let next_menu = app.world().resource::<NextState<Menu>>();
        assert!(
            matches!(*next_menu, NextState::Pending(Menu::None)),
            "Expected NextState<Menu>::None, got {next_menu:?}"
        );
    }

    #[test]
    fn q_quits_to_main_menu() {
        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        app.add_systems(Update, super::handle_pause_input);
        crate::testing::transition_to_ingame(&mut app);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update();

        let next_state = app.world().resource::<NextState<GameState>>();
        assert!(
            matches!(*next_state, NextState::Pending(GameState::MainMenu)),
            "Expected NextState<GameState>::MainMenu, got {next_state:?}"
        );
    }
}

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

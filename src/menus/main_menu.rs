//! Main menu UI: title text, start prompt, and input handling.

use bevy::prelude::*;

use super::Menu;
use crate::screens::GameState;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Main), spawn_main_menu);
    app.add_systems(
        Update,
        handle_main_menu_input.run_if(in_state(Menu::Main)),
    );
}

fn spawn_main_menu(mut commands: Commands) {
    // Title (72px — larger than default header)
    commands.spawn((
        Text::new("Auto Battle"),
        TextFont {
            font_size: 72.0,
            ..default()
        },
        TextColor(crate::theme::palette::HEADER_TEXT),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(30.0),
            ..default()
        },
        DespawnOnExit(Menu::Main),
    ));

    // Start prompt
    commands.spawn((
        crate::theme::widget::label("Press SPACE to Start"),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(60.0),
            ..default()
        },
        DespawnOnExit(Menu::Main),
    ));
}

fn handle_main_menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_game_state: ResMut<NextState<GameState>>,
    mut next_menu: ResMut<NextState<Menu>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        next_game_state.set(GameState::InGame);
        next_menu.set(Menu::None);
    }
}

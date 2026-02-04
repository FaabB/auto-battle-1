//! Main menu plugin.

use bevy::prelude::*;

use crate::GameState;
use crate::components::CleanupMainMenu;
use crate::systems::cleanup_entities;

/// Plugin for the main menu.
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
            .add_systems(
                Update,
                handle_main_menu_input.run_if(in_state(GameState::MainMenu)),
            )
            .add_systems(
                OnExit(GameState::MainMenu),
                cleanup_entities::<CleanupMainMenu>,
            );
    }
}

fn setup_main_menu(mut commands: Commands) {
    // Title
    commands.spawn((
        Text::new("Auto Battle"),
        TextFont {
            font_size: 72.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(30.0),
            ..default()
        },
        CleanupMainMenu,
    ));

    // Start prompt
    commands.spawn((
        Text::new("Press SPACE to Start"),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(60.0),
            ..default()
        },
        CleanupMainMenu,
    ));
}

fn handle_main_menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        next_state.set(GameState::InGame);
    }
}

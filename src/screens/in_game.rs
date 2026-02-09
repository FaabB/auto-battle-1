//! In-game screen plugin: pause/unpause input, pause overlay UI, quit to menu.
//!
//! Gameplay visuals and logic are handled by domain plugins
//! (e.g., `BattlefieldPlugin`). This plugin owns the pause overlay
//! and keybindings that operate across all `InGameState` sub-states.

use bevy::prelude::*;

use crate::{GameState, InGameState};

#[derive(Debug)]
pub struct InGamePlugin;

impl Plugin for InGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(InGameState::Paused), setup_pause_menu)
            .add_systems(
                Update,
                handle_game_input.run_if(in_state(GameState::InGame)),
            );
    }
}

fn handle_game_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<InGameState>>,
    mut next_ingame_state: ResMut<NextState<InGameState>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    match current_state.get() {
        InGameState::Playing => {
            if keyboard.just_pressed(KeyCode::Escape) {
                next_ingame_state.set(InGameState::Paused);
            }
        }
        InGameState::Paused => {
            if keyboard.just_pressed(KeyCode::Escape) {
                next_ingame_state.set(InGameState::Playing);
            }
            if keyboard.just_pressed(KeyCode::KeyQ) {
                next_game_state.set(GameState::MainMenu);
            }
        }
    }
}

fn setup_pause_menu(mut commands: Commands) {
    // Semi-transparent overlay
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        DespawnOnExit(InGameState::Paused),
    ));

    // Pause text
    commands.spawn((
        Text::new("PAUSED"),
        TextFont {
            font_size: 64.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(40.0),
            ..default()
        },
        DespawnOnExit(InGameState::Paused),
    ));

    // Resume prompt
    commands.spawn((
        Text::new("Press ESC to Resume | Q to Quit"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(55.0),
            ..default()
        },
        DespawnOnExit(InGameState::Paused),
    ));
}

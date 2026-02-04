//! In-game plugin.

use bevy::prelude::*;

use crate::GameState;
use crate::components::CleanupInGame;
use crate::systems::cleanup_entities;

/// Plugin for the main gameplay.
pub struct InGamePlugin;

impl Plugin for InGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), setup_game)
            .add_systems(
                Update,
                handle_game_input.run_if(in_state(GameState::InGame)),
            )
            .add_systems(OnExit(GameState::InGame), cleanup_entities::<CleanupInGame>);
    }
}

fn setup_game(mut commands: Commands) {
    commands.spawn((
        Text::new("Game Running - Press ESC to Pause"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(10.0),
            ..default()
        },
        CleanupInGame,
    ));
}

fn handle_game_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_state.set(GameState::Paused);
    }
}

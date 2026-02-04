//! Loading screen plugin.

use bevy::prelude::*;

use crate::GameState;
use crate::components::CleanupLoading;
use crate::systems::cleanup_entities;

/// Plugin for the loading screen.
pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Loading), setup_loading_screen)
            .add_systems(
                Update,
                check_loading_complete.run_if(in_state(GameState::Loading)),
            )
            .add_systems(
                OnExit(GameState::Loading),
                cleanup_entities::<CleanupLoading>,
            );
    }
}

fn setup_loading_screen(mut commands: Commands) {
    commands.spawn((
        Text::new("Loading..."),
        TextFont {
            font_size: 48.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            ..default()
        },
        CleanupLoading,
    ));
}

fn check_loading_complete(mut next_state: ResMut<NextState<GameState>>) {
    // For now, immediately transition to main menu
    // In the future, this will wait for assets to load
    next_state.set(GameState::MainMenu);
}

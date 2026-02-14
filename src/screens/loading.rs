//! Loading screen plugin.

use bevy::prelude::*;

use super::GameState;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameState::Loading), setup_loading_screen)
        .add_systems(
            Update,
            check_loading_complete.run_if(in_state(GameState::Loading)),
        );
}

fn setup_loading_screen(mut commands: Commands) {
    commands.spawn((
        crate::theme::widget::header("Loading..."),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            ..default()
        },
        DespawnOnExit(GameState::Loading),
    ));
}

fn check_loading_complete(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::MainMenu);
}

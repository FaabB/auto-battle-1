//! Victory/Defeat overlay UI and input handling.

use bevy::prelude::*;

use super::Menu;
use crate::screens::GameState;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Victory), spawn_victory_screen);
    app.add_systems(OnEnter(Menu::Defeat), spawn_defeat_screen);
    app.add_systems(
        Update,
        handle_endgame_input.run_if(in_state(Menu::Victory).or(in_state(Menu::Defeat))),
    );
}

fn spawn_victory_screen(mut commands: Commands) {
    spawn_endgame_overlay(&mut commands, "VICTORY!", Menu::Victory);
}

fn spawn_defeat_screen(mut commands: Commands) {
    spawn_endgame_overlay(&mut commands, "DEFEAT", Menu::Defeat);
}

/// Shared overlay spawning for both victory and defeat screens.
/// Uses the same pattern as pause menu: overlay + header + prompt.
fn spawn_endgame_overlay(commands: &mut Commands, title: &str, menu: Menu) {
    // Semi-transparent overlay
    commands.spawn((crate::theme::widget::overlay(), DespawnOnExit(menu)));

    // Result text (64px header)
    commands.spawn((
        crate::theme::widget::header(title),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(40.0),
            ..default()
        },
        DespawnOnExit(menu),
    ));

    // Action prompt (24px)
    commands.spawn((
        Text::new("Press Q to Continue"),
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
        DespawnOnExit(menu),
    ));
}

fn handle_endgame_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyQ) {
        next_game_state.set(GameState::MainMenu);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::assert_entity_count;
    use bevy::state::app::StatesPlugin;

    /// Creates a test app that transitions to InGame then to the given Menu overlay.
    fn create_overlay_test_app(menu: Menu) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.init_state::<Menu>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_plugins(plugin);
        // Transition to InGame first
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update(); // Apply GameState transition
        // Now transition to the target menu
        app.world_mut().resource_mut::<NextState<Menu>>().set(menu);
        app.update(); // Apply Menu transition → triggers OnEnter
        app.update(); // Apply deferred commands
        app
    }

    #[test]
    fn victory_screen_spawns_overlay_and_text() {
        let mut app = create_overlay_test_app(Menu::Victory);

        // 3 entities: overlay + header text + prompt text
        assert_entity_count::<With<Text>>(&mut app, 2); // header + prompt
        assert_entity_count::<With<DespawnOnExit<Menu>>>(&mut app, 3); // all 3 entities
    }

    #[test]
    fn defeat_screen_spawns_overlay_and_text() {
        let mut app = create_overlay_test_app(Menu::Defeat);

        assert_entity_count::<With<Text>>(&mut app, 2);
        assert_entity_count::<With<DespawnOnExit<Menu>>>(&mut app, 3);
    }

    #[test]
    fn handle_endgame_input_returns_to_menu_on_q() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.init_state::<Menu>();
        // Skip InputPlugin to keep just_pressed persistent
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, handle_endgame_input);
        // Transition to InGame
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();

        // Simulate Q press
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update();

        let next_state = app.world().resource::<NextState<GameState>>();
        assert!(
            matches!(*next_state, NextState::Pending(GameState::MainMenu)),
            "Expected MainMenu transition, got {next_state:?}",
        );
    }
}

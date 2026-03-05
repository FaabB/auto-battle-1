//! Victory/Defeat overlay UI with bordered panel and clickable buttons.

use bevy::prelude::*;

use super::Menu;
use crate::screens::GameState;
use crate::theme::palette;
use crate::theme::widget::{self, Activate};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Victory), spawn_victory_screen);
    app.add_systems(OnEnter(Menu::Defeat), spawn_defeat_screen);
    app.add_systems(
        Update,
        close_endgame_on_escape.run_if(in_state(Menu::Victory).or(in_state(Menu::Defeat))),
    );
}

fn close_endgame_on_escape(
    input: Res<ButtonInput<KeyCode>>,
    mut next_game: ResMut<NextState<GameState>>,
) {
    if input.just_pressed(KeyCode::Escape) {
        next_game.set(GameState::MainMenu);
    }
}

fn spawn_victory_screen(mut commands: Commands) {
    spawn_endgame_overlay(
        &mut commands,
        "VICTORY!",
        palette::HEALTH_BAR_FILL,
        Menu::Victory,
    );
}

fn spawn_defeat_screen(mut commands: Commands) {
    spawn_endgame_overlay(
        &mut commands,
        "DEFEAT",
        palette::ENEMY_FORTRESS,
        Menu::Defeat,
    );
}

/// Shared overlay spawning for both victory and defeat screens.
fn spawn_endgame_overlay(commands: &mut Commands, title: &str, title_color: Color, menu: Menu) {
    commands.spawn((
        widget::ui_root("Endgame Screen"),
        BackgroundColor(palette::OVERLAY_BACKGROUND),
        GlobalZIndex(1),
        DespawnOnExit(menu),
        children![
            // Bordered panel
            (
                Name::new("Endgame Panel"),
                Node {
                    width: Val::Px(500.0),
                    min_height: Val::Px(300.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceEvenly,
                    padding: UiRect::all(Val::Px(40.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(palette::PANEL_BACKGROUND),
                BorderColor::all(palette::PANEL_BORDER),
                bevy::input_focus::tab_navigation::TabGroup::new(0),
                children![
                    // Title with color accent (green for victory, red for defeat)
                    (
                        Text::new(title),
                        TextFont::from_font_size(palette::FONT_SIZE_HEADER),
                        TextColor(title_color),
                    ),
                    // Exit to Menu button
                    widget::button(
                        "Exit to Menu",
                        0,
                        true,
                        |_: On<Activate>, mut next_game: ResMut<NextState<GameState>>| {
                            next_game.set(GameState::MainMenu);
                        },
                    ),
                ],
            ),
        ],
    ));
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
        crate::testing::init_input_resources(&mut app);
        app.init_state::<GameState>();
        app.init_state::<Menu>();
        app.add_plugins(plugin);
        // Transition to InGame first
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        // Now transition to the target menu
        app.world_mut().resource_mut::<NextState<Menu>>().set(menu);
        app.update();
        app.update(); // Apply deferred
        app
    }

    #[test]
    fn victory_screen_spawns_panel_and_button() {
        let mut app = create_overlay_test_app(Menu::Victory);

        // Title + 1 button label
        assert_entity_count::<With<Text>>(&mut app, 2);
        // Exit to Menu
        assert_entity_count::<With<Button>>(&mut app, 1);
    }

    #[test]
    fn defeat_screen_spawns_panel_and_button() {
        let mut app = create_overlay_test_app(Menu::Defeat);

        assert_entity_count::<With<Text>>(&mut app, 2);
        assert_entity_count::<With<Button>>(&mut app, 1);
    }

    fn create_endgame_escape_test_app(menu: Menu) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(StatesPlugin);
        crate::testing::init_input_resources(&mut app);
        app.init_state::<GameState>();
        app.init_state::<Menu>();
        app.add_systems(
            Update,
            close_endgame_on_escape.run_if(in_state(Menu::Victory).or(in_state(Menu::Defeat))),
        );

        // Transition to InGame then to the target menu
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app.world_mut().resource_mut::<NextState<Menu>>().set(menu);
        app.update();
        app
    }

    #[test]
    fn escape_exits_victory_to_main_menu() {
        let mut app = create_endgame_escape_test_app(Menu::Victory);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        let next = app.world().resource::<NextState<GameState>>();
        assert!(
            matches!(*next, NextState::Pending(GameState::MainMenu)),
            "Expected NextState to be GameState::MainMenu, got {next:?}"
        );
    }

    #[test]
    fn escape_exits_defeat_to_main_menu() {
        let mut app = create_endgame_escape_test_app(Menu::Defeat);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        let next = app.world().resource::<NextState<GameState>>();
        assert!(
            matches!(*next, NextState::Pending(GameState::MainMenu)),
            "Expected NextState to be GameState::MainMenu, got {next:?}"
        );
    }
}

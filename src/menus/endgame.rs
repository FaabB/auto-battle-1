//! Victory/Defeat overlay UI with bordered panel and clickable buttons.

use bevy::prelude::*;

use super::Menu;
use crate::screens::GameState;
use crate::theme::{palette, widget};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Victory), spawn_victory_screen);
    app.add_systems(OnEnter(Menu::Defeat), spawn_defeat_screen);
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
                        |_: On<Pointer<Click>>, mut next_game: ResMut<NextState<GameState>>| {
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
}

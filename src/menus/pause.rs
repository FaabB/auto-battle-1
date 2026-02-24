//! Pause menu UI: bordered panel with "Continue" and "Exit Game" buttons.

use bevy::prelude::*;

use super::Menu;
use crate::screens::GameState;
use crate::theme::{palette, widget};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Pause), spawn_pause_menu);
}

fn spawn_pause_menu(mut commands: Commands) {
    commands.spawn((
        widget::ui_root("Pause Menu"),
        BackgroundColor(palette::OVERLAY_BACKGROUND),
        GlobalZIndex(1),
        DespawnOnExit(Menu::Pause),
        children![
            // Bordered panel
            (
                Name::new("Pause Panel"),
                Node {
                    width: Val::Px(500.0),
                    min_height: Val::Px(400.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(Val::Px(40.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(palette::PANEL_BACKGROUND),
                BorderColor::all(palette::PANEL_BORDER),
                children![
                    // Title
                    (
                        Text::new("Auto Battle"),
                        TextFont::from_font_size(palette::FONT_SIZE_TITLE),
                        TextColor(palette::HEADER_TEXT),
                    ),
                    // Continue button
                    widget::button(
                        "Continue",
                        |_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>| {
                            next_menu.set(Menu::None);
                        },
                    ),
                    // Exit Game button
                    widget::button(
                        "Exit Game",
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
    use bevy::prelude::*;

    use crate::menus::Menu;
    use crate::screens::GameState;
    use crate::testing::assert_entity_count;

    #[test]
    fn pause_menu_spawns_panel_and_buttons() {
        use bevy::state::app::StatesPlugin;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.init_state::<Menu>();
        app.add_plugins(super::plugin);

        // Transition to InGame then Pause
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::Pause);
        app.update();
        app.update(); // Apply deferred

        // Title + 2 button labels
        assert_entity_count::<With<Text>>(&mut app, 3);
        // Continue + Exit Game
        assert_entity_count::<With<Button>>(&mut app, 2);
    }
}

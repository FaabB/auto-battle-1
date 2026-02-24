//! Main menu UI: bordered panel with title and clickable buttons.

use bevy::prelude::*;

use super::Menu;
use crate::screens::GameState;
use crate::theme::{palette, widget};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Main), spawn_main_menu);
}

fn spawn_main_menu(mut commands: Commands) {
    commands.spawn((
        widget::ui_root("Main Menu Screen"),
        DespawnOnExit(Menu::Main),
        children![
            // Bordered panel
            (
                Name::new("Main Menu Panel"),
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
                    // Start button
                    widget::button(
                        "Start Battle",
                        |_: On<Pointer<Click>>,
                         mut next_game: ResMut<NextState<GameState>>,
                         mut next_menu: ResMut<NextState<Menu>>| {
                            next_game.set(GameState::InGame);
                            next_menu.set(Menu::None);
                        },
                    ),
                    // Exit button
                    widget::button(
                        "Exit Game",
                        |_: On<Pointer<Click>>, mut exit: MessageWriter<AppExit>| {
                            exit.write(AppExit::Success);
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

    /// Verify that the main menu spawns UI entities when entering Menu::Main.
    #[test]
    fn main_menu_spawns_panel_and_buttons() {
        use crate::screens::GameState;
        use crate::testing::assert_entity_count;
        use bevy::state::app::StatesPlugin;

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.init_state::<Menu>();
        app.add_plugins(super::plugin);

        // Transition to Menu::Main
        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::Main);
        app.update();
        app.update(); // Apply deferred

        // Should have at least 1 Text entity (the title) and 2 Button entities
        assert_entity_count::<With<Text>>(&mut app, 3); // title + 2 button labels
        assert_entity_count::<With<Button>>(&mut app, 2); // start + exit
    }
}

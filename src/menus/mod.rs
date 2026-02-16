//! Menu overlays that can appear on top of any screen.
//!
//! The `Menu` state is orthogonal to `GameState` — menus are overlays,
//! not screens. For example, `Menu::Pause` appears while `GameState::InGame`
//! is active, and `Menu::Main` appears while `GameState::MainMenu` is active.

mod endgame;
mod main_menu;
mod pause;

use bevy::prelude::*;

/// Menu overlay states. Orthogonal to `GameState`.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[states(scoped_entities)]
pub enum Menu {
    /// No menu overlay is active.
    #[default]
    None,
    /// Main menu (shown on the title/main-menu screen).
    Main,
    /// Pause menu (shown in-game).
    Pause,
    /// Victory overlay (enemy fortress destroyed).
    Victory,
    /// Defeat overlay (player fortress destroyed).
    Defeat,
}

pub fn plugin(app: &mut App) {
    app.init_state::<Menu>();
    app.add_plugins((main_menu::plugin, pause::plugin, endgame::plugin));

    // Pause/unpause virtual time when any menu overlay opens/closes.
    // This stops physics (avian2d runs in FixedPostUpdate, which accumulates from Time<Virtual>)
    // and all timer-based systems (production, attack, income, waves).
    app.add_systems(OnExit(Menu::None), pause_virtual_time);
    app.add_systems(OnEnter(Menu::None), unpause_virtual_time);
}

fn pause_virtual_time(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn unpause_virtual_time(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_menu_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<Menu>();
        app.add_systems(OnExit(Menu::None), pause_virtual_time);
        app.add_systems(OnEnter(Menu::None), unpause_virtual_time);
        app.update();
        app
    }

    #[test]
    fn virtual_time_paused_on_menu_exit_none() {
        let mut app = create_menu_test_app();

        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::Pause);
        app.update();

        let time = app.world().resource::<Time<Virtual>>();
        assert!(
            time.is_paused(),
            "Time<Virtual> should be paused when menu is open"
        );
    }

    #[test]
    fn virtual_time_unpaused_on_menu_enter_none() {
        let mut app = create_menu_test_app();

        // Transition to Pause
        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::Pause);
        app.update();

        // Transition back to None
        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::None);
        app.update();

        let time = app.world().resource::<Time<Virtual>>();
        assert!(
            !time.is_paused(),
            "Time<Virtual> should be unpaused when menu closes"
        );
    }
}

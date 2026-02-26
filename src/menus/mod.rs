//! Menu overlays that can appear on top of any screen.
//!
//! The `Menu` state is orthogonal to `GameState` — menus are overlays,
//! not screens. For example, `Menu::Pause` appears while `GameState::InGame`
//! is active, and `Menu::Main` appears while `GameState::MainMenu` is active.

mod endgame;
mod main_menu;
mod pause;

use bevy::prelude::*;

use crate::screens::GameState;

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

    // Guarantee virtual time is unpaused when leaving InGame, regardless of
    // which Menu variant is active. Without this, Menu::Pause → Menu::Main
    // (and Victory/Defeat → Main) skip Menu::None, so `unpause_virtual_time`
    // never fires and Time<Virtual> stays paused for the rest of the session.
    app.add_systems(OnExit(GameState::InGame), unpause_virtual_time_on_game_exit);
}

fn pause_virtual_time(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn unpause_virtual_time(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

fn unpause_virtual_time_on_game_exit(mut time: ResMut<Time<Virtual>>) {
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

    /// Create a test app with both GameState and Menu states, plus the
    /// game-exit unpause system. Starts in GameState::InGame with Menu::Pause.
    fn create_game_exit_test_app(menu: Menu) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<GameState>();
        app.init_state::<Menu>();
        app.add_systems(OnExit(Menu::None), pause_virtual_time);
        app.add_systems(OnEnter(Menu::None), unpause_virtual_time);
        app.add_systems(OnExit(GameState::InGame), unpause_virtual_time_on_game_exit);
        app.update();

        // Enter InGame
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();

        // Open the specified menu overlay (pauses virtual time via OnExit(Menu::None))
        app.world_mut().resource_mut::<NextState<Menu>>().set(menu);
        app.update();

        let time = app.world().resource::<Time<Virtual>>();
        assert!(
            time.is_paused(),
            "Time<Virtual> should be paused when menu overlay is open"
        );

        app
    }

    #[test]
    fn virtual_time_unpaused_after_ingame_to_main_menu_via_pause() {
        let mut app = create_game_exit_test_app(Menu::Pause);

        // Exit game: GameState → MainMenu (Menu goes Pause → stays Pause,
        // never enters None — but OnExit(InGame) fires unpause)
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MainMenu);
        app.update();

        let time = app.world().resource::<Time<Virtual>>();
        assert!(
            !time.is_paused(),
            "Time<Virtual> should be unpaused after exiting InGame via pause menu"
        );
    }

    #[test]
    fn virtual_time_unpaused_after_ingame_to_main_menu_via_victory() {
        let mut app = create_game_exit_test_app(Menu::Victory);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MainMenu);
        app.update();

        let time = app.world().resource::<Time<Virtual>>();
        assert!(
            !time.is_paused(),
            "Time<Virtual> should be unpaused after exiting InGame via victory"
        );
    }

    #[test]
    fn virtual_time_unpaused_after_ingame_to_main_menu_via_defeat() {
        let mut app = create_game_exit_test_app(Menu::Defeat);

        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MainMenu);
        app.update();

        let time = app.world().resource::<Time<Virtual>>();
        assert!(
            !time.is_paused(),
            "Time<Virtual> should be unpaused after exiting InGame via defeat"
        );
    }
}

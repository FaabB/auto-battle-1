//! Endgame detection: checks fortress health and triggers victory/defeat.

use bevy::prelude::*;

use crate::gameplay::battlefield::{EnemyFortress, PlayerFortress};
use crate::gameplay::units::Health;
use crate::menus::Menu;
use crate::screens::GameState;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        detect_endgame
            .in_set(crate::GameSet::Death)
            .before(crate::gameplay::combat::check_death)
            .run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
    );
}

/// Checks fortress health each frame. If either fortress is dead, transitions
/// to the appropriate Menu overlay (Victory or Defeat).
fn detect_endgame(
    player_fortress: Query<&Health, With<PlayerFortress>>,
    enemy_fortress: Query<&Health, With<EnemyFortress>>,
    mut next_menu: ResMut<NextState<Menu>>,
) {
    // Check defeat first (player fortress destroyed)
    if let Ok(health) = player_fortress.single() {
        if health.current <= 0.0 {
            next_menu.set(Menu::Defeat);
            return;
        }
    }

    // Check victory (enemy fortress destroyed)
    if let Ok(health) = enemy_fortress.single() {
        if health.current <= 0.0 {
            next_menu.set(Menu::Victory);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;

    fn create_detection_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(StatesPlugin);
        app.init_state::<GameState>();
        app.init_state::<Menu>();
        // Must be in InGame + Menu::None for system to run
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.add_systems(
            Update,
            detect_endgame.run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
        );
        app.update(); // Apply state transitions
        app
    }

    #[test]
    fn detect_endgame_triggers_defeat_when_player_fortress_dead() {
        let mut app = create_detection_test_app();

        // Spawn player fortress with 0 HP
        app.world_mut().spawn((
            PlayerFortress,
            Health {
                current: 0.0,
                max: 2000.0,
            },
        ));
        // Spawn healthy enemy fortress
        app.world_mut().spawn((EnemyFortress, Health::new(2000.0)));

        app.update();

        let next_menu = app.world().resource::<NextState<Menu>>();
        assert!(
            matches!(*next_menu, NextState::Pending(Menu::Defeat)),
            "Expected Menu::Defeat, got {next_menu:?}",
        );
    }

    #[test]
    fn detect_endgame_triggers_victory_when_enemy_fortress_dead() {
        let mut app = create_detection_test_app();

        // Spawn healthy player fortress
        app.world_mut().spawn((PlayerFortress, Health::new(2000.0)));
        // Spawn enemy fortress with 0 HP
        app.world_mut().spawn((
            EnemyFortress,
            Health {
                current: 0.0,
                max: 2000.0,
            },
        ));

        app.update();

        let next_menu = app.world().resource::<NextState<Menu>>();
        assert!(
            matches!(*next_menu, NextState::Pending(Menu::Victory)),
            "Expected Menu::Victory, got {:?}",
            next_menu
        );
    }

    #[test]
    fn detect_endgame_does_nothing_when_both_alive() {
        let mut app = create_detection_test_app();

        app.world_mut().spawn((PlayerFortress, Health::new(2000.0)));
        app.world_mut().spawn((EnemyFortress, Health::new(2000.0)));

        app.update();

        let next_menu = app.world().resource::<NextState<Menu>>();
        assert!(
            matches!(*next_menu, NextState::Unchanged),
            "Expected no menu change, got {:?}",
            next_menu
        );
    }

    #[test]
    fn detect_endgame_prioritizes_defeat_over_victory() {
        let mut app = create_detection_test_app();

        // Both fortresses dead
        app.world_mut().spawn((
            PlayerFortress,
            Health {
                current: 0.0,
                max: 2000.0,
            },
        ));
        app.world_mut().spawn((
            EnemyFortress,
            Health {
                current: 0.0,
                max: 2000.0,
            },
        ));

        app.update();

        let next_menu = app.world().resource::<NextState<Menu>>();
        assert!(
            matches!(*next_menu, NextState::Pending(Menu::Defeat)),
            "Expected Menu::Defeat (player checked first), got {:?}",
            next_menu
        );
    }
}

//! Income systems: farm income and kill rewards.

use bevy::prelude::*;

use super::Gold;
use crate::gameplay::combat::DeathCheck;
use crate::gameplay::{Health, Team};
use crate::{GameSet, gameplay_running};

// === Components ===

/// Timer for passive gold income (e.g., Farms).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct IncomeTimer(pub Timer);

// === Systems ===

/// Ticks income timers and adds gold when they fire.
/// Runs in `GameSet::Production`.
fn tick_farm_income(time: Res<Time>, mut farms: Query<&mut IncomeTimer>, mut gold: ResMut<Gold>) {
    for mut timer in &mut farms {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            gold.0 += super::FARM_INCOME_PER_TICK;
        }
    }
}

/// Awards gold for each enemy that is about to die (Health <= 0).
/// Runs in `GameSet::Death` BEFORE `check_death` so entities still exist.
fn award_kill_gold(mut gold: ResMut<Gold>, query: Query<(&Health, &Team)>) {
    for (health, team) in &query {
        if health.current <= 0.0 && *team == Team::Enemy {
            gold.0 += super::KILL_REWARD;
        }
    }
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<IncomeTimer>();

    app.add_systems(
        Update,
        tick_farm_income
            .in_set(GameSet::Production)
            .run_if(gameplay_running),
    );

    app.add_systems(
        Update,
        award_kill_gold
            .in_set(GameSet::Death)
            .before(DeathCheck)
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // === Farm Income Tests ===

    fn create_farm_income_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Gold>();
        app.add_systems(Update, tick_farm_income);
        app.update(); // Initialize time (first frame delta=0)
        app
    }

    /// Create an income timer that will fire on the next tick with any positive delta.
    fn nearly_elapsed_income_timer() -> IncomeTimer {
        let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
        crate::testing::nearly_expire_timer(&mut timer);
        IncomeTimer(timer)
    }

    #[test]
    fn farm_income_adds_gold() {
        let mut app = create_farm_income_test_app();

        app.world_mut().spawn(nearly_elapsed_income_timer());
        app.update();

        let gold = app.world().resource::<Gold>();
        assert_eq!(
            gold.0,
            super::super::STARTING_GOLD + super::super::FARM_INCOME_PER_TICK
        );
    }

    #[test]
    fn multiple_farms_add_gold_independently() {
        let mut app = create_farm_income_test_app();

        app.world_mut().spawn(nearly_elapsed_income_timer());
        app.world_mut().spawn(nearly_elapsed_income_timer());
        app.update();

        let gold = app.world().resource::<Gold>();
        assert_eq!(
            gold.0,
            super::super::STARTING_GOLD + super::super::FARM_INCOME_PER_TICK * 2
        );
    }

    #[test]
    fn farm_income_no_farms_no_change() {
        let mut app = create_farm_income_test_app();

        app.update();

        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, super::super::STARTING_GOLD);
    }

    // === Kill Reward Tests ===

    fn create_kill_reward_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Gold>();
        app.add_systems(Update, award_kill_gold);
        app
    }

    #[test]
    fn kill_reward_for_enemy_death() {
        let mut app = create_kill_reward_test_app();

        // Enemy with HP <= 0 should award gold
        app.world_mut().spawn((
            Health {
                current: 0.0,
                max: 100.0,
            },
            Team::Enemy,
        ));
        app.update();

        let gold = app.world().resource::<Gold>();
        assert_eq!(
            gold.0,
            super::super::STARTING_GOLD + super::super::KILL_REWARD
        );
    }

    #[test]
    fn kill_reward_for_negative_hp_enemy() {
        let mut app = create_kill_reward_test_app();

        app.world_mut().spawn((
            Health {
                current: -10.0,
                max: 100.0,
            },
            Team::Enemy,
        ));
        app.update();

        let gold = app.world().resource::<Gold>();
        assert_eq!(
            gold.0,
            super::super::STARTING_GOLD + super::super::KILL_REWARD
        );
    }

    #[test]
    fn no_kill_reward_for_player_death() {
        let mut app = create_kill_reward_test_app();

        // Player unit with HP <= 0 should NOT award gold
        app.world_mut().spawn((
            Health {
                current: 0.0,
                max: 100.0,
            },
            Team::Player,
        ));
        app.update();

        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, super::super::STARTING_GOLD);
    }

    #[test]
    fn no_kill_reward_for_alive_enemy() {
        let mut app = create_kill_reward_test_app();

        // Enemy with HP > 0 should NOT award gold
        app.world_mut().spawn((
            Health {
                current: 50.0,
                max: 100.0,
            },
            Team::Enemy,
        ));
        app.update();

        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, super::super::STARTING_GOLD);
    }

    #[test]
    fn multiple_enemy_kills_award_multiple_rewards() {
        let mut app = create_kill_reward_test_app();

        for _ in 0..3 {
            app.world_mut().spawn((
                Health {
                    current: 0.0,
                    max: 100.0,
                },
                Team::Enemy,
            ));
        }
        app.update();

        let gold = app.world().resource::<Gold>();
        assert_eq!(
            gold.0,
            super::super::STARTING_GOLD + super::super::KILL_REWARD * 3
        );
    }
}

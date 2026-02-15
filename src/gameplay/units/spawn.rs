//! Continuous enemy spawning with ramping difficulty.

use bevy::prelude::*;
use rand::Rng;

use crate::gameplay::battlefield::{
    BATTLEFIELD_ROWS, ENEMY_FORT_START_COL, col_to_world_x, row_to_world_y,
};
use crate::screens::GameState;
use crate::{GameSet, Z_UNIT, gameplay_running};

use crate::gameplay::Team;

use super::UnitAssets;

// === Constants ===

/// Seconds before the first enemy spawns after entering `InGame`.
pub const INITIAL_DELAY: f32 = 5.0;

/// Starting spawn interval (seconds between enemies).
pub const START_INTERVAL: f32 = 3.0;

/// Minimum spawn interval (floor — never spawns faster than this).
pub const MIN_INTERVAL: f32 = 0.5;

/// Duration (seconds) over which the interval ramps from START to MIN.
pub const RAMP_DURATION: f32 = 600.0; // 10 minutes

/// Column where enemies spawn (at the enemy fortress).
const ENEMY_SPAWN_COL: u16 = ENEMY_FORT_START_COL; // col 80

// === Resource ===

/// Tracks enemy spawn timing with ramping difficulty.
///
/// Inserted on `OnEnter(GameState::InGame)`, reset each time the state is entered.
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct EnemySpawnTimer {
    /// Timer that fires to trigger a spawn. Starts as one-shot for initial delay,
    /// then re-created as one-shot with decreasing intervals after each spawn.
    pub timer: Timer,
    /// Total elapsed time (seconds) since entering `InGame`. Used for ramp calculation.
    pub elapsed_secs: f32,
}

impl Default for EnemySpawnTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(INITIAL_DELAY, TimerMode::Once),
            elapsed_secs: 0.0,
        }
    }
}

// === Pure Functions ===

/// Compute the current spawn interval based on elapsed time.
///
/// Returns `START_INTERVAL` at the moment spawning begins (after `INITIAL_DELAY`),
/// linearly decreasing to `MIN_INTERVAL` over `RAMP_DURATION` seconds.
#[must_use]
pub fn current_interval(elapsed_secs: f32) -> f32 {
    let spawning_time = (elapsed_secs - INITIAL_DELAY).max(0.0);
    let t = (spawning_time / RAMP_DURATION).min(1.0);
    (MIN_INTERVAL - START_INTERVAL).mul_add(t, START_INTERVAL)
}

// === Systems ===

/// Reset (or insert) the spawn timer when entering `InGame`.
fn reset_enemy_spawn_timer(mut commands: Commands) {
    commands.insert_resource(EnemySpawnTimer::default());
}

/// Tick the spawn timer and spawn an enemy when it fires.
fn tick_enemy_spawner(
    time: Res<Time>,
    mut spawn_timer: ResMut<EnemySpawnTimer>,
    unit_assets: Res<UnitAssets>,
    mut commands: Commands,
) {
    spawn_timer.elapsed_secs += time.delta_secs();
    spawn_timer.timer.tick(time.delta());

    if !spawn_timer.timer.just_finished() {
        return;
    }

    // Pick a random row
    let row = rand::rng().random_range(0..BATTLEFIELD_ROWS);
    let spawn_x = col_to_world_x(ENEMY_SPAWN_COL);
    let spawn_y = row_to_world_y(row);

    super::spawn_unit(
        &mut commands,
        super::UnitType::Soldier,
        Team::Enemy,
        Vec3::new(spawn_x, spawn_y, Z_UNIT),
        &unit_assets,
    );

    // Set next spawn interval based on elapsed time
    let next_interval = current_interval(spawn_timer.elapsed_secs);
    spawn_timer.timer = Timer::from_seconds(next_interval, TimerMode::Once);
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<EnemySpawnTimer>();

    app.add_systems(OnEnter(GameState::InGame), reset_enemy_spawn_timer);

    app.add_systems(
        Update,
        tick_enemy_spawner
            .in_set(GameSet::Production)
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn constants_are_valid() {
        assert!(INITIAL_DELAY > 0.0);
        assert!(START_INTERVAL > 0.0);
        assert!(MIN_INTERVAL > 0.0);
        assert!(START_INTERVAL > MIN_INTERVAL);
        assert!(RAMP_DURATION > 0.0);
    }

    #[test]
    fn default_timer_has_initial_delay() {
        let timer = EnemySpawnTimer::default();
        assert_eq!(timer.timer.duration().as_secs_f32(), INITIAL_DELAY);
        assert_eq!(timer.elapsed_secs, 0.0);
    }

    #[test]
    fn current_interval_at_start_is_start_interval() {
        let interval = current_interval(INITIAL_DELAY);
        assert!((interval - START_INTERVAL).abs() < f32::EPSILON);
    }

    #[test]
    fn current_interval_at_ramp_end_is_min_interval() {
        let interval = current_interval(INITIAL_DELAY + RAMP_DURATION);
        assert!((interval - MIN_INTERVAL).abs() < f32::EPSILON);
    }

    #[test]
    fn current_interval_beyond_ramp_stays_at_min() {
        let interval = current_interval(INITIAL_DELAY + RAMP_DURATION + 100.0);
        assert!((interval - MIN_INTERVAL).abs() < f32::EPSILON);
    }

    #[test]
    fn current_interval_at_midpoint() {
        let midpoint = INITIAL_DELAY + RAMP_DURATION / 2.0;
        let expected = (START_INTERVAL + MIN_INTERVAL) / 2.0;
        let interval = current_interval(midpoint);
        assert!((interval - expected).abs() < 0.01);
    }

    #[test]
    fn current_interval_before_initial_delay_is_start() {
        let interval = current_interval(0.0);
        assert!((interval - START_INTERVAL).abs() < f32::EPSILON);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::super::{CombatStats, CurrentTarget, Movement, Unit, UnitType};
    use super::*;
    use crate::gameplay::{Health, Target, Team};
    use crate::testing::{assert_entity_count, transition_to_ingame};
    use std::time::Duration;

    /// Create a test app with the spawn plugin active.
    fn create_spawn_test_app() -> App {
        let mut app = crate::testing::create_base_test_app();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();

        // Register unit assets setup + spawn plugin
        app.add_systems(OnEnter(GameState::InGame), super::super::setup_unit_assets);
        plugin(&mut app);
        transition_to_ingame(&mut app);
        app
    }

    #[test]
    fn spawn_timer_resource_exists_after_entering_ingame() {
        let app = create_spawn_test_app();
        assert!(app.world().get_resource::<EnemySpawnTimer>().is_some());
    }

    #[test]
    fn no_enemies_during_initial_delay() {
        let mut app = create_spawn_test_app();
        // Run a few frames — should still be in initial delay
        app.update();
        app.update();
        assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 0);
    }

    /// Set elapsed to 1 nanosecond before the timer's duration so any positive
    /// wall-clock delta triggers `just_finished()`.
    fn nearly_expire_timer(app: &mut App) {
        let duration = app.world().resource::<EnemySpawnTimer>().timer.duration();
        app.world_mut()
            .resource_mut::<EnemySpawnTimer>()
            .timer
            .set_elapsed(duration - Duration::from_nanos(1));
    }

    #[test]
    fn enemy_spawns_after_initial_delay() {
        let mut app = create_spawn_test_app();

        nearly_expire_timer(&mut app);
        app.update();
        assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 1);
    }

    #[test]
    fn spawned_enemy_has_correct_team() {
        let mut app = create_spawn_test_app();

        nearly_expire_timer(&mut app);
        app.update();

        let mut query = app.world_mut().query_filtered::<&Team, With<Unit>>();
        let team = query.single(app.world()).unwrap();
        assert_eq!(*team, Team::Enemy);
    }

    #[test]
    fn spawned_enemy_has_all_components() {
        let mut app = create_spawn_test_app();

        nearly_expire_timer(&mut app);
        app.update();

        assert_entity_count::<(With<Unit>, With<UnitType>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<Target>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<CurrentTarget>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<Health>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<crate::gameplay::combat::HealthBarConfig>)>(
            &mut app, 1,
        );
        assert_entity_count::<(With<Unit>, With<CombatStats>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<Movement>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<DespawnOnExit<GameState>>)>(&mut app, 1);
    }

    #[test]
    fn timer_updates_interval_after_spawn() {
        let mut app = create_spawn_test_app();

        // Trigger initial spawn
        nearly_expire_timer(&mut app);
        app.update();

        // After first spawn, timer should have START_INTERVAL duration
        let timer = app.world().resource::<EnemySpawnTimer>();
        let duration = timer.timer.duration().as_secs_f32();
        assert!(
            (duration - START_INTERVAL).abs() < 0.01,
            "Expected ~{START_INTERVAL}s, got {duration}s"
        );
    }

    #[test]
    fn second_enemy_spawns_after_next_interval() {
        let mut app = create_spawn_test_app();

        // Trigger first spawn
        nearly_expire_timer(&mut app);
        app.update();
        assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 1);

        // Nearly expire next timer (now START_INTERVAL duration)
        nearly_expire_timer(&mut app);
        app.update();
        assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 2);
    }
}

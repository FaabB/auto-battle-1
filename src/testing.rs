//! Testing utilities for Bevy systems.

use std::time::Duration;

use avian2d::prelude::*;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::state::state::FreelyMutableState;
use bevy::window::WindowPlugin;

use crate::gameplay::combat::AttackTimer;
use crate::gameplay::units::pathfinding::NavPath;
use crate::gameplay::units::{UNIT_RADIUS, Unit, UnitType, unit_stats};
use crate::gameplay::{CombatStats, CurrentTarget, Health, Movement, Target, Team};

/// Creates a minimal app for testing with essential plugins.
pub fn create_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app
}

/// Creates a test app with state support.
#[allow(dead_code)]
pub fn create_test_app_with_state<S: FreelyMutableState + Default>() -> App {
    let mut app = create_test_app();
    app.init_state::<S>();
    app
}

/// Creates a base test app with states, input, window, and camera.
///
/// Does NOT transition to `InGame` — add your domain plugins first, then
/// call [`transition_to_ingame`] to trigger `OnEnter` systems.
#[allow(dead_code)]
pub fn create_base_test_app() -> App {
    let mut app = create_test_app();
    app.add_plugins(StatesPlugin);
    app.add_plugins(InputPlugin);
    app.add_plugins(WindowPlugin::default());
    app.init_state::<crate::screens::GameState>();
    app.init_state::<crate::menus::Menu>();
    app.world_mut().spawn(Camera2d);
    app
}

/// Same as [`create_base_test_app`] but without `InputPlugin`.
///
/// Use when testing systems that read `ButtonInput` and you need `press()`
/// to persist through to `Update` (since `InputPlugin` clears `just_pressed`
/// in `PreUpdate`). Manually `init_resource::<ButtonInput<MouseButton>>()` etc.
#[allow(dead_code)]
pub fn create_base_test_app_no_input() -> App {
    let mut app = create_test_app();
    app.add_plugins(StatesPlugin);
    app.add_plugins(WindowPlugin::default());
    app.init_state::<crate::screens::GameState>();
    app.init_state::<crate::menus::Menu>();
    app.world_mut().spawn(Camera2d);
    app
}

/// Transitions the app to `GameState::InGame` and runs two updates
/// (first applies the transition + `OnEnter`, second applies deferred commands).
pub fn transition_to_ingame(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<crate::screens::GameState>>()
        .set(crate::screens::GameState::InGame);
    app.update();
    app.update();
}

/// Count entities that match a query filter.
///
/// Usage: `assert_eq!(count_entities::<With<PlayerFortress>>(&mut app), 1);`
#[allow(dead_code)]
pub fn count_entities<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), F>()
        .iter(app.world())
        .count()
}

/// Assert exactly N entities match a query filter.
///
/// Panics with a descriptive message including the count.
#[allow(dead_code)]
pub fn assert_entity_count<F: bevy::ecs::query::QueryFilter>(app: &mut App, expected: usize) {
    let actual = count_entities::<F>(app);
    assert_eq!(
        actual, expected,
        "Expected {expected} entities matching filter, found {actual}"
    );
}

/// Helper to advance the app by multiple frames.
#[allow(dead_code)]
pub fn tick_multiple(app: &mut App, count: usize) {
    for _ in 0..count {
        app.update();
    }
}

// === Timer Helpers ===

/// Set a timer's elapsed to `duration - 1ns` so the next `tick()` with any
/// positive delta triggers `just_finished()`.
///
/// Works for any `Timer` regardless of duration or mode.
#[allow(dead_code)]
pub fn nearly_expire_timer(timer: &mut Timer) {
    let duration = timer.duration();
    timer.set_elapsed(duration - Duration::from_nanos(1));
}

// === Resource Init Helpers ===

/// Init `Assets<Mesh>` and `Assets<ColorMaterial>` — needed by any test that
/// uses `UnitAssets` or spawns mesh-based entities.
#[allow(dead_code)]
pub fn init_asset_resources(app: &mut App) {
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<ColorMaterial>>();
}

/// Init `Gold` and `Shop` resources — needed by building placement and
/// production tests.
#[allow(dead_code)]
pub fn init_economy_resources(app: &mut App) {
    app.init_resource::<crate::gameplay::economy::Gold>();
    app.init_resource::<crate::gameplay::economy::shop::Shop>();
}

/// Init `ButtonInput<KeyCode>` and `ButtonInput<MouseButton>` — needed when
/// `InputPlugin` is skipped to avoid `just_pressed` being cleared in `PreUpdate`.
#[allow(dead_code)]
pub fn init_input_resources(app: &mut App) {
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ButtonInput<MouseButton>>();
}

// === Entity Spawn Helpers ===

/// Spawn a test unit with the full Soldier archetype at `(x, y)`.
///
/// Includes: Unit, UnitType::Soldier, Team, Target, CurrentTarget(None),
/// Health, CombatStats, Movement, AttackTimer, Transform, GlobalTransform,
/// Collider, LinearVelocity, NavPath.
///
/// Callers can override specific components via `world.entity_mut(id).insert(...)`.
#[allow(dead_code)]
pub fn spawn_test_unit(world: &mut World, team: Team, x: f32, y: f32) -> Entity {
    let stats = unit_stats(UnitType::Soldier);
    world
        .spawn((
            Unit,
            UnitType::Soldier,
            team,
            Target,
            CurrentTarget(None),
            Health::new(stats.hp),
            CombatStats {
                damage: stats.damage,
                attack_speed: stats.attack_speed,
                range: stats.attack_range,
            },
            Movement {
                speed: stats.move_speed,
            },
            AttackTimer(Timer::from_seconds(
                1.0 / stats.attack_speed,
                TimerMode::Repeating,
            )),
            Transform::from_xyz(x, y, 0.0),
            GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
            Collider::circle(UNIT_RADIUS),
            LinearVelocity::ZERO,
            NavPath::default(),
        ))
        .id()
}

/// Spawn a non-unit targetable entity at `(x, y)`.
///
/// Includes: Team, Target, Transform, GlobalTransform, Collider (5px radius).
/// Add `Health` via `world.entity_mut(id).insert(Health::new(hp))` for attack tests.
#[allow(dead_code)]
pub fn spawn_test_target(world: &mut World, team: Team, x: f32, y: f32) -> Entity {
    world
        .spawn((
            team,
            Target,
            Transform::from_xyz(x, y, 0.0),
            GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
            Collider::circle(5.0),
        ))
        .id()
}

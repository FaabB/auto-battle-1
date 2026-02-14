# Enemy Stream (Continuous Spawning) — GAM-7 Implementation Plan

## Overview

Replace the debug enemy spawner (E key in dev_tools) with a continuous automatic enemy spawner that ramps up spawn rate over time. Enemies appear from the right side of the battlefield with linearly decreasing spawn intervals.

## Current State Analysis

- **Debug spawner** in `src/dev_tools/mod.rs:26-66` — spawns 3 enemies on E key at column 75, rows 2/5/8. Feature-gated on `dev`.
- **Unit bundle** for enemies: `Unit`, `Team::Enemy`, `Target`, `CurrentTarget(None)`, `Health`, `CombatStats`, `Movement`, `AttackTimer`, mesh/material, `Transform`, `DespawnOnExit(GameState::InGame)`.
- **UnitAssets** resource (`units/mod.rs:95-101`) provides shared mesh + material handles, created `OnEnter(GameState::InGame)`.
- **GameSet::Production** is where barracks unit spawning lives — enemy spawning slots in here too.
- **`rand` 0.9** already in `Cargo.toml`.

### Key Discoveries:
- Enemy spawn bundle is identical between `dev_tools/mod.rs:42-64` and `building/production.rs:30-52` except for `Team` and material. Not extracting a shared helper (minimal scope).
- `dev_tools/mod.rs` contains ONLY the debug spawner — after removal, it becomes an empty plugin stub.
- GAM-9 (Victory/Defeat) mentions "wave counter reset" — since GAM-7 is continuous (not discrete waves), GAM-9 just needs `EnemySpawnTimer` reset on restart. No wave counter resource needed.

## Desired End State

A new `src/gameplay/units/spawn.rs` module that:
1. Inserts an `EnemySpawnTimer` resource on `OnEnter(GameState::InGame)`
2. Runs a `tick_enemy_spawner` system in `GameSet::Production` that automatically spawns enemies
3. Spawn rate ramps from 3s → 0.5s linearly over 10 minutes, with a 5s initial delay
4. Debug spawner in `dev_tools/mod.rs` is removed

### Verification:
- `make check` passes (clippy + formatting)
- `make test` passes with ≥90% coverage maintained
- `cargo run` — enemies appear automatically ~5s after entering InGame, spawn rate visibly increases
- E key no longer spawns enemies in dev builds

## What We're NOT Doing

- Enemy type variety (different unit types at higher elapsed time)
- Stat scaling (HP/damage increase over time)
- Spawn burst patterns
- Wave counter UI (no discrete waves)
- Extracting a shared `spawn_unit()` helper (acceptable duplication for now)

## Implementation Approach

Single resource (`EnemySpawnTimer`) with a `Timer` that fires once to handle the initial delay, then re-creates itself with decreasing intervals after each spawn. A pure function `current_interval(elapsed_secs)` computes the ramp. Enemies spawn at a fixed column near the enemy fortress, random row.

## Constants

```rust
/// Seconds before the first enemy spawns after entering InGame.
pub const INITIAL_DELAY: f32 = 5.0;

/// Starting spawn interval (seconds between enemies).
pub const START_INTERVAL: f32 = 3.0;

/// Minimum spawn interval (floor — never spawns faster than this).
pub const MIN_INTERVAL: f32 = 0.5;

/// Duration (seconds) over which the interval ramps from START to MIN.
pub const RAMP_DURATION: f32 = 600.0; // 10 minutes

/// Column where enemies spawn (near enemy fortress side of combat zone).
pub const ENEMY_SPAWN_COL: u16 = COMBAT_ZONE_START_COL + COMBAT_ZONE_COLS - 5; // col 75
```

## Verified API Patterns (Bevy 0.18)

- `Timer::from_seconds(secs, TimerMode::Once)` — one-shot timer for initial delay
- `timer.tick(time.delta())` + `timer.just_finished()` — standard tick-and-check
- `DespawnOnExit(GameState::InGame)` — state-scoped cleanup, in prelude
- `rand::rng().random_range(0..N)` — rand 0.9 API for random row selection
- `#[reflect(Resource)]` for resource reflection registration

---

## Phase 1: Create Spawn Module

### Overview
Add `src/gameplay/units/spawn.rs` with the resource, constants, ramp function, and spawn system. Register in the units plugin.

### Changes Required:

#### 1. New file: `src/gameplay/units/spawn.rs`

```rust
//! Continuous enemy spawning with ramping difficulty.

use std::time::Duration;

use bevy::prelude::*;

use crate::Z_UNIT;
use crate::gameplay::battlefield::{
    BATTLEFIELD_ROWS, COMBAT_ZONE_COLS, COMBAT_ZONE_START_COL, col_to_world_x, row_to_world_y,
};
use crate::gameplay::combat::AttackTimer;
use crate::screens::GameState;

use super::{
    CombatStats, CurrentTarget, Health, Movement, SOLDIER_ATTACK_RANGE, SOLDIER_ATTACK_SPEED,
    SOLDIER_DAMAGE, SOLDIER_HEALTH, SOLDIER_MOVE_SPEED, Target, Team, Unit, UnitAssets,
};

// === Constants ===

/// Seconds before the first enemy spawns after entering InGame.
pub const INITIAL_DELAY: f32 = 5.0;

/// Starting spawn interval (seconds between enemies).
pub const START_INTERVAL: f32 = 3.0;

/// Minimum spawn interval (floor — never spawns faster than this).
pub const MIN_INTERVAL: f32 = 0.5;

/// Duration (seconds) over which the interval ramps from START to MIN.
pub const RAMP_DURATION: f32 = 600.0; // 10 minutes

/// Column where enemies spawn (near enemy fortress side of combat zone).
const ENEMY_SPAWN_COL: u16 = COMBAT_ZONE_START_COL + COMBAT_ZONE_COLS - 5; // col 75

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
    /// Total elapsed time (seconds) since entering InGame. Used for ramp calculation.
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

/// Reset (or insert) the spawn timer when entering InGame.
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

    commands.spawn((
        Unit,
        Team::Enemy,
        Target,
        CurrentTarget(None),
        Health::new(SOLDIER_HEALTH),
        CombatStats {
            damage: SOLDIER_DAMAGE,
            attack_speed: SOLDIER_ATTACK_SPEED,
            range: SOLDIER_ATTACK_RANGE,
        },
        Movement {
            speed: SOLDIER_MOVE_SPEED,
        },
        AttackTimer(Timer::from_seconds(
            1.0 / SOLDIER_ATTACK_SPEED,
            TimerMode::Repeating,
        )),
        Mesh2d(unit_assets.mesh.clone()),
        MeshMaterial2d(unit_assets.enemy_material.clone()),
        Transform::from_xyz(spawn_x, spawn_y, Z_UNIT),
        DespawnOnExit(GameState::InGame),
    ));

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
            .in_set(crate::GameSet::Production)
            .run_if(
                in_state(GameState::InGame)
                    .and(in_state(crate::menus::Menu::None)),
            ),
    );
}
```

#### 2. Register spawn module in `src/gameplay/units/mod.rs`

Add module declaration and plugin registration:

```rust
// At top of file, after existing mod declarations:
mod ai;
mod movement;
pub(crate) mod spawn; // NEW

// In the plugin function, add:
spawn::plugin(app);
```

The `pub(crate)` visibility allows GAM-9 to reference `EnemySpawnTimer` for reset logic.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes
- [x] `cargo build` succeeds

#### Manual Verification:
- [x] `cargo run` — enemies appear automatically ~5s after entering InGame
- [x] Spawn rate visibly increases over time (watch for ~30 seconds)
- [x] Enemies spawn at varying rows (not always the same position)

---

## Phase 2: Remove Debug Spawner

### Overview
Remove the debug enemy spawner from `dev_tools/mod.rs`. Leave the module as an empty plugin stub for future dev tools.

### Changes Required:

#### 1. Gut `src/dev_tools/mod.rs`

Replace entire contents with:

```rust
//! Development tools — only included with `cargo run --features dev`.
//!
//! Debug overlays, inspector setup, and diagnostic tools go here.
//! This module is stripped from release builds.

use bevy::prelude::*;

#[allow(clippy::missing_const_for_fn)]
pub(super) fn plugin(_app: &mut App) {
    // Future dev tools go here.
}
```

This removes:
- `debug_spawn_enemies` system
- `ENEMIES_PER_SPAWN` and `DEBUG_SPAWN_COL` constants
- All imports related to spawning
- All tests (they tested the debug spawner)

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes
- [x] `cargo build --features dev` succeeds

#### Manual Verification:
- [x] `cargo run --features dev` — pressing E no longer spawns enemies
- [x] Enemies still auto-spawn from Phase 1

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Phase 3: Tests

### Overview
Add unit tests for the ramp function and integration tests for the spawn system. Target ≥90% coverage.

### Changes Required:

#### 1. Unit tests in `src/gameplay/units/spawn.rs`

```rust
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
        // Right when spawning begins (elapsed == INITIAL_DELAY)
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
        // Before spawning starts, interval should be START_INTERVAL
        let interval = current_interval(0.0);
        assert!((interval - START_INTERVAL).abs() < f32::EPSILON);
    }
}
```

#### 2. Integration tests in `src/gameplay/units/spawn.rs`

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::gameplay::units::{UNIT_RADIUS, UnitAssets};
    use crate::testing::{assert_entity_count, transition_to_ingame};
    use std::time::Duration;

    /// Create a test app with the spawn plugin active.
    /// Does NOT include InputPlugin (not needed for spawning).
    fn create_spawn_test_app() -> App {
        let mut app = crate::testing::create_base_test_app();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();

        // Register unit assets setup + spawn plugin
        app.add_systems(
            OnEnter(GameState::InGame),
            super::super::setup_unit_assets,
        );
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

    #[test]
    fn enemy_spawns_after_initial_delay() {
        let mut app = create_spawn_test_app();

        // Nearly expire the initial delay timer
        app.world_mut()
            .resource_mut::<EnemySpawnTimer>()
            .timer
            .set_elapsed(Duration::from_secs_f32(INITIAL_DELAY - 0.001));

        app.update();
        assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 1);
    }

    #[test]
    fn spawned_enemy_has_correct_team() {
        let mut app = create_spawn_test_app();

        app.world_mut()
            .resource_mut::<EnemySpawnTimer>()
            .timer
            .set_elapsed(Duration::from_secs_f32(INITIAL_DELAY - 0.001));
        app.update();

        let mut query = app.world_mut().query_filtered::<&Team, With<Unit>>();
        let team = query.single(app.world()).unwrap();
        assert_eq!(*team, Team::Enemy);
    }

    #[test]
    fn spawned_enemy_has_all_components() {
        let mut app = create_spawn_test_app();

        app.world_mut()
            .resource_mut::<EnemySpawnTimer>()
            .timer
            .set_elapsed(Duration::from_secs_f32(INITIAL_DELAY - 0.001));
        app.update();

        assert_entity_count::<(With<Unit>, With<Target>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<CurrentTarget>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<Health>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<CombatStats>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<Movement>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<DespawnOnExit<GameState>>)>(&mut app, 1);
    }

    #[test]
    fn timer_updates_interval_after_spawn() {
        let mut app = create_spawn_test_app();

        // Trigger initial spawn
        app.world_mut()
            .resource_mut::<EnemySpawnTimer>()
            .timer
            .set_elapsed(Duration::from_secs_f32(INITIAL_DELAY - 0.001));
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
        app.world_mut()
            .resource_mut::<EnemySpawnTimer>()
            .timer
            .set_elapsed(Duration::from_secs_f32(INITIAL_DELAY - 0.001));
        app.update();
        assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 1);

        // Nearly expire next timer
        app.world_mut()
            .resource_mut::<EnemySpawnTimer>()
            .timer
            .set_elapsed(Duration::from_secs_f32(START_INTERVAL - 0.001));
        app.update();
        assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 2);
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes
- [x] All new tests pass individually

#### Manual Verification:
- [x] Full game loop works: enemies auto-spawn, player can build, combat works, economy works

---

## Testing Strategy

### Unit Tests:
- `current_interval` ramp function: start value, end value, beyond-end clamping, midpoint, before-delay
- Constants validity (positive, start > min)
- Default resource state

### Integration Tests:
- Resource creation on state enter
- No spawns during initial delay
- Spawn triggers after delay expires
- Correct enemy team and components
- Timer interval updates after spawn
- Multiple spawns work in sequence

### Manual Testing Steps:
1. `cargo run` — enter InGame, wait 5s for first enemy
2. Watch for ~30s — spawn rate should visibly increase
3. Verify enemies come from the right side, appear at different rows
4. Build barracks, verify player units still spawn and fight enemies
5. Pause and unpause — spawning should pause/resume correctly
6. Return to main menu and re-enter InGame — timer should reset

## References

- Linear ticket: GAM-7 "Enemy Stream (Continuous Spawning)"
- Debug spawner being replaced: `src/dev_tools/mod.rs:26-66`
- Unit component bundle pattern: `src/dev_tools/mod.rs:42-64`
- Barracks production pattern: `src/gameplay/building/production.rs:16-55`
- Unit assets setup: `src/gameplay/units/mod.rs:105-115`
- GameSet ordering: `src/lib.rs:40-56`

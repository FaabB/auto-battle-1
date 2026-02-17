# GAM-15: Overhaul Fortress — Implementation Plan

## Overview

Rework fortresses from full-height zone markers (2×10 cells, 128×640px) into compact 2×2 buildings (128×128px) centered vertically, with combat capability (targeting + projectile attacks) and enemy spawning from the fortress entity position.

## Current State Analysis

- **Fortress size**: 2 cols × 10 rows (128×640px) — spans full battlefield height
- **Position**: Player at cols 0-1, enemy at cols 80-81, both at `battlefield_center_y()` (320px)
- **Components**: `PlayerFortress`/`EnemyFortress`, `Team`, `Target`, `Health(2000)`, `HealthBarConfig`, `Sprite`, `RigidBody::Static`, `Collider::rectangle(128, 640)`, `CollisionLayers`
- **Missing for combat**: `CombatStats`, `AttackTimer`, `CurrentTarget`
- **Attack system** (`combat/attack.rs:52`): `unit_attack` filters `With<Unit>` — fortresses excluded
- **Enemy spawner** (`units/spawn.rs:31`): Uses `const ENEMY_SPAWN_COL` — hardcoded, not reading fortress position
- **Health bar**: `width=100, height=6, y_offset=0` (centered on 640px-tall fortress)
- **Dependencies**: GAM-11 (component refactor) ✅ Done, GAM-10 (physics) ✅ Done

### Key Discoveries:
- `unit_attack` (`combat/attack.rs:52-115`) queries `With<Unit>` — removing this makes any entity with `CombatStats + AttackTimer + CurrentTarget + GlobalTransform + Collider + Team` able to attack
- `unit_find_target` (`units/ai.rs:20-77`) uses `With<Unit>` and backtrack logic — can be generalized by removing `With<Unit>` and making backtrack conditional on `Option<&Movement>` (static entities skip backtrack)
- `check_death` (`combat/death.rs:14-19`) despawns ANY entity with `Health <= 0` — fortresses already handled
- `detect_endgame` (`endgame_detection.rs:23-41`) runs `.before(DeathCheck)` — catches fortress death before despawn
- `Single<D, F>` skips the system entirely if 0 or >1 entities match — perfect for spawner when fortress is destroyed

## Desired End State

After implementation:
1. Fortresses are 2×2 cells (128×128px) centered vertically in their columns
2. Both fortresses fire projectiles at enemy units within 200px range (turret-like: 50 dmg, 0.5 attacks/sec)
3. Enemy units spawn at the enemy fortress entity's position (not a hardcoded column)
4. If enemy fortress is destroyed, enemy spawning stops naturally
5. Health bars sit above the fortress, proportional to its visual size
6. All existing endgame/targeting/combat systems continue working unchanged

## What We're NOT Doing

- Moving `CurrentTarget` or `CombatStats` to `gameplay/mod.rs` (would be cleaner but out of scope)
- Player fortress building/repair mechanics
- Fortress upgrade system
- Different stats per fortress (symmetric for now)
- Visual fortress redesign (keep colored rectangles)

## Implementation Approach

Three phases: resize → add combat → update spawning. Each phase is independently testable.

---

## Phase 1: Resize Fortress to 2×2 and Update Health Bar

### Overview
Change fortress from full-height zone marker to a compact 2×2 building centered vertically. Update collider and health bar to match.

### Changes Required:

#### 1. Add fortress height constant
**File**: `src/gameplay/battlefield/mod.rs`
**Changes**: Add `FORTRESS_ROWS` constant after `FORTRESS_COLS`

```rust
/// Number of rows for each fortress (2x2 building).
pub const FORTRESS_ROWS: u16 = 2;
```

#### 2. Update health bar config constants
**File**: `src/gameplay/battlefield/mod.rs`
**Changes**: Update health bar dimensions for 2×2 fortress

```rust
/// Fortress health bar dimensions — sized for 2x2 fortress.
const FORTRESS_HEALTH_BAR_WIDTH: f32 = 100.0; // keep — looks good on 128px
const FORTRESS_HEALTH_BAR_HEIGHT: f32 = 6.0;  // keep
/// Y offset: above the 128px fortress. Half height (64) + padding (10) = 74.
const FORTRESS_HEALTH_BAR_Y_OFFSET: f32 = 74.0;
```

#### 3. Update fortress spawning in renderer
**File**: `src/gameplay/battlefield/renderer.rs`
**Changes**: Use `FORTRESS_ROWS` for fortress size, update import

Line 38 — change fortress size calculation:
```rust
let fortress_size = Vec2::new(
    f32::from(FORTRESS_COLS) * CELL_SIZE,
    f32::from(FORTRESS_ROWS) * CELL_SIZE,
);
```

Add `FORTRESS_ROWS` to imports from `super::`:
```rust
use super::{
    ..., FORTRESS_ROWS, ...
};
```

No other changes in renderer — the `zone_center_x()` and `battlefield_center_y()` position calculations remain correct. The collider already uses `fortress_size` so it updates automatically.

#### 4. Update tests
**File**: `src/gameplay/battlefield/mod.rs`

Update `spawn_battlefield_creates_expected_sprites` test — the entity count may stay the same (fortresses are still sprites). Verify the count is still 65 (2 fortresses + build zone + combat zone + background + 60 grid slots).

Add a test for fortress size:
```rust
#[test]
fn fortress_is_2x2_cells() {
    let mut app = create_battlefield_test_app();
    let mut query = app
        .world_mut()
        .query_filtered::<&Sprite, With<PlayerFortress>>();
    let sprite = query.single(app.world()).unwrap();
    let expected = Vec2::new(
        f32::from(FORTRESS_COLS) * CELL_SIZE,
        f32::from(FORTRESS_ROWS) * CELL_SIZE,
    );
    assert_eq!(sprite.custom_size.unwrap_or(expected), expected);
}
```

Note: `Sprite::from_color()` stores size in the `Sprite` component's internal `custom_size`. Check Bevy 0.18 source to verify the field name — it may be `rect` or `custom_size`. Adjust test accordingly.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (no clippy warnings)
- [ ] `make test` passes (all existing + new tests)
- [ ] `make build` passes

#### Manual Verification:
- [ ] Fortress appears as a small square centered vertically in its column
- [ ] Health bar sits above the fortress (not centered on it)
- [ ] Health bar updates correctly when fortress takes damage
- [ ] No visual regression in other zones

**Implementation Note**: Pause here for manual verification before proceeding.

---

## Phase 2: Fortress Combat Capability

### Overview
Give both fortresses the ability to target and attack enemy units. Generalize both `unit_find_target` and `unit_attack` to work on any entity with the right components — no separate fortress-specific systems.

### Changes Required:

#### 1. Add fortress combat constants
**File**: `src/gameplay/battlefield/mod.rs`
**Changes**: Add constants for fortress combat stats

```rust
// === Fortress Combat Stats ===

/// Fortress damage per projectile — high damage, slow rate.
pub const FORTRESS_DAMAGE: f32 = 50.0;

/// Fortress attacks per second — slow turret cadence.
pub const FORTRESS_ATTACK_SPEED: f32 = 0.5;

/// Fortress attack range in pixels (~3 cells).
pub const FORTRESS_RANGE: f32 = 200.0;
```

#### 2. Add combat components to fortress spawn
**File**: `src/gameplay/battlefield/renderer.rs`
**Changes**: Import and add `CombatStats`, `AttackTimer`, `CurrentTarget` to both fortress spawns

Add imports:
```rust
use crate::gameplay::combat::AttackTimer;
use crate::gameplay::units::{CombatStats, CurrentTarget};
```

Add to `super::` imports:
```rust
use super::{
    ..., FORTRESS_ATTACK_SPEED, FORTRESS_DAMAGE, FORTRESS_RANGE, ...
};
```

Add these components to BOTH fortress spawns (player and enemy), after `HealthBarConfig`:
```rust
CombatStats {
    damage: FORTRESS_DAMAGE,
    attack_speed: FORTRESS_ATTACK_SPEED,
    range: FORTRESS_RANGE,
},
AttackTimer(Timer::from_seconds(
    1.0 / FORTRESS_ATTACK_SPEED,
    TimerMode::Repeating,
)),
CurrentTarget(None),
```

#### 3. Generalize `unit_attack` — remove `With<Unit>` filter
**File**: `src/gameplay/combat/attack.rs`
**Changes**: Remove the `With<Unit>` filter from the `unit_attack` query

Before (line 63):
```rust
    ),
    With<Unit>,
>,
```

After:
```rust
    ),
>,
```

Also remove the `Unit` import (line 6):
```rust
// Before
use crate::gameplay::units::{CombatStats, CurrentTarget, Unit};
// After
use crate::gameplay::units::{CombatStats, CurrentTarget};
```

The system is private and only called within the combat plugin chain. Removing the filter means any entity with `(CurrentTarget, CombatStats, AttackTimer, GlobalTransform, Collider, Team)` can attack — units, fortresses, and future turret buildings.

#### 4. Generalize `unit_find_target` — remove `With<Unit>`, conditional backtrack
**File**: `src/gameplay/units/ai.rs`
**Changes**: Rename to `find_target`, remove `With<Unit>` filter, add `Option<&Movement>` for conditional backtrack

Before:
```rust
pub(super) fn unit_find_target(
    mut counter: Local<u32>,
    mut units: Query<
        (
            Entity,
            &Team,
            &GlobalTransform,
            &Collider,
            &mut CurrentTarget,
        ),
        With<Unit>,
    >,
    all_targets: Query<(Entity, &Team, &GlobalTransform, &Collider), With<Target>>,
) {
```

After:
```rust
pub(super) fn find_target(
    mut counter: Local<u32>,
    mut seekers: Query<
        (
            Entity,
            &Team,
            &GlobalTransform,
            &Collider,
            &mut CurrentTarget,
            Option<&Movement>,
        ),
    >,
    all_targets: Query<(Entity, &Team, &GlobalTransform, &Collider), With<Target>>,
) {
```

And the inner loop changes — backtrack only applies to mobile entities:
```rust
    for (entity, team, transform, collider, mut current_target, movement) in &mut seekers {
        // ... stagger retarget unchanged ...

        for (candidate, candidate_team, candidate_pos, candidate_collider) in &all_targets {
            // ... team check unchanged ...

            // Backtrack filter: only applies to moving entities (units)
            if movement.is_some() {
                let candidate_xy = candidate_pos.translation().xy();
                let behind = match team {
                    Team::Player => my_pos.x - candidate_xy.x,
                    Team::Enemy => candidate_xy.x - my_pos.x,
                };
                if behind > BACKTRACK_DISTANCE {
                    continue;
                }
            }

            // ... surface_distance + nearest check unchanged ...
        }
    }
```

Remove the `Unit` import (no longer needed in this file):
```rust
// Before
use super::{BACKTRACK_DISTANCE, CurrentTarget, Unit};
// After
use super::{BACKTRACK_DISTANCE, CurrentTarget, Movement};
```

#### 5. Update `find_target` registration
**File**: `src/gameplay/units/mod.rs`
**Changes**: Rename the system reference

```rust
// Before
ai::unit_find_target
// After
ai::find_target
```

No new system registration needed in `battlefield/` — the generalized system in `units/` handles everything.

#### 6. Update `unit_attack` tests
**File**: `src/gameplay/combat/attack.rs`
**Changes**: Existing tests use `Unit` component on attackers. Since we removed `With<Unit>` from the query, the tests still work — they spawn entities with `Unit` but the filter no longer requires it. Tests pass unchanged.

Add a new test for fortress attacking:
```rust
#[test]
fn fortress_can_attack_in_range() {
    let mut app = create_attack_test_app();

    // Spawn a "fortress-like" entity (no Unit marker)
    let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
    timer.set_elapsed(Duration::from_nanos(999_000));
    let fortress = app.world_mut().spawn((
        Team::Player,
        CurrentTarget(None),
        CombatStats {
            damage: 50.0,
            attack_speed: 0.5,
            range: 200.0,
        },
        AttackTimer(timer),
        Transform::from_xyz(64.0, 320.0, 0.0),
        GlobalTransform::from(Transform::from_xyz(64.0, 320.0, 0.0)),
        Collider::rectangle(128.0, 128.0),
    )).id();

    let target = spawn_target(app.world_mut(), 200.0, 100.0);

    // Set fortress target
    app.world_mut().get_mut::<CurrentTarget>(fortress).unwrap().0 = Some(target);

    advance_and_update(&mut app, Duration::from_millis(100));
    assert_entity_count::<With<Projectile>>(&mut app, 1);
}
```

#### 7. Update `find_target` tests
**File**: `src/gameplay/units/ai.rs`
**Changes**: Update existing tests (they still pass since units have `Movement`, so backtrack still applies). Rename references from `unit_find_target` to `find_target`.

Add new tests for fortress targeting via the generalized system:

```rust
#[test]
fn fortress_targets_nearest_enemy() {
    let mut app = create_ai_test_app();

    // Spawn a fortress-like entity (no Unit, no Movement — static)
    let fortress = app.world_mut().spawn((
        Team::Player,
        Target,
        CurrentTarget(None),
        Transform::from_xyz(64.0, 320.0, 0.0),
        GlobalTransform::from(Transform::from_xyz(64.0, 320.0, 0.0)),
        Collider::rectangle(128.0, 128.0),
    )).id();

    // Spawn two enemy targets
    let near_enemy = spawn_target(app.world_mut(), Team::Enemy, 200.0, 320.0);
    let _far_enemy = spawn_target(app.world_mut(), Team::Enemy, 500.0, 320.0);

    app.update();

    let ct = app.world().get::<CurrentTarget>(fortress).unwrap();
    assert_eq!(ct.0, Some(near_enemy));
}

#[test]
fn static_entity_has_no_backtrack_limit() {
    let mut app = create_ai_test_app();

    // Fortress at x=500 with enemy "behind" at x=100 (would be filtered for units)
    let fortress = app.world_mut().spawn((
        Team::Player,
        Target,
        CurrentTarget(None),
        Transform::from_xyz(500.0, 320.0, 0.0),
        GlobalTransform::from(Transform::from_xyz(500.0, 320.0, 0.0)),
        Collider::rectangle(128.0, 128.0),
    )).id();

    let behind_enemy = spawn_target(app.world_mut(), Team::Enemy, 100.0, 320.0);

    app.update();

    // Static entity (no Movement) should target regardless of direction
    let ct = app.world().get::<CurrentTarget>(fortress).unwrap();
    assert_eq!(ct.0, Some(behind_enemy));
}
```

#### 8. Add fortress component integration tests
**File**: `src/gameplay/battlefield/mod.rs` (integration_tests section)

```rust
#[test]
fn fortress_has_combat_stats() {
    let mut app = create_battlefield_test_app();
    use crate::gameplay::units::CombatStats;
    assert_entity_count::<(With<PlayerFortress>, With<CombatStats>)>(&mut app, 1);
    assert_entity_count::<(With<EnemyFortress>, With<CombatStats>)>(&mut app, 1);
}

#[test]
fn fortress_has_current_target() {
    let mut app = create_battlefield_test_app();
    use crate::gameplay::units::CurrentTarget;
    assert_entity_count::<(With<PlayerFortress>, With<CurrentTarget>)>(&mut app, 1);
    assert_entity_count::<(With<EnemyFortress>, With<CurrentTarget>)>(&mut app, 1);
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes (all existing + new tests)
- [ ] `make build` passes

#### Manual Verification:
- [ ] Fortresses fire projectiles at enemy units within range
- [ ] Projectiles originate from fortress position
- [ ] Fortress correctly targets nearest enemy
- [ ] Fortress stops attacking when no enemies in range
- [ ] Both player and enemy fortress attack
- [ ] No friendly fire from fortress projectiles
- [ ] Endgame detection still works (fortress can still die)

**Implementation Note**: Pause here for manual verification before proceeding.

---

## Phase 3: Enemy Spawning from Fortress Position

### Overview
Change enemy unit spawning to read the enemy fortress entity's position instead of a hardcoded column constant. If the fortress is destroyed, spawning stops naturally.

### Changes Required:

#### 1. Update enemy spawner to read fortress position
**File**: `src/gameplay/units/spawn.rs`
**Changes**: Replace `ENEMY_SPAWN_COL` constant with fortress entity query

Remove:
```rust
use crate::gameplay::battlefield::{
    BATTLEFIELD_ROWS, ENEMY_FORT_START_COL, col_to_world_x, row_to_world_y,
};
```
```rust
const ENEMY_SPAWN_COL: u16 = ENEMY_FORT_START_COL;
```

Replace with:
```rust
use crate::gameplay::battlefield::{
    BATTLEFIELD_ROWS, CELL_SIZE, EnemyFortress, row_to_world_y,
};
```

Update `tick_enemy_spawner` to take a fortress query:
```rust
fn tick_enemy_spawner(
    time: Res<Time>,
    mut spawn_timer: ResMut<EnemySpawnTimer>,
    unit_assets: Res<UnitAssets>,
    enemy_fortress: Single<&Transform, With<EnemyFortress>>,
    mut commands: Commands,
) {
    spawn_timer.elapsed_secs += time.delta_secs();
    spawn_timer.timer.tick(time.delta());

    if !spawn_timer.timer.just_finished() {
        return;
    }

    let fortress_pos = enemy_fortress.translation;

    // Spawn at fortress X, random Y within ±1 cell of fortress center
    let row = rand::rng().random_range(0..BATTLEFIELD_ROWS);
    let spawn_x = fortress_pos.x;
    let spawn_y = row_to_world_y(row);

    super::spawn_unit(
        &mut commands,
        super::UnitType::Soldier,
        Team::Enemy,
        Vec3::new(spawn_x, spawn_y, Z_UNIT),
        &unit_assets,
    );

    // Set next spawn interval
    let next_interval = current_interval(spawn_timer.elapsed_secs);
    spawn_timer.timer = Timer::from_seconds(next_interval, TimerMode::Once);
}
```

Using `Single<&Transform, With<EnemyFortress>>` means:
- If fortress exists: system runs normally, reads position
- If fortress is destroyed (despawned): system is silently skipped — no more enemies spawn

This is correct behavior: destroying the enemy fortress should stop enemy reinforcements.

#### 2. Update spawn tests
**File**: `src/gameplay/units/spawn.rs`

The integration tests in `create_spawn_test_app()` currently don't spawn a fortress entity. With the `Single` query, the spawner system will be skipped unless an enemy fortress exists in the test world.

Update `create_spawn_test_app()` to spawn an enemy fortress entity:
```rust
fn create_spawn_test_app() -> App {
    let mut app = crate::testing::create_base_test_app();
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<ColorMaterial>>();

    app.add_systems(OnEnter(GameState::InGame), super::super::setup_unit_assets);
    plugin(&mut app);
    transition_to_ingame(&mut app);

    // Spawn a mock enemy fortress for the spawner to read position from
    app.world_mut().spawn((
        crate::gameplay::battlefield::EnemyFortress,
        crate::gameplay::Team::Enemy,
        Transform::from_xyz(5152.0, 320.0, 0.0), // Approx col 80 center
    ));
    app
}
```

Add a test for spawn-from-fortress behavior:
```rust
#[test]
fn enemy_spawns_at_fortress_x_position() {
    let mut app = create_spawn_test_app();

    nearly_expire_timer(&mut app);
    app.update();

    let mut query = app
        .world_mut()
        .query_filtered::<&Transform, With<Unit>>();
    let unit_transform = query.single(app.world()).unwrap();
    // Should spawn at fortress X (5152.0)
    assert!(
        (unit_transform.translation.x - 5152.0).abs() < f32::EPSILON,
        "Unit should spawn at fortress X, got {}",
        unit_transform.translation.x,
    );
}
```

Add a test for fortress-destroyed behavior:
```rust
#[test]
fn no_enemies_spawn_when_fortress_destroyed() {
    let mut app = create_spawn_test_app();

    // Despawn the enemy fortress
    let mut fortress_query = app
        .world_mut()
        .query_filtered::<Entity, With<crate::gameplay::battlefield::EnemyFortress>>();
    let fortress = fortress_query.single(app.world()).unwrap();
    app.world_mut().despawn(fortress);

    // Nearly expire timer and update — system should be skipped
    nearly_expire_timer(&mut app);
    app.update();

    assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 0);
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes (all existing + new tests)
- [ ] `make build` passes

#### Manual Verification:
- [ ] Enemy units spawn from the enemy fortress position (not from the right edge)
- [ ] When enemy fortress is destroyed, no more enemies spawn
- [ ] Enemy units still move left toward player fortress after spawning
- [ ] Spawn timing/ramping still works correctly

---

## Testing Strategy

### Unit Tests:
- Fortress combat stat constants are positive
- `FORTRESS_ROWS` is 2

### Integration Tests:
- Fortress spawns with correct size (128×128)
- Fortress has CombatStats, AttackTimer, CurrentTarget
- Fortress health bar config has correct y_offset
- Generalized `find_target` targets nearest enemy for fortress (no backtrack)
- Static entities (no Movement) skip backtrack filter
- Units still respect backtrack limit (existing tests pass)
- Generalized `unit_attack` fires projectiles from fortress entities
- Enemy units spawn at fortress X position
- No spawns when fortress is destroyed
- Endgame detection still works with resized fortress

### Manual Testing Steps:
1. Start a game → both fortresses appear as small squares centered vertically
2. Wait for enemy units to approach player fortress → fortress fires projectiles
3. Place barracks → player units cross and approach enemy fortress → enemy fortress fires
4. Wait for units to destroy enemy fortress → Victory screen, enemies stop spawning
5. Let enemies destroy player fortress → Defeat screen

## Verified API Patterns (Bevy 0.18)

- `Single<D, F>` — skips system if 0 or >1 matches. Perfect for fortress query in spawner.
- `Option<&T>` in query — returns `Some` if component exists, `None` otherwise. Perfect for conditional backtrack.
- `Sprite::from_color(color, size)` — stores size internally, collider uses `fortress_size` variable.
- `Timer::from_seconds(duration, TimerMode::Repeating)` — for attack cooldown.
- `surface_distance()` — in `crate::third_party`, uses GJK contact query. Works with circles and rectangles.

## Performance Considerations

- Generalized `find_target` adds 2 fortress entities to existing query — negligible overhead
- Generalized `unit_attack` adds 2 fortress entities — negligible
- Staggered retargeting still applies (frame counter + entity index) — fortresses benefit from same optimization
- No new systems, no new per-frame allocations

## References

- Linear ticket: [GAM-15](https://linear.app/tayhu-games/issue/GAM-15/overhaul-fortress)
- Fortress spawning: `src/gameplay/battlefield/renderer.rs:56-142`
- Attack system: `src/gameplay/combat/attack.rs:52-115`
- Unit AI: `src/gameplay/units/ai.rs:20-77`
- Enemy spawner: `src/gameplay/units/spawn.rs:78-107`
- Health bar system: `src/gameplay/combat/health_bar.rs:47-73`

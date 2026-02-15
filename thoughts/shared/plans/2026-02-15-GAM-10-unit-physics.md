# GAM-10: Add Unit Physics — Implementation Plan

## Overview

Add `avian2d` as the physics engine so units, buildings, and fortresses have collision bodies. Units will push each other apart instead of stacking, and will navigate around static obstacles (buildings, fortresses). The movement system switches from direct Transform manipulation to velocity-based movement.

## Current State Analysis

- **Movement**: `unit_movement()` directly modifies `Transform.translation` each frame (`movement.rs:41-42`)
- **No physics**: entities walk through each other and through buildings
- **Entity dimensions**:
  - Units: circle, `UNIT_RADIUS = 12.0` px (`units/mod.rs:20`)
  - Buildings: rectangle, `BUILDING_SPRITE_SIZE = 60.0` px (`building/mod.rs:19`)
  - Fortresses: rectangle, 128px × 640px (2 cols × 10 rows) (`battlefield/renderer.rs:35`)
- **Data-driven spawning**: `spawn_unit()` (`units/mod.rs:110`) and `handle_building_placement()` (`building/placement.rs:117`) are the single sources of truth for entity archetypes

### Key Discoveries:
- `spawn_unit()` at `units/mod.rs:110-155` is the single spawn function for both player and enemy units
- Fortress spawning at `battlefield/renderer.rs:54-125` creates two fortress entities with Health, Target, Team
- Movement system at `movement.rs:9-45` uses `time.delta_secs()` for frame-rate-independent movement
- Projectile movement at `combat/attack.rs:100-132` also uses direct Transform manipulation — stays unchanged (projectiles don't need physics)
- Existing test apps use `MinimalPlugins` — will need `PhysicsPlugins` added for movement/collision tests

## Desired End State

After this plan is complete:

1. Units have `RigidBody::Dynamic` + `Collider::circle(UNIT_RADIUS)` — they push each other apart
2. Buildings have `RigidBody::Static` + `Collider::rectangle(...)` — immovable obstacles
3. Fortresses have `RigidBody::Static` + `Collider::rectangle(...)` — immovable obstacles
4. Movement system sets `LinearVelocity` instead of modifying Transform directly
5. `src/third_party/avian2d.rs` isolates physics configuration per ARCHITECTURE.md
6. Physics debug visualization available under `dev` feature
7. All existing tests pass, new physics-specific tests added
8. 90% coverage maintained

### How to verify:
- `make check` passes (clippy + compile)
- `make test` passes (all existing + new tests)
- Manual: run the game, spawn units, observe they push apart instead of stacking
- Manual: place buildings, observe units navigate around them (basic collision avoidance, not pathfinding)

## What We're NOT Doing

- **Pathfinding** — that's GAM-12. Units will bump into obstacles and slide along them, not intelligently route around
- **Sensor colliders for combat range** — current distance-based range checks work fine
- **Collision layers** — all entities collide with all for now. Infrastructure for team-based filtering deferred to GAM-12
- **Resizing entities** — that's GAM-20. Keeping current dimensions
- **Fortress rework** — that's GAM-15. Just adding colliders to existing fortress entities

## Implementation Approach

1. Add avian2d dependency and third-party isolation module
2. Add physics components to all entity spawning code
3. Rewrite movement system to velocity-based
4. Update tests to work with physics

## Verified API Patterns (avian2d 0.5 / Bevy 0.18)

Verified against docs.rs/avian2d/0.5:

- **Plugin**: `PhysicsPlugins::default().with_length_unit(64.0)` — 64px = 1 meter (cell size)
- **Gravity**: `Gravity(Vec2::ZERO)` or `Gravity::ZERO` — top-down game, no falling
- **RigidBody**: enum with `Dynamic`, `Static`, `Kinematic` variants. Derives `Component`
- **Collider**: `Collider::circle(radius)`, `Collider::rectangle(x_length, y_length)` — constructors take full dimensions (not half-extents)
- **LinearVelocity**: `LinearVelocity(Vec2)` tuple struct. Derives `Component`, `Deref`, `DerefMut`. Has `LinearVelocity::ZERO`
- **LockedAxes**: `LockedAxes::ROTATION_LOCKED` constant locks rotation, allows translation
- **PhysicsDebugPlugin**: available via `debug-plugin` feature (enabled by default)
- **Prelude**: `use avian2d::prelude::*` re-exports all common types
- **Schedule**: physics runs in `FixedPostUpdate` by default, after our `Update` systems

---

## Phase 1: Dependencies & Third-Party Isolation

### Overview
Add avian2d crate, create the third-party isolation module, configure physics for a top-down game.

### Changes Required:

#### 1. Cargo.toml
**File**: `Cargo.toml`
**Changes**: Add avian2d dependency

```toml
[dependencies]
bevy = { version = "0.18", default-features = false, features = ["2d"] }
avian2d = { version = "0.5", default-features = false, features = ["2d", "f32", "debug-plugin"] }
rand = "0.9"
```

Note: Use `default-features = false` with explicit feature selection, matching the project's Bevy convention. The `debug-plugin` feature enables `PhysicsDebugPlugin` for visual debugging.

#### 2. Third-Party Module
**File**: `src/third_party/mod.rs` (NEW)

```rust
//! Third-party plugin isolation.

mod avian;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(avian::plugin);
}
```

#### 3. Avian Physics Configuration
**File**: `src/third_party/avian.rs` (NEW)

```rust
//! Avian2d physics configuration for top-down gameplay.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::gameplay::battlefield::CELL_SIZE;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(
        PhysicsPlugins::default().with_length_unit(CELL_SIZE),
    );
    app.insert_resource(Gravity::ZERO);

    #[cfg(feature = "dev")]
    app.add_plugins(PhysicsDebugPlugin::default());
}
```

#### 4. Wire into lib.rs
**File**: `src/lib.rs`
**Changes**: Add `third_party` module and plugin

Add module declaration:
```rust
pub(crate) mod third_party;
```

Add to compositor:
```rust
app.add_plugins((
    third_party::plugin,
    ui_camera::plugin,
    screens::plugin,
    menus::plugin,
    gameplay::plugin,
    theme::plugin,
));
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` compiles successfully with avian2d
- [x] `make check` passes (clippy + compile)
- [x] `make test` passes (all existing tests still work)

#### Manual Verification:
- [ ] Game runs (physics debug overlay deferred — requires render pipeline; add `PhysicsDebugPlugin` in main.rs when needed)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding.

---

## Phase 2: Add Physics Components to Entities

### Overview
Add RigidBody, Collider, LockedAxes, and LinearVelocity to all game entities. No behavior changes yet — movement system still uses direct Transform manipulation (Phase 3 changes that).

### Changes Required:

#### 1. Unit Spawning
**File**: `src/gameplay/units/mod.rs`
**Changes**: Add physics components to `spawn_unit()` and add avian2d import

Add import at top of file:
```rust
use avian2d::prelude::*;
```

Add physics components to the spawn tuple (after `DespawnOnExit`):
```rust
// Physics
RigidBody::Dynamic,
Collider::circle(UNIT_RADIUS),
LockedAxes::ROTATION_LOCKED,
LinearVelocity::ZERO,
```

#### 2. Building Spawning
**File**: `src/gameplay/building/placement.rs`
**Changes**: Add physics components to `handle_building_placement()`

Add import at top of file:
```rust
use avian2d::prelude::*;
```

Add physics components to the building spawn tuple (after `DespawnOnExit`):
```rust
// Physics
RigidBody::Static,
Collider::rectangle(BUILDING_SPRITE_SIZE, BUILDING_SPRITE_SIZE),
```

#### 3. Fortress Spawning
**File**: `src/gameplay/battlefield/renderer.rs`
**Changes**: Add physics components to both fortress spawns

Add import at top of file:
```rust
use avian2d::prelude::*;
```

For **Player Fortress** (line 54-72), add after `DespawnOnExit`:
```rust
// Physics
RigidBody::Static,
Collider::rectangle(fortress_size.x, fortress_size.y),
```

For **Enemy Fortress** (line 107-125), add the same:
```rust
// Physics
RigidBody::Static,
Collider::rectangle(fortress_size.x, fortress_size.y),
```

#### 4. Update Entity Archetype Documentation
**File**: `src/gameplay/mod.rs`
**Changes**: Update the entity archetype comments at the top to include physics components

```
// Entity archetypes (components per entity type):
// Units: Unit, UnitType, Team, Target, CurrentTarget, Health, CombatStats,
//        Movement, AttackTimer, HealthBarConfig, Mesh2d, MeshMaterial2d,
//        RigidBody::Dynamic, Collider, LockedAxes, LinearVelocity
// Buildings: Building, Team, Target, Health, HealthBarConfig,
//            ProductionTimer/IncomeTimer, Sprite,
//            RigidBody::Static, Collider
// Fortresses: PlayerFortress/EnemyFortress, Team, Target, Health,
//             HealthBarConfig, Sprite, RigidBody::Static, Collider
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` compiles successfully
- [x] `make check` passes
- [x] `make test` passes (all 164 tests green, no adjustments needed)

#### Manual Verification:
- [ ] Game runs, colliders visible when PhysicsDebugPlugin added to main.rs
- [ ] Units still move (using old Transform-based movement — Phase 3 changes this)
- [ ] Units visibly push each other apart when spawning at overlapping positions
- [ ] Buildings block unit movement (units slide along building edges instead of walking through)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding.

---

## Phase 3: Velocity-Based Movement

### Overview
Rewrite the movement system to set `LinearVelocity` instead of directly modifying `Transform`. The physics engine now handles position updates and collision resolution.

### Changes Required:

#### 1. Rewrite Movement System
**File**: `src/gameplay/units/movement.rs`
**Changes**: Replace Transform manipulation with LinearVelocity

```rust
//! Unit movement toward current target.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{CombatStats, CurrentTarget, Movement, Unit};

/// Sets unit `LinearVelocity` toward their `CurrentTarget`, stopping at attack range.
/// The physics engine handles actual position updates and collision resolution.
/// Runs in `GameSet::Movement`.
pub(super) fn unit_movement(
    mut units: Query<
        (
            &CurrentTarget,
            &Movement,
            &CombatStats,
            &GlobalTransform,
            &mut LinearVelocity,
        ),
        With<Unit>,
    >,
    positions: Query<&GlobalTransform>,
) {
    for (current_target, movement, stats, global_transform, mut velocity) in &mut units {
        let Some(target_entity) = current_target.0 else {
            velocity.0 = Vec2::ZERO;
            continue;
        };
        let Ok(target_pos) = positions.get(target_entity) else {
            velocity.0 = Vec2::ZERO;
            continue;
        };

        let target_xy = target_pos.translation().xy();
        let current_xy = global_transform.translation().xy();
        let diff = target_xy - current_xy;
        let distance = diff.length();

        // Already within attack range — stop
        if distance <= stats.range {
            velocity.0 = Vec2::ZERO;
            continue;
        }

        // Avoid division by near-zero
        if distance < f32::EPSILON {
            velocity.0 = Vec2::ZERO;
            continue;
        }

        let direction = diff / distance; // normalized
        velocity.0 = direction * movement.speed;
    }
}
```

Key changes from old system:
- **No `Res<Time>`** — physics handles time integration
- **No `&mut Transform`** — replaced with `&mut LinearVelocity`
- **Uses `&GlobalTransform`** — reads position from physics-managed transform instead of writing to it
- **No overshoot detection** — physics integrator handles smooth movement; velocity is set to zero when within range
- **Explicit zero velocity** when no target or target invalid — prevents units from drifting

#### 2. No changes to projectile movement
**File**: `src/gameplay/combat/attack.rs`
Projectiles continue using direct Transform manipulation. They don't have physics components and shouldn't interact with the physics engine. No changes needed.

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` compiles
- [x] `make check` passes
- [x] `make test` passes (movement tests rewritten inline with Phase 3)

#### Manual Verification:
- [ ] Units move toward enemies smoothly
- [ ] Units stop at attack range and begin attacking
- [ ] Units push each other apart when converging on the same target
- [ ] Units slide along buildings/fortresses instead of walking through
- [ ] No visible jitter or oscillation at attack range boundary
- [ ] Game still plays correctly end-to-end (waves spawn, units fight, fortresses take damage)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding.

---

## Phase 4: Test Updates

### Overview
Update existing movement tests to work with velocity-based movement, and add new physics-specific tests. Other tests (placement, AI, combat) should work without changes or with minor physics plugin additions.

### Changes Required:

#### 1. Rewrite Movement Tests
**File**: `src/gameplay/units/movement.rs` (test module)
**Changes**: Tests now verify `LinearVelocity` output instead of Transform changes

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::units::{UnitType, unit_stats};

    fn create_movement_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, unit_movement);
        app.update(); // Initialize time
        app
    }

    fn spawn_unit_at(
        world: &mut World,
        x: f32,
        speed: f32,
        target: Option<Entity>,
    ) -> Entity {
        let stats = unit_stats(UnitType::Soldier);
        world
            .spawn((
                Unit,
                CurrentTarget(target),
                Movement { speed },
                CombatStats {
                    damage: stats.damage,
                    attack_speed: stats.attack_speed,
                    range: stats.attack_range,
                },
                Transform::from_xyz(x, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, 100.0, 0.0)),
                LinearVelocity::ZERO,
            ))
            .id()
    }

    fn spawn_target_at(world: &mut World, x: f32) -> Entity {
        world
            .spawn((
                Transform::from_xyz(x, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, 100.0, 0.0)),
            ))
            .id()
    }

    #[test]
    fn unit_sets_velocity_toward_target() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        // Velocity should point right (positive x) toward target
        assert!(
            velocity.x > 0.0,
            "Velocity x should be positive toward target, got {}",
            velocity.x
        );
        // Magnitude should be approximately move_speed
        let speed = velocity.0.length();
        assert!(
            (speed - stats.move_speed).abs() < 0.1,
            "Velocity magnitude should be ~{}, got {}",
            stats.move_speed,
            speed
        );
    }

    #[test]
    fn unit_stops_at_attack_range() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        // Place unit within attack range
        let unit = spawn_unit_at(
            app.world_mut(),
            500.0 - stats.attack_range + 1.0,
            stats.move_speed,
            Some(target),
        );

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Unit within range should have zero velocity, got {:?}",
            velocity.0
        );
    }

    #[test]
    fn unit_zero_velocity_without_target() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, None);

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Unit with no target should have zero velocity, got {:?}",
            velocity.0
        );
    }

    #[test]
    fn unit_zero_velocity_when_target_despawned() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        // Despawn the target
        app.world_mut().despawn(target);
        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Unit with despawned target should have zero velocity, got {:?}",
            velocity.0
        );
    }

    #[test]
    fn unit_velocity_direction_is_normalized() {
        let mut app = create_movement_test_app();

        // Target at a diagonal — velocity direction should be normalized * speed
        let target = spawn_target_at(app.world_mut(), 400.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, 50.0, Some(target));

        // Move unit to different Y to create diagonal
        app.world_mut()
            .get_mut::<Transform>(unit)
            .unwrap()
            .translation
            .y = 200.0;
        app.world_mut()
            .get_mut::<GlobalTransform>(unit)
            .unwrap()
            .0 = Transform::from_xyz(100.0, 200.0, 0.0).into();

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        let speed = velocity.0.length();
        assert!(
            (speed - 50.0).abs() < 0.1,
            "Velocity magnitude should be 50.0, got {}",
            speed
        );
    }
}
```

#### 2. Fix Test Apps That Need Physics (if needed)
Test apps that register full domain plugins and trigger `OnEnter(GameState::InGame)` might need `PhysicsPlugins` if avian systems panic without the physics schedule. The test helpers in `testing.rs` may need a physics-aware variant:

**File**: `src/testing.rs`
**Changes**: Add physics-aware test app helper if needed

```rust
/// Creates a base test app with physics support.
/// Use for tests that exercise the full gameplay pipeline including physics.
#[allow(dead_code)]
pub fn create_base_test_app_with_physics() -> App {
    let mut app = create_base_test_app();
    app.add_plugins(
        avian2d::prelude::PhysicsPlugins::default()
            .with_length_unit(crate::gameplay::battlefield::CELL_SIZE),
    );
    app.insert_resource(avian2d::prelude::Gravity::ZERO);
    app
}
```

This is a "if needed" change — only add if tests fail without physics plugins. The approach: start without this helper, see which tests break, and add it only if necessary.

#### 3. Fix dead_code warning
**File**: `src/testing.rs`
**Changes**: The `tick` function at line 90 has a dead_code warning. While we're touching this file, either add `#[allow(dead_code)]` or remove it if `tick_multiple` supersedes it.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (zero warnings)
- [x] `make test` passes (all tests green)
- [ ] Test coverage >= 90% (check with `cargo tarpaulin` or similar if available)

#### Manual Verification:
- [x] Full game loop works: start game, build barracks/farms, units spawn, fight enemies, win/lose
- [x] No regression from pre-physics behavior

---

## Testing Strategy

### Unit Tests:
- Movement system sets correct `LinearVelocity` direction and magnitude
- Movement system sets zero velocity when within range
- Movement system sets zero velocity when target is None or despawned
- Velocity magnitude equals `Movement.speed` when moving

### Integration Tests:
- Spawned units have all physics components (RigidBody, Collider, LinearVelocity, LockedAxes)
- Spawned buildings have RigidBody::Static and Collider
- Spawned fortresses have RigidBody::Static and Collider

### Manual Testing Steps:
1. Run game, let enemy units spawn — verify they don't stack on top of each other
2. Place buildings in a row — verify units slide along building edges
3. Spawn many units at once — verify performance is acceptable (no frame drops)
4. Play a full game to victory/defeat — verify no regressions
5. Check physics debug overlay — verify collider shapes match visual sprites

## Performance Considerations

- **`with_length_unit(CELL_SIZE)`**: scales internal tolerances so 64px = 1 meter. This ensures collision detection precision is appropriate for our entity sizes
- **LockedAxes::ROTATION_LOCKED**: prevents unnecessary rotation calculations for units
- **Static rigid bodies**: buildings and fortresses use `RigidBody::Static`, which is the most efficient (no velocity integration, no force application)

## Migration Notes

- No data migration needed — purely additive change (new components on entities)
- Existing save/replay systems (if any): N/A, no persistence yet
- The movement system signature changes, which affects any code that references `unit_movement` by name in tests or plugin registration

## References

- Linear ticket: [GAM-10](https://linear.app/tayhu-games/issue/GAM-10/add-unit-physics)
- Blocks: [GAM-12](https://linear.app/tayhu-games/issue/GAM-12/unit-pathfinding-based-on-physics-objects) (pathfinding), [GAM-20](https://linear.app/tayhu-games/issue/GAM-20/decrese-unit-and-building-size-so-that-pathing-will-be-possible) (sizing)
- Related: [GAM-15](https://linear.app/tayhu-games/issue/GAM-15/overhaul-fortress) (fortress overhaul benefits from physics colliders)
- ARCHITECTURE.md: Third-party isolation pattern (section "Third-Party Plugin Isolation")
- avian2d docs: https://docs.rs/avian2d/0.5/

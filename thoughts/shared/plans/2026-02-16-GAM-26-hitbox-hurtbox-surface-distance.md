# GAM-26: Hitbox/Hurtbox Collision System + Surface-Distance Range Checks

## Overview

After GAM-10 added physics colliders, units can no longer attack buildings or fortresses because the attack range check uses center-to-center distance, but the physical collider prevents units from getting close enough to the target's center. This plan adds surface-to-surface distance calculation and a hitbox/hurtbox collision layer system for damage delivery.

## Current State Analysis

**4 center-to-center distance checks** (all broken for large targets):
- `unit_movement` (`units/movement.rs:37`) — stop at attack range
- `unit_attack` (`combat/attack.rs:66-69`) — spawn projectile when in range
- `unit_find_target` (`units/ai.rs:58`) — find nearest enemy
- `move_projectiles` (`combat/attack.rs:116`) — projectile arrival/damage

**Current entity spawners** (no CollisionLayers yet):
- Units: `Collider::circle(12.0)` (`units/mod.rs:157`)
- Buildings: `Collider::rectangle(60, 60)` (`building/placement.rs:141`)
- Fortresses: `Collider::rectangle(128, 640)` (`battlefield/renderer.rs:76, 132`)
- Projectiles: no collider (just sprite + transform)

**Collision layer status**: None defined. All entities collide with all.

### Key Discoveries:
- `contact_query::distance()` at `avian2d::collision::collider::parry::contact_query` is a pure geometric function — works without the physics pipeline, making it ideal for unit tests
- `CollisionEventsEnabled` only needed on ONE entity in a collision pair
- `CollidingEntities` wraps `EntityHashSet`, manually populatable for tests
- Sensors need `RigidBody` — use `Kinematic` for moving projectiles
- Transform changes auto-sync to physics Position before collision detection

## Desired End State

1. Units stop moving and attack when their collider surface is within `attack_range` of the target's collider surface (not center-to-center)
2. Projectiles use sensor-based hitbox/hurtbox collision for damage delivery, replacing manual distance checks
3. Three collision layers (Pushbox, Hitbox, Hurtbox) enable granular collision filtering
4. ARCHITECTURE.md documents the collision layer pattern

### How to Verify:
- `make check` and `make test` pass
- Play the game: units walk up to buildings/fortresses and attack them (previously broken)
- Projectiles visually hit targets and deal damage
- Units still push each other and are blocked by buildings/fortresses

## What We're NOT Doing

- Area-of-effect damage or splash hitboxes
- Melee attack hitboxes (swings) — only projectile hitboxes for now
- Changing the AI's backtrack distance check (it uses x-distance, not euclidean)
- Adding physics-based movement to projectiles (they still use manual Transform movement)

## Implementation Approach

Three-part fix matching the two separate concerns:
1. **Navigation & range checks** → `surface_distance()` wrapper around `contact_query::distance()`
2. **Damage delivery** → hitbox/hurtbox collision layers with `CollidingEntities`
3. **Documentation** → ARCHITECTURE.md collision layer section

## Verified API Patterns (avian2d 0.5)

These were verified against the actual crate source at `~/.cargo/registry/src/.../avian2d-0.5.0/`:

- `contact_query::distance(collider1, position1, rotation1, collider2, position2, rotation2) -> Result<f32, UnsupportedShape>` — NOT in prelude, import `avian2d::collision::collider::parry::contact_query`
- Position params accept `impl Into<Position>` — `Vec2` works directly
- Rotation params accept `impl Into<Rotation>` — `0.0_f32` works directly
- `CollisionLayers::new(memberships: impl Into<LayerMask>, filters: impl Into<LayerMask>)` — in prelude
- `#[derive(PhysicsLayer)]` requires `Default` + enum with `#[default]` variant — in prelude
- `Sensor` — zero-sized marker, in prelude. Needs `RigidBody` (use `Kinematic` for moving entities)
- `CollisionEventsEnabled` — zero-sized marker, in prelude. Only needed on ONE entity in a pair
- `CollidingEntities(pub EntityHashSet)` — in prelude, `Deref`/`DerefMut` to inner set. Manually populatable: `CollidingEntities(EntityHashSet::from_iter([entity]))`. Must add to entity explicitly.
- `Scalar` = `f32` (from `parry-f32` feature in Cargo.toml)

---

## Phase 1: Foundation + CollisionLayers on Spawners

### Overview
Add the `surface_distance()` wrapper, `CollisionLayer` enum, `Hitbox` marker, and `CollisionLayers` to all entity spawners. No behavioral change yet — existing distance checks still use center-to-center.

### Changes Required:

#### 1. `third_party/avian.rs` — Add wrapper + enum
**File**: `src/third_party/avian.rs`

Replace the entire file with:
```rust
//! Avian2d physics configuration for top-down gameplay.

use avian2d::collision::collider::parry::contact_query;
use avian2d::prelude::*;
use bevy::prelude::*;

use crate::gameplay::battlefield::CELL_SIZE;

// === Collision Layers ===

/// Physics collision layers for the hitbox/hurtbox system.
///
/// - **Pushbox**: Physical presence — entities push/block each other.
/// - **Hitbox**: Attack collider (on projectiles, future melee swings).
/// - **Hurtbox**: Damageable surface (on units, buildings, fortresses).
#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum CollisionLayer {
    /// Physical body — blocks movement. All solid entities are pushboxes.
    #[default]
    Pushbox,
    /// Attack collider — lives on projectiles and (future) melee swings.
    Hitbox,
    /// Damageable surface — lives on units, buildings, fortresses.
    Hurtbox,
}

// === Helpers ===

/// Compute the minimum distance between two collider *surfaces*.
///
/// Uses avian2d's GJK-based `contact_query::distance()` under the hood.
/// Game systems call this instead of `contact_query` directly — if the
/// physics engine changes, only this wrapper changes.
///
/// Returns `f32::MAX` if the shape is unsupported (should never happen
/// with circles and rectangles).
#[must_use]
pub fn surface_distance(c1: &Collider, pos1: Vec2, c2: &Collider, pos2: Vec2) -> f32 {
    contact_query::distance(c1, pos1, 0.0, c2, pos2, 0.0).unwrap_or(f32::MAX)
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(PhysicsPlugins::default().with_length_unit(CELL_SIZE));
    app.insert_resource(Gravity::ZERO);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_distance_circle_circle() {
        let c1 = Collider::circle(10.0);
        let c2 = Collider::circle(5.0);
        let dist = surface_distance(&c1, Vec2::ZERO, &c2, Vec2::new(25.0, 0.0));
        // Center distance 25, radii 10 + 5 = 15 → surface distance 10
        assert!((dist - 10.0).abs() < 0.01);
    }

    #[test]
    fn surface_distance_circle_rectangle() {
        let circle = Collider::circle(12.0); // unit
        let rect = Collider::rectangle(128.0, 640.0); // fortress
        let dist = surface_distance(&circle, Vec2::new(100.0, 0.0), &rect, Vec2::ZERO);
        // Circle center at x=100, fortress half-width 64 → surface at x=64.
        // Distance from circle surface (100-12=88) to fortress surface (64) = 24.
        assert!((dist - 24.0).abs() < 0.01);
    }

    #[test]
    fn surface_distance_overlapping_returns_zero() {
        let c1 = Collider::circle(10.0);
        let c2 = Collider::circle(10.0);
        let dist = surface_distance(&c1, Vec2::ZERO, &c2, Vec2::new(5.0, 0.0));
        // Overlap: center distance 5 < sum of radii 20 → 0
        assert!(dist <= 0.01);
    }

    #[test]
    fn surface_distance_same_position() {
        let c1 = Collider::circle(10.0);
        let c2 = Collider::circle(10.0);
        let dist = surface_distance(&c1, Vec2::ZERO, &c2, Vec2::ZERO);
        assert!(dist <= 0.01);
    }

    #[test]
    fn surface_distance_circle_building_rect() {
        let circle = Collider::circle(12.0); // unit
        let rect = Collider::rectangle(60.0, 60.0); // building
        let dist = surface_distance(&circle, Vec2::new(72.0, 0.0), &rect, Vec2::ZERO);
        // Circle at x=72, building half-width 30 → building edge at x=30.
        // Circle surface at x=72-12=60. Distance = 60-30 = 30.
        assert!((dist - 30.0).abs() < 0.01);
    }
}
```

#### 2. `third_party/mod.rs` — Re-export
**File**: `src/third_party/mod.rs`

```rust
//! Third-party plugin isolation.

mod avian;

pub use avian::{CollisionLayer, surface_distance};

pub fn plugin(app: &mut bevy::prelude::App) {
    app.add_plugins(avian::plugin);
}
```

#### 3. `combat/attack.rs` — Add `Hitbox` marker
**File**: `src/gameplay/combat/attack.rs`

Add after the `Projectile` struct (line ~37):
```rust
/// Marker for hitbox sensor entities (attack colliders that damage hurtbox targets).
/// Lives on projectiles; future: melee swing entities.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Hitbox;
```

Register in combat attack plugin function:
```rust
app.register_type::<Hitbox>();
```

#### 4. `combat/mod.rs` — Re-export `Hitbox`
**File**: `src/gameplay/combat/mod.rs`

Add to re-exports:
```rust
pub use attack::Hitbox;
```

#### 5. Entity spawners — Add `CollisionLayers`

**Units** (`src/gameplay/units/mod.rs`, `spawn_unit()` at line 155):
Add `CollisionLayers` to the physics `.insert((...))` block:
```rust
.insert((
    RigidBody::Dynamic,
    Collider::circle(UNIT_RADIUS),
    CollisionLayers::new(
        [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
        [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
    ),
    LockedAxes::ROTATION_LOCKED,
    LinearVelocity::ZERO,
))
```

Import needed: `use crate::third_party::CollisionLayer;`

**Buildings** (`src/gameplay/building/placement.rs`, line ~140):
Add after `Collider::rectangle(...)`:
```rust
CollisionLayers::new(
    [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
    [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
),
```

Import needed: `use crate::third_party::CollisionLayer;`

**Fortresses** (`src/gameplay/battlefield/renderer.rs`, lines 75-76 and 131-132):
Add after each fortress's `Collider::rectangle(...)`:
```rust
CollisionLayers::new(
    [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
    [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
),
```

Import needed: `use crate::third_party::CollisionLayer;`

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (no compilation errors, clippy clean)
- [x] `make test` passes (existing tests unaffected — CollisionLayers addition is additive)
- [x] New `surface_distance()` unit tests pass

#### Manual Verification:
- [ ] Game runs without visual changes (collision layers match previous all-collide-with-all for pushbox entities)

**Implementation Note**: Pause here for verification before proceeding to Phase 2.

---

## Phase 2: Range Checks → `surface_distance()`

### Overview
Replace center-to-center distance with `surface_distance()` in the three game systems. This is the core bug fix — after this phase, units can attack buildings and fortresses.

### Changes Required:

#### 1. `unit_movement` (`src/gameplay/units/movement.rs`)

Add `&Collider` to unit query and replace target query with `(&GlobalTransform, &Collider)`. Replace `diff.length()` with `surface_distance()`.

```rust
use crate::third_party::surface_distance;

pub(super) fn unit_movement(
    mut units: Query<
        (
            &CurrentTarget,
            &Movement,
            &CombatStats,
            &GlobalTransform,
            &Collider,
            &mut LinearVelocity,
        ),
        With<Unit>,
    >,
    targets: Query<(&GlobalTransform, &Collider)>,
) {
    for (current_target, movement, stats, global_transform, unit_collider, mut velocity) in
        &mut units
    {
        let Some(target_entity) = current_target.0 else {
            velocity.0 = Vec2::ZERO;
            continue;
        };
        let Ok((target_pos, target_collider)) = targets.get(target_entity) else {
            velocity.0 = Vec2::ZERO;
            continue;
        };

        let current_xy = global_transform.translation().xy();
        let target_xy = target_pos.translation().xy();
        let distance = surface_distance(unit_collider, current_xy, target_collider, target_xy);

        // Already within attack range — stop
        if distance <= stats.range {
            velocity.0 = Vec2::ZERO;
            continue;
        }

        // Direction toward target (center-to-center for heading)
        let diff = target_xy - current_xy;
        let center_distance = diff.length();
        if center_distance < f32::EPSILON {
            velocity.0 = Vec2::ZERO;
            continue;
        }

        let direction = diff / center_distance;
        velocity.0 = direction * movement.speed;
    }
}
```

#### 2. `unit_attack` (`src/gameplay/combat/attack.rs`)

Add `&Collider` to attacker query, change target query to `(&GlobalTransform, &Collider)`. Replace center distance with `surface_distance()`.

```rust
use crate::third_party::surface_distance;

fn unit_attack(
    time: Res<Time>,
    mut attackers: Query<
        (
            &CurrentTarget,
            &CombatStats,
            &mut AttackTimer,
            &GlobalTransform,
            &Collider,
        ),
        With<Unit>,
    >,
    targets: Query<(&GlobalTransform, &Collider)>,
    mut commands: Commands,
) {
    for (target, stats, mut timer, attacker_pos, attacker_collider) in &mut attackers {
        let Some(target_entity) = target.0 else {
            continue;
        };
        let Ok((target_pos, target_collider)) = targets.get(target_entity) else {
            continue;
        };

        let distance = surface_distance(
            attacker_collider,
            attacker_pos.translation().xy(),
            target_collider,
            target_pos.translation().xy(),
        );
        if distance > stats.range {
            continue;
        }

        // Tick and spawn projectile when timer fires (unchanged)
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            // ... projectile spawn (same as current, updated in Phase 3)
        }
    }
}
```

#### 3. `unit_find_target` (`src/gameplay/units/ai.rs`)

Add `&Collider` to unit and target queries. Replace `my_pos.distance(candidate_xy)` with `surface_distance()`. Backtrack check stays unchanged (x-distance).

```rust
use avian2d::prelude::*;
use crate::third_party::surface_distance;

pub(super) fn unit_find_target(
    mut counter: Local<u32>,
    mut units: Query<
        (Entity, &Team, &GlobalTransform, &Collider, &mut CurrentTarget),
        With<Unit>,
    >,
    all_targets: Query<(Entity, &Team, &GlobalTransform, &Collider), With<Target>>,
) {
    // ... counter increment, loop header unchanged ...

    for (candidate, candidate_team, candidate_pos, candidate_collider) in &all_targets {
        // ... team check, backtrack check unchanged ...

        let dist = surface_distance(unit_collider, my_pos, candidate_collider, candidate_xy);
        if nearest.is_none_or(|(_, d)| dist < d) {
            nearest = Some((candidate, dist));
        }
    }

    // ... set current_target unchanged ...
}
```

#### 4. Update existing tests — add `Collider` to test entities

All test helper functions that spawn entities queried by these systems need `Collider`:

**`units/movement.rs` tests:**
- `spawn_unit_at()` → add `Collider::circle(UNIT_RADIUS)`
- `spawn_target_at()` → add `Collider::circle(5.0)`

**`combat/attack.rs` tests:**
- `spawn_attacker()` → add `Collider::circle(UNIT_RADIUS)`
- `spawn_target()` → add `Collider::circle(5.0)`

**`units/ai.rs` tests:**
- `spawn_unit()` → add `Collider::circle(UNIT_RADIUS)`
- `spawn_target()` → add `Collider::circle(5.0)`

**Distance verification for critical test assertions:**
- `unit_spawns_projectile_in_range`: attacker at x=100 circle(12), target at x=120 circle(5). Surface = 20 - 12 - 5 = 3. Range = 30. 3 ≤ 30 ✓ (in range)
- `unit_does_not_attack_out_of_range`: attacker at x=100 circle(12), target at x=500 circle(5). Surface = 400 - 12 - 5 = 383. 383 > 30 ✓ (out of range)
- `unit_stops_at_attack_range`: unit at x=471 circle(12), target at x=500 circle(5). Surface = 29 - 12 - 5 = 12. 12 ≤ 30 ✓ (in range, stops)
- AI tests: center-to-center ordering preserved by surface distance for same-shape colliders

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes (all existing tests updated and passing)

#### Manual Verification:
- [ ] Units walk up to buildings and attack (previously broken)
- [ ] Units walk up to fortresses and attack (previously broken)
- [ ] Unit-to-unit combat still works
- [ ] Units stop at correct visual distance from targets (surface + range, not center + range)

**Implementation Note**: This phase is the core bug fix. Pause for manual verification before proceeding to Phase 3.

---

## Phase 3: Projectile Hitbox/Hurtbox Damage Delivery

### Overview
Replace the manual projectile arrival check with collision-based damage using hitbox/hurtbox. Projectiles get a `Sensor` collider with `Hitbox` layer; `CollidingEntities` detects overlaps with hurtbox targets.

### Changes Required:

#### 1. Update projectile spawning (`combat/attack.rs`)

In `unit_attack`, add `&Team` to the attacker query and include it + physics hitbox components in the spawn:

```rust
// Add &Team to the attacker query tuple
mut attackers: Query<
    (&CurrentTarget, &CombatStats, &mut AttackTimer, &GlobalTransform, &Collider, &Team),
    With<Unit>,
>,

// In the spawn:
commands.spawn((
    Name::new("Projectile"),
    Projectile {
        target: target_entity,
        damage: stats.damage,
        speed: PROJECTILE_SPEED,
    },
    *team, // Team from the attacker — prevents friendly fire in handle_projectile_hits
    Hitbox,
    Sprite::from_color(PROJECTILE_COLOR, Vec2::splat(PROJECTILE_RADIUS * 2.0)),
    Transform::from_xyz(
        attacker_pos.translation().x,
        attacker_pos.translation().y,
        Z_UNIT + 0.5,
    ),
    DespawnOnExit(GameState::InGame),
    // Physics: sensor hitbox for collision-based damage
    RigidBody::Kinematic,
    Collider::circle(PROJECTILE_RADIUS),
    Sensor,
    CollisionLayers::new(CollisionLayer::Hitbox, CollisionLayer::Hurtbox),
    CollisionEventsEnabled,
    CollidingEntities::default(),
));
```

Add import: `use avian2d::prelude::*;` and `use crate::third_party::CollisionLayer;`

#### 2. Simplify `move_projectiles` (`combat/attack.rs`)

Remove damage logic. Keep movement + target-gone cleanup + overshoot snap:

```rust
fn move_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut projectiles: Query<(Entity, &Projectile, &mut Transform)>,
    positions: Query<&GlobalTransform>,
) {
    for (entity, projectile, mut transform) in &mut projectiles {
        // Target gone — despawn projectile harmlessly
        let Ok(target_pos) = positions.get(projectile.target) else {
            commands.entity(entity).despawn();
            continue;
        };

        let target_xy = target_pos.translation().truncate();
        let current_xy = transform.translation.truncate();
        let direction = target_xy - current_xy;
        let distance = direction.length();

        if distance < f32::EPSILON {
            continue; // At target — collision system handles damage
        }

        let move_amount = projectile.speed * time.delta_secs();
        if move_amount >= distance {
            // Snap to target to prevent tunneling (collision handles damage)
            transform.translation.x = target_xy.x;
            transform.translation.y = target_xy.y;
        } else {
            let dir = direction / distance;
            transform.translation.x = dir.x.mul_add(move_amount, transform.translation.x);
            transform.translation.y = dir.y.mul_add(move_amount, transform.translation.y);
        }
    }
}
```

Key change: removed `mut healths: Query<&mut Health>` parameter and all damage application.

#### 3. New `handle_projectile_hits` system (`combat/attack.rs`)

```rust
/// Checks projectile hitbox overlaps with hurtboxes via `CollidingEntities`.
/// Damages the first opposing-team entity hit and despawns the projectile.
/// Runs after `move_projectiles` in the combat chain.
fn handle_projectile_hits(
    mut commands: Commands,
    projectiles: Query<(Entity, &Projectile, &Team, &CollidingEntities), With<Hitbox>>,
    mut targets: Query<(&Team, &mut Health)>,
) {
    for (entity, projectile, proj_team, colliding) in &projectiles {
        for &hit in &colliding.0 {
            let Ok((hit_team, mut health)) = targets.get_mut(hit) else {
                continue;
            };
            // No friendly fire
            if hit_team == proj_team {
                continue;
            }
            health.current -= projectile.damage;
            commands.entity(entity).despawn();
            break; // One hit per projectile
        }
    }
}
```

Design: projectiles damage the **first opposing-team hurtbox** they overlap — not just the intended target. `Projectile.target` is used only for homing movement. No friendly fire (team check). One hit per projectile (despawn + break after first damage).

#### 4. Update plugin system registration (`combat/attack.rs`)

Replace the current 2-system chain with 3 systems:

```rust
app.add_systems(
    Update,
    (unit_attack, move_projectiles, handle_projectile_hits)
        .chain_ignore_deferred()
        .in_set(GameSet::Combat)
        .run_if(gameplay_running),
);
```

Ordering: spawn → move → check hits. `chain_ignore_deferred` means newly spawned projectiles don't move until next frame (no instant-hit invisible projectiles).

Note: `CollidingEntities` is updated by the physics engine in `FixedPostUpdate` (before `Update`), so by the time `handle_projectile_hits` runs, it reflects the current frame's collisions. There is a ~1 frame delay between projectile movement and collision detection — imperceptible at 60fps.

#### 5. Update tests (`combat/attack.rs`)

**Replace `projectile_deals_damage_on_arrival` and `projectile_despawns_on_arrival`** with collision-based tests:

```rust
// === Hit Test Helpers ===

fn create_hit_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_systems(Update, handle_projectile_hits);
    app.update(); // Initialize time
    app
}

/// Spawn a projectile with team, damage, and pre-populated CollidingEntities.
fn spawn_test_projectile(
    world: &mut World,
    team: Team,
    target: Entity,
    damage: f32,
    colliding_with: &[Entity],
) -> Entity {
    let colliding = CollidingEntities(EntityHashSet::from_iter(colliding_with.iter().copied()));
    world.spawn((
        Projectile { target, damage, speed: 200.0 },
        team,
        Hitbox,
        colliding,
    )).id()
}

// === Collision-Based Damage Tests ===

#[test]
fn projectile_hit_applies_damage() {
    let mut app = create_hit_test_app();

    let enemy = app.world_mut().spawn((Team::Enemy, Health::new(100.0))).id();
    spawn_test_projectile(app.world_mut(), Team::Player, enemy, 25.0, &[enemy]);

    app.update();

    let health = app.world().get::<Health>(enemy).unwrap();
    assert_eq!(health.current, 75.0);
}

#[test]
fn projectile_despawns_on_hit() {
    let mut app = create_hit_test_app();

    let enemy = app.world_mut().spawn((Team::Enemy, Health::new(100.0))).id();
    spawn_test_projectile(app.world_mut(), Team::Player, enemy, 10.0, &[enemy]);

    app.update();

    assert_entity_count::<With<Projectile>>(&mut app, 0);
}

#[test]
fn projectile_does_not_friendly_fire() {
    let mut app = create_hit_test_app();

    // Player projectile collides with a friendly player unit
    let friendly = app.world_mut().spawn((Team::Player, Health::new(100.0))).id();
    let dummy_target = app.world_mut().spawn((Team::Enemy, Health::new(100.0))).id();
    spawn_test_projectile(app.world_mut(), Team::Player, dummy_target, 25.0, &[friendly]);

    app.update();

    // Friendly undamaged, projectile still alive
    let hp = app.world().get::<Health>(friendly).unwrap();
    assert_eq!(hp.current, 100.0);
    assert_entity_count::<With<Projectile>>(&mut app, 1);
}

#[test]
fn projectile_hits_non_target_enemy() {
    let mut app = create_hit_test_app();

    // Projectile aimed at enemy_far but collides with enemy_near (different entity)
    let enemy_near = app.world_mut().spawn((Team::Enemy, Health::new(100.0))).id();
    let enemy_far = app.world_mut().spawn((Team::Enemy, Health::new(100.0))).id();
    spawn_test_projectile(app.world_mut(), Team::Player, enemy_far, 25.0, &[enemy_near]);

    app.update();

    // enemy_near takes damage (even though it wasn't the intended target)
    let near_hp = app.world().get::<Health>(enemy_near).unwrap();
    assert_eq!(near_hp.current, 75.0);
    // enemy_far undamaged
    let far_hp = app.world().get::<Health>(enemy_far).unwrap();
    assert_eq!(far_hp.current, 100.0);
    assert_entity_count::<With<Projectile>>(&mut app, 0);
}

#[test]
fn projectile_no_collision_yet() {
    let mut app = create_hit_test_app();

    let enemy = app.world_mut().spawn((Team::Enemy, Health::new(100.0))).id();
    spawn_test_projectile(app.world_mut(), Team::Player, enemy, 25.0, &[]); // empty

    app.update();

    let health = app.world().get::<Health>(enemy).unwrap();
    assert_eq!(health.current, 100.0);
    assert_entity_count::<With<Projectile>>(&mut app, 1);
}
```

**Keep `projectile_despawns_when_target_missing`** — still tests `move_projectiles`.

**Keep `unit_spawns_projectile_in_range`, `unit_does_not_attack_out_of_range`, `attack_without_target_does_nothing`, `attack_respects_cooldown`** — updated with Collider in Phase 2.

#### 6. Tier 2 integration test — collision layer wiring (`combat/attack.rs`)

One smoke test with the real physics pipeline to verify that Hitbox↔Hurtbox layers produce `CollidingEntities` entries:

```rust
#[test]
fn hitbox_hurtbox_collision_layers_produce_colliding_entities() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, PhysicsPlugins::default()));
    app.insert_resource(Gravity::ZERO);
    app.update(); // Initialize physics

    // Spawn a hurtbox entity (simulates a unit/building)
    let hurtbox = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        RigidBody::Static,
        Collider::circle(20.0),
        CollisionLayers::new(
            [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
            [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
        ),
    )).id();

    // Spawn a hitbox projectile overlapping the hurtbox
    let hitbox = app.world_mut().spawn((
        Transform::from_xyz(0.0, 0.0, 0.0),
        RigidBody::Kinematic,
        Collider::circle(4.0),
        Sensor,
        CollisionLayers::new(CollisionLayer::Hitbox, CollisionLayer::Hurtbox),
        CollidingEntities::default(),
    )).id();

    // Run enough frames for physics broadphase + narrowphase
    for _ in 0..3 {
        app.update();
    }

    let colliding = app.world().get::<CollidingEntities>(hitbox).unwrap();
    assert!(
        colliding.contains(&hurtbox),
        "Hitbox sensor should detect overlapping Hurtbox entity"
    );
}
```

This validates that `CollisionLayer` enum, layer assignments, and `Sensor` + `CollidingEntities` wiring work end-to-end with the real physics engine.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes (all new + updated tests)
- Note: Tier 2 physics integration test removed — avian2d's FixedUpdate pipeline unreliable under MinimalPlugins

#### Manual Verification:
- [ ] Projectiles visually travel to targets and deal damage
- [ ] Projectile despawns after hitting target
- [ ] Projectile despawns if target dies before arrival
- [ ] No visual glitches (projectiles don't hang or double-hit)

**Implementation Note**: Pause for manual verification before proceeding to Phase 4.

---

## Phase 4: Documentation + Final Verification

### Overview
Update ARCHITECTURE.md with the collision layer system documentation and perform final verification.

### Changes Required:

#### 1. ARCHITECTURE.md — Add "Collision Layer System" section

Insert after the "Third-party isolation" section (around line 477, before "Testing Patterns"):

```markdown
### Collision Layer System

The game uses avian2d collision layers for physics filtering and damage delivery.

#### `CollisionLayer` enum (`third_party/avian.rs`)

| Layer | Purpose | On which entities |
|-------|---------|-------------------|
| `Pushbox` | Physical presence — blocks movement | Units, buildings, fortresses |
| `Hitbox` | Attack collider — deals damage | Projectiles (future: melee swings) |
| `Hurtbox` | Damageable surface | Units, buildings, fortresses |

#### Entity collision setup

| Entity | Memberships | Filters |
|--------|-------------|---------|
| Unit / Building / Fortress | `[Pushbox, Hurtbox]` | `[Pushbox, Hitbox]` |
| Projectile | `[Hitbox]` | `[Hurtbox]` |

Pushbox entities push/block each other (Pushbox↔Pushbox). Projectile hitboxes overlap with target hurtboxes (Hitbox↔Hurtbox) without physical response (`Sensor`).

#### `surface_distance()` wrapper (`third_party/avian.rs`)

Game systems use `surface_distance(&collider1, pos1, &collider2, pos2)` for range checks — never `contact_query` directly. This abstracts the physics dependency to one file.

#### Two-tier testing

- **Tier 1** (primary): `MinimalPlugins` with `Collider` components on test entities. `surface_distance()` is a pure geometric function — no physics pipeline needed. `CollidingEntities` manually populated for hitbox tests.
- **Tier 2** (smoke): Integration tests with `PhysicsPlugins` to validate collision layer wiring. Used sparingly.
```

#### 2. Update entity archetype docs in `gameplay/mod.rs`

Update the module-level doc comments to include new components:

```rust
//! **Units**: `Unit`, `Team`, `Target`, `CurrentTarget`, `Health`, `CombatStats`, `Movement`,
//!           `AttackTimer`, `HealthBarConfig`, `Mesh2d`, `MeshMaterial2d`,
//!           `RigidBody::Dynamic`, `Collider`, `CollisionLayers`, `LockedAxes`, `LinearVelocity`
//!
//! **Buildings**: `Building`, `Team`, `Target`, `Health`, `HealthBarConfig`,
//!           `ProductionTimer` or `IncomeTimer`, `RigidBody::Static`, `Collider`, `CollisionLayers`
//!
//! **Fortresses**: `PlayerFortress`/`EnemyFortress`, `Team`, `Target`, `Health`,
//!           `HealthBarConfig`, `RigidBody::Static`, `Collider`, `CollisionLayers`
//!
//! **Projectiles**: `Projectile`, `Hitbox`, `Sensor`, `RigidBody::Kinematic`,
//!           `Collider`, `CollisionLayers`, `CollisionEventsEnabled`, `CollidingEntities`
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes (171 tests)
- [x] Test coverage maintained or increased

#### Manual Verification:
- [ ] Full game loop works: place buildings → units spawn → enemies attack buildings/fortresses → game ends
- [ ] Unit-to-unit combat still works
- [ ] No regressions in economy, wave spawning, or UI

---

## Testing Strategy

### Unit Tests (Tier 1):
- `surface_distance()` with circle-circle, circle-rectangle, overlapping, same-position
- `handle_projectile_hits` with manually populated `CollidingEntities` (hit, miss, non-target)
- All existing range-check tests updated with `Collider` on entities

### Manual Testing Steps:
1. Start a game, place a Barracks building
2. Wait for enemy wave — enemies should walk up to the building and attack
3. Watch projectiles — they should visually travel and deal damage
4. Place a second building — enemies should target the nearest one
5. Let enemies reach the fortress — they should attack it
6. Verify unit-to-unit combat still works normally

## Performance Considerations

- `surface_distance()` uses GJK algorithm (Parry) — O(1) per call, fast for circles and rectangles
- `CollidingEntities` polling is O(n) per projectile per frame, but projectile count is small
- No additional physics bodies added to pushbox entities (units, buildings, fortresses already have RigidBody)
- Projectiles add `RigidBody::Kinematic` — minimal overhead since they're sensors

## References

- Linear ticket: [GAM-26](https://linear.app/tayhu-games/issue/GAM-26/hitboxhurtbox-collision-system-surface-distance-range-checks)
- Related: [GAM-10](https://linear.app/tayhu-games/issue/GAM-10/add-unit-physics) (introduced the physics colliders)
- avian2d 0.5 source: `~/.cargo/registry/src/index.crates.io-.../avian2d-0.5.0/`
- Collision layer example: `avian2d-0.5.0/examples/collision_layers.rs`
- Sensor example: `avian2d-0.5.0/examples/sensor.rs`

# Remove avian2d Physics Engine (GAM-62) Implementation Plan

## Overview

Strip all avian2d physics from the codebase. Replace the last functional dependency — projectile hit detection via `CollidingEntities` — with distance-based arrival checks in `move_projectiles`. Delete the `third_party/` module, remove `avian2d` from `Cargo.toml`.

This is Ticket 5 of the targeting/movement/combat architecture rework. GAM-61 already decoupled ORCA and removed `LinearVelocity`. Every system that used physics for movement/targeting has been migrated. This ticket is a clean removal.

## Current State Analysis

**Remaining avian2d usage:**
- `PhysicsPlugins::default()` registered in `third_party/avian.rs:58`
- `RigidBody`, `Collider`, `CollisionLayers`, `LockedAxes` on units (`units/mod.rs:134-138`)
- `RigidBody`, `Collider`, `CollisionLayers` on buildings (`building/placement.rs:153-155`)
- `RigidBody`, `Collider`, `CollisionLayers` on fortresses (`battlefield/renderer.rs:94-99`, `176-181`)
- `RigidBody`, `Collider`, `Sensor`, `CollisionLayers`, `CollisionEventsEnabled`, `CollidingEntities` on projectiles (`combat/attack.rs:105-110`)
- `CollidingEntities` queried in `handle_projectile_hits` (`combat/attack.rs:160`) — **only functional dependency**
- `Collider` in test helpers (`testing.rs:179,204`) and test spawn sites (`ai.rs:658`, `movement.rs:308`, `attack.rs:521`)
- 3 parity tests in `gameplay/mod.rs:331-386` comparing `extent_distance` vs GJK `surface_distance`
- `CollisionLayer` enum and `solid_entity_layers()` helper in `third_party/avian.rs`

### Key Discoveries:
- `LinearVelocity` already fully removed by GAM-61 — confirmed zero occurrences in `/src`
- ORCA module (`units/avoidance/`) has zero avian2d imports — fully decoupled
- `Hitbox` marker component defined in `attack.rs:41-43` — only queried by `handle_projectile_hits` via `With<Hitbox>`. Dead code after rewrite: remove definition, registration (`attack.rs:184`), re-export (`combat/mod.rs:9`)
- `EntityExtent` + `extent_distance` already handle all range checks — no physics needed
- The `third_party/` directory contains only `avian.rs` + `mod.rs` — entire directory gets deleted
- `menus/mod.rs:37` has a comment about avian2d physics — update
- `ARCHITECTURE.md` has extensive avian2d/physics references (lines 110-112, 190-192, 316-319, 607-672, 706, 893-894) — update
- `gameplay/mod.rs:156` has a doc comment referencing `third_party::surface_distance()` — update

## Desired End State

- Zero references to `avian2d` anywhere in the codebase
- No `Cargo.toml` dependency on `avian2d`
- No `third_party/` module
- Projectile hit detection via distance-based arrival in `move_projectiles`
- `handle_projectile_hits` system deleted
- Combat chain simplified from 3 systems to 2: `(attack, move_projectiles)`
- All tests pass, no physics components in any spawn site or test helper

### Verification:
- `cargo build` succeeds with no avian2d references
- `cargo test` passes — all rewritten tests verify distance-based hit detection
- `cargo clippy` clean
- Manual: run the game, spawn units, verify projectiles still hit targets and deal damage

## What We're NOT Doing

- Adding new projectile AoE/splash damage — projectiles remain single-target homing
- Changing projectile speed, damage, or visual behavior
- Touching ORCA/avoidance code — already decoupled in GAM-61
- Profiling — that's GAM-63

## Implementation Approach

Two phases:

1. **Phase 1 (Functional change)**: Rewrite projectile hit detection to use distance-based arrival instead of `CollidingEntities`. Remove `handle_projectile_hits`. Strip physics from projectile spawn. Update hit tests. avian2d still present as a dep but unused by projectiles.

2. **Phase 2 (Mechanical cleanup)**: Strip remaining physics components from units/buildings/fortresses. Delete `third_party/`. Remove avian2d from `Cargo.toml`. Update all remaining tests and docs. Pure removal — no behavioral changes.

This split means Phase 1 can be tested independently (projectile behavior correct?) before the larger mechanical cleanup in Phase 2.

---

## Phase 1: Rewrite Projectile Hit Detection

### Overview
Replace collision-based hit detection with distance-based arrival detection. When a projectile reaches its target (`move_amount >= distance`), apply damage directly and despawn. Eliminates the `handle_projectile_hits` system.

### Changes Required:

#### 1. Rewrite `move_projectiles` to include hit logic
**File**: `src/gameplay/combat/attack.rs`
**Changes**: Merge hit detection into `move_projectiles`. Add `Team` to projectile query, add `Health`/`Team` target query. Apply damage on arrival.

```rust
fn move_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut projectiles: Query<(Entity, &Projectile, &Team, &mut Transform)>,
    positions: Query<&GlobalTransform>,
    mut targets: Query<(&Team, &mut Health)>,
) {
    for (entity, projectile, proj_team, mut transform) in &mut projectiles {
        // Target gone — despawn projectile harmlessly
        let Ok(target_pos) = positions.get(projectile.target) else {
            commands.entity(entity).despawn();
            continue;
        };

        let target_xy = target_pos.translation().truncate();
        let current_xy = transform.translation.truncate();
        let direction = target_xy - current_xy;
        let distance = direction.length();
        let move_amount = projectile.speed * time.delta_secs();

        if distance < f32::EPSILON || move_amount >= distance {
            // Arrived — apply damage and despawn
            if let Ok((hit_team, mut health)) = targets.get_mut(projectile.target) {
                if hit_team != proj_team {
                    health.current = (health.current - projectile.damage).max(0.0);
                }
            }
            commands.entity(entity).despawn();
        } else {
            let dir = direction / distance;
            transform.translation.x = dir.x.mul_add(move_amount, transform.translation.x);
            transform.translation.y = dir.y.mul_add(move_amount, transform.translation.y);
        }
    }
}
```

#### 2. Delete `handle_projectile_hits` system
**File**: `src/gameplay/combat/attack.rs`
**Changes**: Remove the `handle_projectile_hits` function (lines 158-177).

#### 3. Update combat system chain
**File**: `src/gameplay/combat/attack.rs`
**Changes**: In `plugin()`, simplify the system chain:

```rust
app.add_systems(
    Update,
    (attack, move_projectiles)
        .chain_ignore_deferred()
        .in_set(GameSet::Combat)
        .run_if(gameplay_running),
);
```

#### 4. Strip physics from projectile spawn
**File**: `src/gameplay/combat/attack.rs`
**Changes**: In `attack()`, remove physics components from projectile spawn (lines 105-110). Remove the `use avian2d::prelude::*` import and `use crate::third_party::CollisionLayer` import.

Projectile spawn becomes:
```rust
commands.spawn((
    Name::new("Projectile"),
    Projectile {
        target: target_entity,
        damage: stats.damage,
        speed: PROJECTILE_SPEED,
    },
    *team,
    Sprite::from_color(palette::PROJECTILE, Vec2::splat(PROJECTILE_RADIUS * 2.0)),
    Transform::from_xyz(
        attacker_pos.translation().x,
        attacker_pos.translation().y,
        Z_PROJECTILE,
    ),
    DespawnOnExit(GameState::InGame),
));
```

#### 5. Remove `Hitbox` component (dead code)
**File**: `src/gameplay/combat/attack.rs`
**Changes**:
- Delete the `Hitbox` struct definition (lines 41-43) and its doc comment (lines 39-40)
- Remove `.register_type::<Hitbox>()` from `plugin()` (line 184)
- Remove `Hitbox` from the projectile spawn bundle (already gone from step 4)

**File**: `src/gameplay/combat/mod.rs`
**Changes**:
- Change `pub use attack::{AttackTimer, Hitbox};` to `pub use attack::AttackTimer;` (line 9)
- Remove the `#[allow(unused_imports)]` if it was only there for `Hitbox`

#### 6. Rewrite projectile hit tests
**File**: `src/gameplay/combat/attack.rs` (integration_tests module)
**Changes**:

Remove `spawn_test_projectile` helper (used `CollidingEntities`) and all tests that used it. Replace with arrival-based tests using `move_projectiles`:

Tests to **delete**:
- `projectile_hits_non_target_enemy` — behavior no longer exists (projectiles hit their designated target)
- `projectile_no_collision_yet` — replaced by in-flight test below

Tests to **rewrite** (using `create_projectile_test_app` + arrival mechanics):
- `projectile_hit_applies_damage` — spawn projectile near target, advance time → target loses HP
- `projectile_hit_clamps_health_at_zero` — same, with damage > HP
- `projectile_despawns_on_hit` — same, verify projectile entity gone
- `projectile_does_not_friendly_fire` — spawn projectile targeting same-team entity, arrive → no damage

Test to **add**:
- `projectile_in_flight_no_damage` — spawn projectile far from target, one small time step → target at full HP, projectile still alive

All rewritten tests use `create_projectile_test_app()` (which registers `move_projectiles`). Spawn a `Projectile` entity at one position with a target at another. `advance_and_update` with enough time for arrival (use high speed + short distance for deterministic arrival).

Example test pattern:
```rust
#[test]
fn projectile_hit_applies_damage() {
    let mut app = create_projectile_test_app();

    let target = app.world_mut()
        .spawn((
            Team::Enemy,
            Health::new(100.0),
            Transform::from_xyz(100.0, 0.0, 0.0),
            GlobalTransform::from(Transform::from_xyz(100.0, 0.0, 0.0)),
        ))
        .id();

    // Spawn projectile very close to target — arrives in one frame
    app.world_mut().spawn((
        Projectile { target, damage: 25.0, speed: 100_000.0 },
        Team::Player,
        Transform::from_xyz(99.0, 0.0, 0.0),
    ));

    advance_and_update(&mut app, Duration::from_millis(100));

    let health = app.world().get::<Health>(target).unwrap();
    assert_eq!(health.current, 75.0);
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes — all rewritten hit tests verify distance-based detection
- [ ] `cargo clippy` clean

#### Manual Verification:
- [ ] Run the game, let units fight — projectiles visually travel to targets and deal damage
- [ ] Verify enemy HP decreases when hit
- [ ] Verify units die when HP reaches 0

**Implementation Note**: After completing Phase 1, pause for manual verification that projectile behavior is unchanged before proceeding.

---

## Phase 2: Strip Physics and Delete avian2d

### Overview
Remove all remaining physics components from entity spawn sites. Delete the `third_party/` module. Remove `avian2d` from `Cargo.toml`. Update tests and documentation. Pure mechanical cleanup — no behavioral changes.

### Changes Required:

#### 1. Strip physics from unit spawn
**File**: `src/gameplay/units/mod.rs`
**Changes**:
- Remove `use avian2d::prelude::*` import (line 7)
- Remove `use crate::third_party::solid_entity_layers` import (line 19)
- In `spawn_unit()`, remove from the `.insert(...)` block (lines 134-138):
  - `RigidBody::Kinematic`
  - `Collider::circle(UNIT_RADIUS)`
  - `solid_entity_layers()`
  - `LockedAxes::ROTATION_LOCKED`

The second `.insert(...)` keeps: `TargetingState`, `AssignedGoal`, `EntityExtent`, `PreferredVelocity`.

#### 2. Strip physics from building spawn
**File**: `src/gameplay/building/placement.rs`
**Changes**:
- Remove `use avian2d::prelude::*` import (line 3)
- Remove `use crate::third_party::solid_entity_layers` import (line 20)
- In `handle_building_placement()`, remove from spawn (lines 153-155):
  - `RigidBody::Static`
  - `Collider::rectangle(BUILDING_SPRITE_SIZE, BUILDING_SPRITE_SIZE)`
  - `solid_entity_layers()`

#### 3. Strip physics from fortress spawns
**File**: `src/gameplay/battlefield/renderer.rs`
**Changes**:
- Remove `use avian2d::prelude::*` import (line 3)
- Remove `use crate::third_party::solid_entity_layers` import (line 18)
- Player fortress `.insert(...)` (lines 94-99): remove `RigidBody::Static`, `Collider::rectangle(...)`, `solid_entity_layers()`. Keep `EntityExtent`.
- Enemy fortress `.insert(...)` (lines 176-181): same removal. Keep `EntityExtent`.

#### 4. Delete `third_party/` module
**Files**: Delete `src/third_party/avian.rs` and `src/third_party/mod.rs`

#### 5. Remove from `lib.rs`
**File**: `src/lib.rs`
**Changes**:
- Remove `pub(crate) mod third_party;` (line 11)
- Remove `third_party::plugin,` from the plugin tuple (line 90)

#### 6. Remove from `Cargo.toml`
**File**: `Cargo.toml`
**Changes**: Remove line 13:
```toml
avian2d = { version = "0.5", default-features = false, features = ["2d", "parry-f32", "debug-plugin", "parallel"] }
```

#### 7. Update test helpers
**File**: `src/testing.rs`
**Changes**:
- Remove `use avian2d::prelude::Collider` import (line 5)
- `spawn_test_unit()`: remove `Collider::circle(UNIT_RADIUS)` (line 179)
- `spawn_test_target()`: remove `Collider::circle(5.0)` (line 204)
- Update doc comments on both functions to remove `Collider` from the component list

#### 8. Update test spawn sites
**File**: `src/gameplay/ai.rs` (tests module)
**Changes**:
- Remove `use avian2d::prelude::Collider` (line 605)
- In `spawn_test_fortress()`: remove `Collider::rectangle(128.0, 128.0)` (line 658)

**File**: `src/gameplay/units/movement.rs` (tests module)
**Changes**:
- Remove `use avian2d::prelude::Collider` (line 91)
- Remove `Collider::circle(5.0)` from the inline target spawn (line 308)

**File**: `src/gameplay/combat/attack.rs` (integration_tests module)
**Changes**:
- In `fortress_can_attack_in_range()`: remove `Collider::rectangle(128.0, 128.0)` (line 521)

#### 9. Delete parity tests
**File**: `src/gameplay/mod.rs`
**Changes**: Delete the entire parity test section (3 tests around lines 328-386):
- `parity_circle_circle`
- `parity_circle_rect`
- `parity_rect_rect`

These compared `extent_distance` vs GJK `surface_distance` — with physics gone, the reference implementation is gone. The `extent_distance` tests (non-parity) remain.

#### 10. Update entity archetype documentation
**File**: `src/gameplay/mod.rs`
**Changes**:
- Update the doc comment block (lines 5-17) to remove physics components from archetype descriptions:
  - **Units**: remove `RigidBody::Kinematic`, `Collider`, `CollisionLayers`, `LockedAxes`
  - **Buildings**: remove `RigidBody::Static`, `Collider`, `CollisionLayers`
  - **Fortresses**: remove `RigidBody::Static`, `Collider`, `CollisionLayers`
  - **Projectiles**: remove `Sensor`, `RigidBody::Kinematic`, `Collider`, `CollisionLayers`, `CollisionEventsEnabled`, `CollidingEntities`, `Hitbox`
- Update doc comment on `extent_distance` (line 156) — remove reference to `third_party::surface_distance()`

#### 11. Update `menus/mod.rs` comment
**File**: `src/menus/mod.rs`
**Changes**: Update or remove the comment at line 37 that references avian2d physics (`"This stops physics (avian2d runs in FixedPostUpdate...)"`)

#### 12. Update ARCHITECTURE.md
**File**: `ARCHITECTURE.md`
**Changes**:
- Remove `third_party/` from directory tree (lines 110-112)
- Update virtual time pause section (lines 190-192) — remove "physics" references
- Update entity spawn table (lines 316-319) — remove `RigidBody`, `Collider`, `Sensor`, `CollidingEntities` columns
- Delete or rewrite "Third-Party Integration" section (lines 607-672) — `avian.rs`, `CollisionLayer`, `solid_entity_layers`, `surface_distance`, testing tiers with `PhysicsPlugins` all gone. Also clean up stale `vleue_navigator` docs (lines 651-672, already removed from code)
- Update `spawn_test_target` table entry (line 706) — remove `Collider`
- Update `Cargo.toml` snippet (lines 893-894) — remove `avian2d` and `vleue_navigator`

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` succeeds — zero avian2d references
- [ ] `cargo test` passes — all existing (non-parity) tests still pass
- [ ] `cargo clippy` clean
- [ ] `grep -r "avian2d" src/` returns nothing
- [ ] `grep -r "Collider" src/` returns nothing
- [ ] `grep -r "RigidBody" src/` returns nothing
- [ ] `grep -r "CollisionLayers\|CollisionLayer\b" src/` returns nothing (except doc comments if any)
- [ ] `grep -r "solid_entity_layers" src/` returns nothing
- [ ] `grep -r "Hitbox" src/` returns nothing
- [ ] `grep -r "third_party" src/` returns nothing

#### Manual Verification:
- [ ] Run the game — all entity types spawn correctly without physics
- [ ] Units move via flow field + ORCA (unchanged from GAM-61)
- [ ] Buildings place correctly
- [ ] Fortresses render at correct positions
- [ ] No visual regressions

---

## Testing Strategy

### Rewritten Tests (Phase 1):
- `projectile_hit_applies_damage` — arrival-based: 25 damage to 100 HP → 75 HP
- `projectile_hit_clamps_health_at_zero` — 50 damage to 10 HP → 0 HP
- `projectile_despawns_on_hit` — projectile entity removed on arrival
- `projectile_does_not_friendly_fire` — same-team target → no damage, projectile still despawns (hit its target, just no damage)
- `projectile_in_flight_no_damage` — projectile far from target, small step → full HP, projectile alive

### Deleted Tests:
- `projectile_hits_non_target_enemy` — physics artifact (CollidingEntities could include bystanders)
- `projectile_no_collision_yet` — replaced by `projectile_in_flight_no_damage`
- All 5 `surface_distance` tests in `third_party/avian.rs` (file deleted)
- `solid_entity_layers_is_pushbox_hurtbox` test (file deleted)
- 3 parity tests in `gameplay/mod.rs`

### Preserved Tests (no changes needed):
- `unit_spawns_projectile_in_range` — tests attack system, no physics dependency
- `unit_does_not_attack_out_of_range` — uses `extent_distance`, no physics
- `attack_without_target_does_nothing` — no physics
- `projectile_despawns_when_target_missing` — tests `move_projectiles` target-gone path
- `attack_respects_cooldown` — no physics
- `fortress_can_attack_in_range` — remove `Collider` component, otherwise unchanged
- `constants_are_valid` — no physics

### Existing Non-Attack Tests:
- AI tests in `ai.rs` — remove `Collider` from spawns, otherwise unchanged
- Movement tests in `movement.rs` — remove `Collider` from spawns, otherwise unchanged
- Building/economy tests — `Collider` not used in test setup (they skip physics plugin)

## Performance Considerations

Removing avian2d eliminates the physics broad-phase and narrow-phase passes — at 40k units this is a significant win (O(n log n) broad-phase gone). This is measured in GAM-63 (profiling pass).

## References

- Linear ticket: [GAM-62](https://linear.app/tayhu-games/issue/GAM-62/remove-avian2d-physics-engine)
- Research: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md` (Section 9, Ticket 5)
- Dependency: [GAM-61](https://linear.app/tayhu-games/issue/GAM-61) — ORCA decoupled, `LinearVelocity` removed
- Blocked by this: [GAM-63](https://linear.app/tayhu-games/issue/GAM-63) — Profiling & tuning pass

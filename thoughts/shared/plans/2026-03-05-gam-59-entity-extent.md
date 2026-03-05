# EntityExtent: Replace GJK surface_distance (GAM-59) Implementation Plan

## Overview

Add `EntityExtent` component to all targetable entities and replace GJK-based `surface_distance()` calls with a simple geometric `extent_distance()` free function. This decouples combat range checks from the physics engine, preparing for physics removal in Ticket 5.

## Current State Analysis

Three systems call `surface_distance()` (from `third_party/avian.rs`), which delegates to avian2d's GJK contact query:

| System | File | Line | What it does |
|--------|------|------|-------------|
| `search_radius()` | `gameplay/ai.rs` | 274 | Finds nearest target by surface distance |
| `unit_movement()` | `gameplay/units/movement.rs` | 65 | Checks if unit is within attack range |
| `attack()` | `gameplay/combat/attack.rs` | 78 | Checks if attacker is within firing range |

All three query `&Collider` on both the seeker/attacker and the target entity. The `Collider` component is also used by avian2d physics (pushbox/hurtbox layers), so it must remain until Ticket 5.

### Entity Extent Values

| Entity | Current Collider | EntityExtent | Source |
|--------|-----------------|--------------|--------|
| Unit | `Collider::circle(6.0)` | `Circle(6.0)` | `UNIT_RADIUS` |
| Fortress | `Collider::rectangle(128.0, 128.0)` | `Rect(64.0, 64.0)` | `FORTRESS_COLS * CELL_SIZE / 2` |
| Building | `Collider::rectangle(40.0, 40.0)` | `Rect(20.0, 20.0)` | `BUILDING_SPRITE_SIZE / 2` |
| Test target | `Collider::circle(5.0)` | `Circle(5.0)` | Hardcoded in `spawn_test_target` |

Note: `Collider::rectangle` takes **full** width/height. `EntityExtent::Rect` takes **half** extents.

### Key Discoveries

- `surface_distance()` at `third_party/avian.rs:50` wraps `contact_query::distance()` — returns surface-to-surface distance, 0 for overlap
- The `MAX_ENTITY_HALF_EXTENT` constant in `ai.rs:24` (64.0) is already correct for the new system
- `handle_projectile_hits` uses `CollidingEntities` (physics), NOT `surface_distance` — untouched by this ticket
- `death.rs` test spawns (`spawn_mortal_target`, inline `(Team::Enemy, Target)`) have no `Collider` and the death system doesn't query `EntityExtent` — no changes needed

## Desired End State

- `EntityExtent` component on every entity that has `Collider` + (`Target` or `TargetingState`)
- All three consumer systems use `extent_distance()` instead of GJK `surface_distance()`
- No game system imports `surface_distance` from `third_party` (the wrapper stays in the file, removed in Ticket 5)
- `Collider` still on all entities for physics — unchanged
- All existing tests pass with equivalent behavior
- New unit tests for `surface_distance_from()` and `extent_distance()` math

## What We're NOT Doing

- NOT removing `Collider` from any entity (that's Ticket 5)
- NOT removing `surface_distance()` from `third_party/avian.rs` (still exists, just unused by game systems)
- NOT changing `TargetingState` logic or adding new states
- NOT modifying the spatial hash or target grid
- NOT changing projectile hit detection (`handle_projectile_hits` uses `CollidingEntities`, not `surface_distance`)

## Implementation Approach

Additive then migrate: add the component and math first, then add to all spawn sites, then swap each consumer system. Each phase leaves the game working.

---

## Phase 1: EntityExtent Component + Math

### Overview

Define `EntityExtent` enum and `extent_distance()` free function in `gameplay/mod.rs`. Add unit tests for the math.

### Changes Required

#### 1. `gameplay/mod.rs` — Add EntityExtent enum

After the `CombatStats` component definition:

```rust
/// Physical extent of a targetable entity, used for surface-distance range checks.
/// Replaces GJK `surface_distance()` with simple geometry.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub enum EntityExtent {
    /// Circular extent with the given radius (units).
    Circle(f32),
    /// Rectangular extent with half-width and half-height (fortresses, buildings).
    Rect(f32, f32),
}

impl EntityExtent {
    /// Minimum distance from `point` to the surface of this extent centered at `self_pos`.
    /// Returns 0.0 if the point is inside or overlapping.
    #[must_use]
    pub fn surface_distance_from(&self, self_pos: Vec2, point: Vec2) -> f32 {
        match self {
            Self::Circle(r) => (self_pos.distance(point) - r).max(0.0),
            Self::Rect(hw, hh) => {
                let d = (point - self_pos).abs();
                let dx = (d.x - hw).max(0.0);
                let dy = (d.y - hh).max(0.0);
                (dx * dx + dy * dy).sqrt()
            }
        }
    }
}
```

#### 2. `gameplay/mod.rs` — Add `extent_distance()` free function

Drop-in replacement for `surface_distance()` with identical semantics (surface-to-surface distance between two shapes, 0.0 if overlapping). Handles all three shape pair combinations: circle-circle, circle-rect, rect-rect.

```rust
/// Surface-to-surface distance between two extents. Returns 0.0 if overlapping.
/// Drop-in replacement for `third_party::surface_distance()`.
#[must_use]
pub fn extent_distance(a: &EntityExtent, a_pos: Vec2, b: &EntityExtent, b_pos: Vec2) -> f32 {
    match (a, b) {
        (EntityExtent::Circle(r1), EntityExtent::Circle(r2)) => {
            (a_pos.distance(b_pos) - r1 - r2).max(0.0)
        }
        (EntityExtent::Circle(r), EntityExtent::Rect(hw, hh))
        | (EntityExtent::Rect(hw, hh), EntityExtent::Circle(r)) => {
            let (circle_pos, rect_pos) = if matches!(a, EntityExtent::Circle(_)) {
                (a_pos, b_pos)
            } else {
                (b_pos, a_pos)
            };
            let rect = EntityExtent::Rect(*hw, *hh);
            (rect.surface_distance_from(rect_pos, circle_pos) - r).max(0.0)
        }
        (EntityExtent::Rect(hw1, hh1), EntityExtent::Rect(hw2, hh2)) => {
            let d = (a_pos - b_pos).abs();
            let dx = (d.x - hw1 - hw2).max(0.0);
            let dy = (d.y - hh1 - hh2).max(0.0);
            (dx * dx + dy * dy).sqrt()
        }
    }
}
```

#### 3. `gameplay/mod.rs` — Register type

```rust
app.register_type::<EntityExtent>()
```

#### 4. Unit tests

Tests for `surface_distance_from()` (point-to-surface) and `extent_distance()` (surface-to-surface):

- **Circle**: outside, inside (returns 0), on surface (returns 0)
- **Rect**: outside along axis, outside at corner (diagonal), inside (returns 0)
- **extent_distance circle-circle**: separated, overlapping
- **extent_distance circle-rect**: unit-to-fortress scenario, unit-to-building
- **extent_distance rect-rect**: fortress-to-building, overlapping
- **Parity**: compare `extent_distance()` against `surface_distance()` for representative shapes to confirm mathematical equivalence before migration

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — new unit tests for both functions

#### Manual Verification:
- [ ] None needed — pure data + math, no visual behavior

---

## Phase 2: Add EntityExtent to Spawn Sites + Test Helpers

### Overview

Add `EntityExtent` to every entity that currently has `Collider` + (`Target` or `TargetingState`). Additive only — no behavior change.

### Changes Required

#### 1. Production spawn sites

| File | Entity | EntityExtent value |
|------|--------|-------------------|
| `gameplay/units/mod.rs:99` — `spawn_unit()` | Unit | `EntityExtent::Circle(UNIT_RADIUS)` |
| `gameplay/battlefield/renderer.rs:66` | Player fortress | `EntityExtent::Rect(fortress_size.x / 2.0, fortress_size.y / 2.0)` |
| `gameplay/battlefield/renderer.rs:148` | Enemy fortress | `EntityExtent::Rect(fortress_size.x / 2.0, fortress_size.y / 2.0)` |
| `gameplay/building/placement.rs:125` | Building | `EntityExtent::Rect(BUILDING_SPRITE_SIZE / 2.0, BUILDING_SPRITE_SIZE / 2.0)` |

#### 2. Test helpers

| File | Helper | EntityExtent value |
|------|--------|-------------------|
| `testing.rs:154` — `spawn_test_unit()` | Test unit | `EntityExtent::Circle(UNIT_RADIUS)` |
| `testing.rs:194` — `spawn_test_target()` | Test target | `EntityExtent::Circle(5.0)` |

#### 3. Inline test entity spawns

| File | Test | EntityExtent value |
|------|------|-------------------|
| `gameplay/ai.rs:453` | `fortress_targets_nearest_enemy` | `EntityExtent::Rect(64.0, 64.0)` |
| `gameplay/ai.rs:482` | `static_entity_has_no_backtrack_limit` | `EntityExtent::Rect(64.0, 64.0)` |
| `gameplay/combat/attack.rs:510` | `fortress_can_attack_in_range` | `EntityExtent::Rect(64.0, 64.0)` |

#### 4. NOT changed

- `gameplay/combat/death.rs:67` — `spawn_mortal_target`: has `Target` but no `Collider`. Death system doesn't query `EntityExtent`.
- `gameplay/combat/death.rs:107` — inline `(Team::Enemy, Target)`: same reason.

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — all existing tests still pass

#### Manual Verification:
- [ ] None needed — additive only, no behavior change

---

## Phase 3: Migrate Consumer Systems

### Overview

Replace `&Collider` queries and `surface_distance()` calls with `&EntityExtent` queries and `extent_distance()` in all three consumer systems.

### Changes Required

#### 1. `gameplay/ai.rs` — Migrate targeting

**Import changes**:
- Remove `use avian2d::prelude::*;` (only used for `Collider` in queries)
- Remove `use crate::third_party::surface_distance;`
- Add `use super::extent_distance;`

**Query changes** in `find_target()`:
- Line 104: `&Collider` → `&EntityExtent` in `seekers` query
- Line 108: `&Collider` → `&EntityExtent` in `all_targets` query

**Helper function signature changes**:
- `find_nearest_target()` (line 163): `seeker_collider: &Collider` → `seeker_extent: &EntityExtent`
- `find_nearest_target()` (line 167): `&Collider` → `&EntityExtent` in `all_targets` query type
- `search_radius()` (line 206): `seeker_collider: &Collider` → `seeker_extent: &EntityExtent`
- `search_radius()` (line 210): `&Collider` → `&EntityExtent` in `all_targets` query type
- `valid_candidates` type (line 215): `Vec<(Entity, Vec2, &Collider, f32)>` → `Vec<(Entity, Vec2, &EntityExtent, f32)>`

**Logic change** in `search_radius()`:
```rust
// Before (line 274):
let surf_dist = surface_distance(seeker_collider, seeker_pos, cand_collider, *cand_pos);

// After:
let surf_dist = extent_distance(seeker_extent, seeker_pos, cand_extent, *cand_pos);
```

#### 2. `gameplay/units/movement.rs` — Migrate movement range check

**Import changes**:
- Remove `use avian2d::prelude::Collider;`
- Remove `use crate::third_party::surface_distance;`
- Add `use crate::gameplay::extent_distance;`

**Query changes** in `unit_movement()`:
- Line 36: `&Collider` → `&EntityExtent` in `units` query
- Line 42: `(&GlobalTransform, &Collider)` → `(&GlobalTransform, &EntityExtent)` in `targets` query

**Logic change**:
```rust
// Before (line 65-66):
let distance_to_target =
    surface_distance(unit_collider, current_xy, target_collider, target_xy);

// After:
let distance_to_target =
    extent_distance(unit_extent, current_xy, target_extent, target_xy);
```

#### 3. `gameplay/combat/attack.rs` — Migrate attack range check

**Import changes**:
- Remove `surface_distance` from `use crate::third_party::{CollisionLayer, surface_distance};`
- Add `use crate::gameplay::extent_distance;`
- Keep `use avian2d::prelude::*;` — still needed for `RigidBody`, `Collider`, `Sensor`, `CollisionLayers`, `CollisionEventsEnabled`, `CollidingEntities` in projectile spawning and hit detection

**Query changes** in `attack()`:
- Line 57: `&Collider` → `&EntityExtent` in `attackers` query
- Line 60: `(&GlobalTransform, &Collider)` → `(&GlobalTransform, &EntityExtent)` in `targets` query

**Logic change**:
```rust
// Before (line 78-83):
let distance = surface_distance(
    attacker_collider, attacker_pos.translation().xy(),
    target_collider, target_pos.translation().xy(),
);

// After:
let distance = extent_distance(
    attacker_extent, attacker_pos.translation().xy(),
    target_extent, target_pos.translation().xy(),
);
```

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes (no unused imports, no missing components)
- [ ] `make test` passes — all existing tests produce equivalent results

#### Manual Verification:
- [ ] Units still target and attack correctly
- [ ] Fortresses still fire at enemies in range
- [ ] Buildings are targetable by enemy units

**Implementation Note**: After completing this phase and all automated verification passes, pause for manual confirmation before considering the ticket done.

---

## Testing Strategy

### Unit Tests (Phase 1)
- `surface_distance_from()` — circle and rect variants, inside/outside/on-surface
- `extent_distance()` — all three shape pairs (circle-circle, circle-rect, rect-rect), overlapping cases
- Parity tests comparing `extent_distance()` against `surface_distance()` for representative configurations

### Integration Tests (Phase 2-3)
- All existing AI, movement, and attack tests pass unchanged (same behavior, different implementation)
- No new integration tests needed — the component is tested via existing system tests

## Performance Considerations

- `extent_distance()` is ~4-8 FLOPs per call vs GJK's ~10-20 iterations (each with several FLOPs)
- At 40k units, the AI system calls this in `search_radius` for each candidate — savings compound
- No allocations, no trait objects, pure arithmetic on `f32` values

## Verified API Patterns (Bevy 0.18)

Verified against codebase patterns (matches `TargetingState` added in GAM-58):

- Enum component derives: `Component, Debug, Clone, Copy, Reflect` + `#[reflect(Component)]`
- Registration: `app.register_type::<EntityExtent>()` chained in `gameplay::plugin`
- Import path: `use crate::gameplay::{EntityExtent, extent_distance};`
- `impl` block on derived `Component` enum is allowed (used by `TargetingState::target_entity()`)

## References

- Linear ticket: [GAM-59](https://linear.app/tayhu-games/issue/GAM-59/combat-state-and-range-simplification-entityextent)
- Research: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md` (Section 9, Ticket 2)
- Blocked by: [GAM-58](https://linear.app/tayhu-games/issue/GAM-58) (TargetingState migration) — DONE
- Blocks: [GAM-60](https://linear.app/tayhu-games/issue/GAM-60) (Flow field infrastructure)

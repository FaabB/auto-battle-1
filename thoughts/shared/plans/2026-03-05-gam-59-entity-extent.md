# EntityExtent: Replace GJK surface_distance (GAM-59) Implementation Plan

## Overview

Add `EntityExtent` component to all targetable entities and replace GJK-based `surface_distance()` calls with `EntityExtent::surface_distance_from()` in the AI, movement, and attack systems. This decouples combat range checks from the physics engine, preparing for physics removal in Ticket 5.

## Current State Analysis

Three systems call `surface_distance()` (from `third_party/avian.rs`), which delegates to avian2d's GJK contact query:

1. **`gameplay/ai.rs:274`** — `search_radius()` computes surface distance between seeker and candidate targets to find the nearest one
2. **`gameplay/units/movement.rs:65-66`** — `unit_movement()` checks if unit is within attack range of target
3. **`gameplay/combat/attack.rs:78-83`** — `attack()` checks if attacker is within range of target

All three query `&Collider` on both the seeker/attacker and the target entity. The `Collider` component is also used by avian2d physics (pushbox/hurtbox layers), so it must remain until Ticket 5.

### Entity Extent Values

| Entity | Current Collider | EntityExtent |
|--------|-----------------|--------------|
| Unit | `Collider::circle(6.0)` | `Circle(6.0)` — `UNIT_RADIUS` |
| Fortress | `Collider::rectangle(128.0, 128.0)` | `Rect(64.0, 64.0)` — half-extents |
| Building | `Collider::rectangle(40.0, 40.0)` | `Rect(20.0, 20.0)` — `BUILDING_SPRITE_SIZE / 2.0` |
| Test target | `Collider::circle(5.0)` | `Circle(5.0)` |

### Key Discoveries

- `surface_distance()` at `third_party/avian.rs:50` wraps `contact_query::distance()` — returns signed distance, 0 for overlap
- `Collider::rectangle(w, h)` takes **full** width/height. `EntityExtent::Rect(hw, hh)` takes **half** extents.
- The `MAX_ENTITY_HALF_EXTENT` constant in `ai.rs:24` (64.0) is already correct for the new system
- Test helpers `spawn_test_unit` and `spawn_test_target` in `testing.rs` need `EntityExtent` added

## Desired End State

- `EntityExtent` component on every entity that has a `Target` marker or a `TargetingState`
- All three consumer systems (`search_radius`, `unit_movement`, `attack`) use `EntityExtent::surface_distance_from()` instead of GJK `surface_distance()`
- No game system imports `surface_distance` from `third_party` (it stays in the file for now, removed in Ticket 5)
- `Collider` still on all entities for physics — unchanged
- All existing tests pass with equivalent behavior
- New unit tests for `EntityExtent::surface_distance_from()` math

## What We're NOT Doing

- NOT removing `Collider` from any entity (that's Ticket 5)
- NOT removing `surface_distance()` from `third_party/avian.rs` (still exists, just unused by game systems)
- NOT changing `TargetingState` logic or adding new states
- NOT modifying the spatial hash or target grid
- NOT changing projectile hit detection (`handle_projectile_hits` uses `CollidingEntities`, not `surface_distance`)

## Implementation Approach

Additive then migrate: add the component and math first, then swap each consumer system one at a time. Each phase leaves the game working.

---

## Phase 1: EntityExtent Component + Math

### Overview
Define the `EntityExtent` enum in `gameplay/mod.rs` with `surface_distance_from()`. Add unit tests for the math.

### Changes Required

#### 1. `gameplay/mod.rs` — Add EntityExtent
**After** the `CombatStats` component definition, add:

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
    /// Returns 0.0 if the point is inside.
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

Note: The `Circle` case clamps to 0.0 with `.max(0.0)` to match GJK behavior (returns 0 for overlap).

#### 2. `gameplay/mod.rs` — Register type
Add `EntityExtent` to the plugin's type registrations:

```rust
app.register_type::<EntityExtent>()
```

#### 3. Unit tests for `EntityExtent::surface_distance_from()`
Add tests in `gameplay/mod.rs` (new `#[cfg(test)] mod tests` block, or extend the existing one if present):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Circle tests
    #[test]
    fn circle_surface_distance_outside() {
        let extent = EntityExtent::Circle(10.0);
        let dist = extent.surface_distance_from(Vec2::ZERO, Vec2::new(25.0, 0.0));
        assert!((dist - 15.0).abs() < 0.01);
    }

    #[test]
    fn circle_surface_distance_inside() {
        let extent = EntityExtent::Circle(10.0);
        let dist = extent.surface_distance_from(Vec2::ZERO, Vec2::new(5.0, 0.0));
        assert_eq!(dist, 0.0);
    }

    #[test]
    fn circle_surface_distance_on_surface() {
        let extent = EntityExtent::Circle(10.0);
        let dist = extent.surface_distance_from(Vec2::ZERO, Vec2::new(10.0, 0.0));
        assert!(dist < 0.01);
    }

    // Rect tests
    #[test]
    fn rect_surface_distance_outside_axis() {
        let extent = EntityExtent::Rect(64.0, 64.0);
        let dist = extent.surface_distance_from(Vec2::ZERO, Vec2::new(100.0, 0.0));
        assert!((dist - 36.0).abs() < 0.01);
    }

    #[test]
    fn rect_surface_distance_outside_corner() {
        let extent = EntityExtent::Rect(64.0, 64.0);
        let dist = extent.surface_distance_from(Vec2::ZERO, Vec2::new(67.0, 67.0));
        // dx = 3, dy = 3 → sqrt(9+9) ≈ 4.24
        assert!((dist - (3.0f32 * 2.0).sqrt()).abs() < 0.01);
    }

    #[test]
    fn rect_surface_distance_inside() {
        let extent = EntityExtent::Rect(64.0, 64.0);
        let dist = extent.surface_distance_from(Vec2::ZERO, Vec2::new(30.0, 30.0));
        assert_eq!(dist, 0.0);
    }

    // Cross-type: point distance from circle to rect (simulating unit → fortress)
    #[test]
    fn unit_to_fortress_distance() {
        // Unit (circle r=6) at x=100, fortress (rect 64×64) at origin
        // Unit surface at x=94, fortress surface at x=64 → gap = 30
        let unit_extent = EntityExtent::Circle(6.0);
        let fortress_extent = EntityExtent::Rect(64.0, 64.0);
        let unit_pos = Vec2::new(100.0, 0.0);
        let fortress_pos = Vec2::ZERO;

        // Distance from fortress surface to unit center
        let d_fortress = fortress_extent.surface_distance_from(fortress_pos, unit_pos);
        // Distance from unit surface to fortress center (approx)
        let d_unit = unit_extent.surface_distance_from(unit_pos, fortress_pos);

        // For surface-to-surface: fortress reports 36, unit reports 94
        // The correct surface-to-surface distance is 30 (gap between surfaces)
        // We use: target.surface_distance_from(target_pos, seeker_pos) - seeker_radius
        // OR: seeker.surface_distance_from(seeker_pos, closest_point_on_target)
        // The ticket's approach: target.surface_distance_from(target_pos, attacker_pos)
        // This gives distance from attacker CENTER to target SURFACE.
        // For range checks, this is what we want: "how far is my center from the target surface?"
        assert!((d_fortress - 36.0).abs() < 0.01);
    }
}
```

**Important design note**: The consumer systems use `target_extent.surface_distance_from(target_pos, seeker_pos)` — distance from **seeker center** to **target surface**. This differs from the current GJK which gives surface-to-surface distance between both colliders. The difference is the seeker's own radius (6px for units). This means `CombatStats.range` values may need a small adjustment, OR we compute the full surface-to-surface distance.

### Design Decision: Surface Distance Calculation

The current `surface_distance(c1, pos1, c2, pos2)` gives the distance between the **surfaces** of both colliders. The ticket's `EntityExtent::surface_distance_from()` gives distance from a **point** to the entity's surface.

To replicate the current behavior exactly (surface-to-surface), we need:

```rust
// In consumer systems:
let dist = target_extent.surface_distance_from(target_pos, seeker_pos)
    - seeker_extent.surface_distance_from(seeker_pos, target_pos); // Nope, this double-counts
```

Actually, for two shapes, surface-to-surface distance = center-to-center - extent1 - extent2 (approximately). The simplest correct approach for mixed shapes:

```rust
/// Surface-to-surface distance between two extents.
pub fn surface_distance_between(
    a: &EntityExtent, a_pos: Vec2,
    b: &EntityExtent, b_pos: Vec2,
) -> f32 {
    // Distance from a's surface to b's center, minus b's extent toward a
    // This is exact for circle-circle and circle-axis-aligned-rect
    let d_a_to_b_center = a.surface_distance_from(a_pos, b_pos);
    // But for rect-rect corner cases, we need a different approach
    // Simplest correct: closest point on A to B's center, then distance from that to B's surface
    ...
}
```

This gets complex. The ticket code in the Linear issue actually specifies the simpler approach. Let me re-read it...

The ticket says:
> Replace GJK `surface_distance` calls with `EntityExtent::surface_distance_from()`

And shows the function returning distance from a point to the shape's surface. The consumer systems currently use `surface_distance(seeker_collider, seeker_pos, target_collider, target_pos)` (surface-to-surface between two shapes).

To keep it simple and match the ticket spec, I'll add a **free function** that computes surface-to-surface distance between two `EntityExtent`s. For the shapes we have (circle-circle, circle-rect), this is straightforward:

```rust
/// Surface-to-surface distance between two extents. Returns 0.0 if overlapping.
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

This replaces `surface_distance()` as a drop-in with the same semantics.

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — new unit tests for `surface_distance_from()` and `extent_distance()`

#### Manual Verification:
- [ ] None needed — pure data + math, no visual behavior

---

## Phase 2: Add EntityExtent to Spawn Sites + Test Helpers

### Overview
Add `EntityExtent` to every entity that currently has `Collider` + `Target` (or `TargetingState`).

### Changes Required

#### 1. `gameplay/units/mod.rs` — `spawn_unit()`
Add `EntityExtent::Circle(UNIT_RADIUS)` to the spawn bundle (alongside existing `Collider::circle(UNIT_RADIUS)`).

#### 2. `gameplay/battlefield/renderer.rs` — Fortresses
Add `EntityExtent::Rect(fortress_size.x / 2.0, fortress_size.y / 2.0)` to both Player and Enemy fortress spawns. This equals `EntityExtent::Rect(64.0, 64.0)`.

#### 3. `gameplay/building/placement.rs` — Buildings
Add `EntityExtent::Rect(BUILDING_SPRITE_SIZE / 2.0, BUILDING_SPRITE_SIZE / 2.0)` to the building spawn. This equals `EntityExtent::Rect(20.0, 20.0)`.

#### 4. `testing.rs` — Test helpers
- `spawn_test_unit()`: Add `EntityExtent::Circle(UNIT_RADIUS)`
- `spawn_test_target()`: Add `EntityExtent::Circle(5.0)` (matches `Collider::circle(5.0)`)

#### 5. Test fortress spawns in `ai.rs` tests
The `fortress_targets_nearest_enemy` and `static_entity_has_no_backtrack_limit` tests manually spawn fortress-like entities with `Collider::rectangle(128.0, 128.0)`. Add `EntityExtent::Rect(64.0, 64.0)` to these spawns.

#### 6. Test fortress spawn in `attack.rs` tests
The `fortress_can_attack_in_range` test (line 510-520) spawns a fortress-like attacker entity with `Collider::rectangle(128.0, 128.0)`. Add `EntityExtent::Rect(64.0, 64.0)`. This entity has `TargetingState` and is queried by the `attack` system (which will query `&EntityExtent` after migration).

#### 7. `death.rs` test spawns — NO CHANGES NEEDED
`spawn_mortal_target` (line 67-77) and inline `(Team::Enemy, Target)` (line 107) have `Target` but no `Collider`. The death system doesn't query `EntityExtent`, so these are fine as-is.

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — all existing tests still pass

#### Manual Verification:
- [ ] None needed — additive only, no behavior change

---

## Phase 3: Migrate Consumer Systems

### Overview
Replace `surface_distance()` calls with `extent_distance()` in all three consumer systems: AI targeting, unit movement, and attack.

### Changes Required

#### 1. `gameplay/ai.rs` — Migrate `find_target` + `search_radius`

**Query changes**:
- `seekers` query (line 104): Replace `&Collider` with `&EntityExtent`
- `all_targets` query (line 108): Replace `&Collider` with `&EntityExtent`
- `find_nearest_target` signature (line 163): Replace `seeker_collider: &Collider` with `seeker_extent: &EntityExtent`
- `find_nearest_target` signature (line 167): Replace `&Collider` in `all_targets` query type with `&EntityExtent`
- `search_radius` signature (line 206): Replace `seeker_collider: &Collider` with `seeker_extent: &EntityExtent`
- `search_radius` signature (line 210): Replace `&Collider` in `all_targets` query type with `&EntityExtent`
- `valid_candidates` type (line 215): Change `Vec<(Entity, Vec2, &Collider, f32)>` to `Vec<(Entity, Vec2, &EntityExtent, f32)>`

**Logic change** in `search_radius()`:
```rust
// Before (line 274):
let surf_dist = surface_distance(seeker_collider, seeker_pos, cand_collider, *cand_pos);

// After:
let surf_dist = extent_distance(seeker_extent, seeker_pos, cand_extent, *cand_pos);
```

**Import change**: Remove `use crate::third_party::surface_distance;`, add `use super::extent_distance;`

Also remove `use avian2d::prelude::*;` (the only avian import was for `Collider`). If there's nothing else from avian used, this import is now dead.

#### 2. `gameplay/units/movement.rs` — Migrate `unit_movement`

**Query changes**:
- `units` query: Replace `&Collider` with `&EntityExtent`
- `targets` query: Replace `(&GlobalTransform, &Collider)` with `(&GlobalTransform, &EntityExtent)`

**Logic change**:
```rust
// Before (line 65-66):
let distance_to_target =
    surface_distance(unit_collider, current_xy, target_collider, target_xy);

// After:
let distance_to_target =
    extent_distance(unit_extent, current_xy, target_extent, target_xy);
```

**Import change**: Remove `use avian2d::prelude::Collider;` and `use crate::third_party::surface_distance;`, add `use crate::gameplay::extent_distance;` (or `use super::super::extent_distance;`).

#### 3. `gameplay/combat/attack.rs` — Migrate `attack`

**Query changes**:
- `attackers` query: Replace `&Collider` with `&EntityExtent`
- `targets` query: Replace `(&GlobalTransform, &Collider)` with `(&GlobalTransform, &EntityExtent)`

**Logic change**:
```rust
// Before (line 78-83):
let distance = surface_distance(
    attacker_collider,
    attacker_pos.translation().xy(),
    target_collider,
    target_pos.translation().xy(),
);

// After:
let distance = extent_distance(
    attacker_extent,
    attacker_pos.translation().xy(),
    target_extent,
    target_pos.translation().xy(),
);
```

**Import change**: Remove `use crate::third_party::{CollisionLayer, surface_distance};`, change to `use crate::third_party::CollisionLayer;`, add `use crate::gameplay::extent_distance;`.

Remove `use avian2d::prelude::*;` — check if anything else from avian is used (yes: `RigidBody`, `Collider`, `Sensor`, `CollisionLayers`, `CollisionEventsEnabled`, `CollidingEntities` are all used in the projectile spawn and hit detection). Keep the avian import.

Actually, looking again at `attack.rs:3`, the `avian2d::prelude::*` is used for `RigidBody`, `Collider`, `Sensor`, `CollisionLayers`, `CollisionEventsEnabled`, `CollidingEntities` in the projectile spawn and `CollidingEntities` in hit detection. So `use avian2d::prelude::*` stays. Only `surface_distance` is removed from imports.

#### 4. Update tests

Most tests already spawn entities with `Collider` which worked because the systems queried `&Collider`. After migration, systems query `&EntityExtent`, so test entities need `EntityExtent`. Phase 2 already added `EntityExtent` to all test helpers and manual fortress spawns, so existing tests should work.

**Verify**: No test manually creates entities with `Collider` but without `EntityExtent` that are then queried by the migrated systems. The Phase 2 changes should cover all cases.

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes (no unused imports, no missing components)
- [ ] `make test` passes — all existing tests produce equivalent results

#### Manual Verification:
- [ ] Play test: units still target and attack correctly
- [ ] Fortresses still fire at enemies in range
- [ ] Buildings are targetable by enemy units

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the manual testing was successful before proceeding.

---

## Testing Strategy

### Unit Tests (Phase 1)
- `EntityExtent::surface_distance_from()` — circle outside, inside, on surface
- `EntityExtent::surface_distance_from()` — rect outside axis, outside corner, inside
- `extent_distance()` — circle-circle, circle-rect, rect-rect, overlapping cases
- Parity tests: verify `extent_distance()` matches `surface_distance()` for the same shapes/positions

### Integration Tests (Phase 2-3)
- All existing AI, movement, and attack tests pass unchanged (same behavior, different implementation)
- No new integration tests needed — the component is tested via existing system tests

### Parity Validation
Add temporary tests in Phase 1 that compare `extent_distance()` against `surface_distance()` for representative entity configurations. These confirm mathematical equivalence before migration. Can be removed after Phase 3 passes.

## Performance Considerations

- `extent_distance()` is ~4-8 FLOPs per call vs GJK's ~10-20 iterations (each with several FLOPs)
- At 40k units, the AI system calls this in `search_radius` for each candidate — the savings add up
- No allocations, no trait objects, pure arithmetic on `f32` values

## Verified API Patterns (Bevy 0.18)

These were verified against the actual crate source and existing codebase:

- `Component` derive + `Debug, Clone, Copy, Reflect` + `#[reflect(Component)]` for enum components — matches `TargetingState` pattern
- `app.register_type::<EntityExtent>()` in plugin function — required for reflection
- `impl` block on derived `Component` enum — allowed, used by `TargetingState::target_entity()`

## References

- Linear ticket: [GAM-59](https://linear.app/tayhu-games/issue/GAM-59/combat-state-and-range-simplification-entityextent)
- Research: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md` (Section 9, Ticket 2)
- Blocked by: [GAM-58](https://linear.app/tayhu-games/issue/GAM-58) (TargetingState migration) — DONE
- Blocks: [GAM-60](https://linear.app/tayhu-games/issue/GAM-60) (Flow field infrastructure)

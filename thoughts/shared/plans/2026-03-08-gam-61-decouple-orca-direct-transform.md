# Decouple ORCA from Physics + Direct Transform Movement (GAM-61)

## Overview

Replace avian2d physics integration with an explicit `apply_movement` system that writes ORCA-adjusted velocity directly to `Transform.translation`. Keep the ORCA algorithm unchanged — only decouple its output from the physics engine. This is the prerequisite for GAM-62 (remove avian2d entirely).

## Current State Analysis

Unit movement is a 4-stage pipeline in `GameSet::Movement`:

1. `unit_movement` (`movement.rs:20`) — reads flow field / target steering, writes `PreferredVelocity`
2. `rebuild_spatial_hash` (`avoidance/mod.rs:99`) — rebuilds `AvoidanceSpatialHash` from all unit positions
3. `compute_avoidance` (`avoidance/mod.rs:114`) — reads `PreferredVelocity` + `LinearVelocity`, runs ORCA, writes result to `LinearVelocity`
4. **Avian2d physics** (internal) — integrates `LinearVelocity` into `Transform.translation`, resolves pushbox collisions

Units spawn as `RigidBody::Dynamic` with `Collider`, `CollisionLayers`, `LockedAxes`, and `LinearVelocity` (`units/mod.rs:128-143`).

### Key Discoveries:
- `compute_avoidance` uses `LinearVelocity` for both input (smoothing blend from last frame) and output — `avoidance/mod.rs:121,204-206`
- `orca.rs` is pure math (no Bevy/avian2d imports) — zero changes needed
- `debug_draw_avoidance` in `dev_tools/mod.rs:127-145` reads `LinearVelocity` for the cyan "actual velocity" arrow
- `LinearVelocity` is imported from `avian2d::prelude::*` in: `avoidance/mod.rs`, `units/mod.rs`, `testing.rs`, `dev_tools/mod.rs`, `gameplay/mod.rs`
- Only `compute_avoidance` writes `LinearVelocity` on units — no other system in our codebase does
- Test helpers: `spawn_test_unit` (`testing.rs:154`), `spawn_avoidance_unit` (`avoidance/mod.rs:229`), both spawn `LinearVelocity::ZERO`

## Desired End State

After this plan:
- `compute_avoidance` writes to `AdjustedVelocity` (our own component) instead of `LinearVelocity` (avian2d)
- New `apply_movement` system writes `AdjustedVelocity * dt` to `Transform.translation`
- Units are `RigidBody::Kinematic` — physics doesn't drive movement, colliders remain for GAM-62 to clean up
- `LinearVelocity` is removed from all unit entities
- Movement behavior is identical — same ORCA algorithm, same smoothing, just a different integration path

### Verification:
- `make check` passes (no compilation errors, no clippy warnings)
- `make test` passes (all existing tests updated)
- Manual: units move correctly with flow field + ORCA avoidance, no oscillation, no stuck units
- Manual: F3 debug overlay still shows preferred (green) and adjusted (cyan) velocity arrows

## What We're NOT Doing

- **Not changing ORCA algorithm** — `orca.rs` is untouched
- **Not upgrading spatial hash** — flat `Vec<Vec<Entity>>` grid deferred to GAM-63 profiling pass
- **Not removing Colliders/RigidBody** — that's GAM-62
- **Not changing flow field or targeting** — only the ORCA output → Transform integration path

## Implementation Approach

Single phase — the changes are tightly coupled (can't half-migrate) but small in scope. Every file change is mechanical: replace `LinearVelocity` with `AdjustedVelocity` and add one new system.

## Phase 1: Decouple ORCA + Direct Transform Movement

### Overview
Add `AdjustedVelocity` component, rewire `compute_avoidance` output, add `apply_movement` system, switch to kinematic physics, remove `LinearVelocity` from units.

### Changes Required:

#### 1. New component: `AdjustedVelocity`
**File**: `src/gameplay/units/avoidance/mod.rs`
**Changes**: Add component after `PreferredVelocity` definition (line 29)

```rust
/// The velocity after ORCA adjustment. Written by `compute_avoidance`,
/// read by `apply_movement` to update `Transform` directly.
/// Also read next frame by `compute_avoidance` for velocity smoothing.
#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component)]
pub struct AdjustedVelocity(pub Vec2);
```

#### 2. Rewire `compute_avoidance`
**File**: `src/gameplay/units/avoidance/mod.rs`
**Changes**: Replace `LinearVelocity` with `AdjustedVelocity` in query and writes

- Line 7: Remove `use avian2d::prelude::*;`
- Line 121: `&mut LinearVelocity` → `&mut AdjustedVelocity`
- Line 138: `velocity: velocity.0` (unchanged — `AdjustedVelocity` has same `.0` field)
- Line 204: `mut linear_vel` → `mut adjusted_vel`, `linear_vel.0` → `adjusted_vel.0`
- Update doc comment (lines 111-112) to reference `AdjustedVelocity`

#### 3. New system: `apply_movement`
**File**: `src/gameplay/units/avoidance/mod.rs`
**Changes**: Add system after `compute_avoidance`

```rust
/// Apply ORCA-adjusted velocity to unit transforms directly.
/// Runs after `compute_avoidance` in `GameSet::Movement`.
pub fn apply_movement(
    time: Res<Time>,
    mut units: Query<(&AdjustedVelocity, &mut Transform), With<Unit>>,
) {
    let dt = time.delta_secs();
    for (velocity, mut transform) in &mut units {
        let delta = velocity.0 * dt;
        transform.translation.x += delta.x;
        transform.translation.y += delta.y;
    }
}
```

#### 4. Register `apply_movement` in system chain
**File**: `src/gameplay/units/mod.rs`
**Changes**: Add to the chained movement systems (lines 209-219)

```rust
app.add_systems(
    Update,
    (
        movement::unit_movement,
        avoidance::rebuild_spatial_hash,
        avoidance::compute_avoidance,
        avoidance::apply_movement,
    )
        .chain_ignore_deferred()
        .in_set(GameSet::Movement)
        .run_if(gameplay_running),
);
```

Also register `AdjustedVelocity` reflect type (after line 195):
```rust
.register_type::<AdjustedVelocity>()
```

#### 5. Switch to `RigidBody::Kinematic`, remove `LinearVelocity`
**File**: `src/gameplay/units/mod.rs`
**Changes**: In `spawn_unit` (lines 128-143)

- Line 134: `RigidBody::Dynamic` → `RigidBody::Kinematic`
- Line 139: Remove `LinearVelocity::ZERO`
- Add `AdjustedVelocity::default()` in its place

Also update imports at top of file:
- Remove `LinearVelocity` from the avian2d import (keep `RigidBody`, `Collider`, `LockedAxes`)
- Add `AdjustedVelocity` to the avoidance import

#### 6. Update test helper: `spawn_test_unit`
**File**: `src/testing.rs`
**Changes**: In `spawn_test_unit` (lines 154-191)

- Line 5: Remove `avian2d::prelude::*` import, add specific avian2d imports (`Collider`)
- Line 182: `LinearVelocity::ZERO` → `AdjustedVelocity::default()`
- Add `AdjustedVelocity` to the avoidance import (line 14)

#### 7. Update test helper: `spawn_avoidance_unit`
**File**: `src/gameplay/units/avoidance/mod.rs`
**Changes**: In `spawn_avoidance_unit` (lines 229-247)

- `LinearVelocity(current_vel)` → `AdjustedVelocity(current_vel)`

#### 8. Update avoidance test assertions
**File**: `src/gameplay/units/avoidance/mod.rs`
**Changes**: In all integration tests (lines 249-331)

- All `app.world().get::<LinearVelocity>(...)` → `app.world().get::<AdjustedVelocity>(...)`
- Assertions on `.0` field remain identical

#### 9. Update dev_tools debug visualization
**File**: `src/dev_tools/mod.rs`
**Changes**:

- Line 8: Remove `use avian2d::prelude::LinearVelocity;`
- Line 13: Add `AdjustedVelocity` to the avoidance import: `use crate::gameplay::units::avoidance::{AdjustedVelocity, PreferredVelocity};`
- Line 128: `&LinearVelocity` → `&AdjustedVelocity` in query
- Variable name in loop body stays `velocity`

#### 10. Clean up unused `LinearVelocity` imports
**Files**: Check and remove unused `LinearVelocity` imports from:
- `src/gameplay/mod.rs` — if it re-exports or imports `LinearVelocity`, remove
- Any other file that imported it only for unit usage

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (compilation + clippy)
- [ ] `make test` passes (all tests, including updated avoidance + movement tests)
- [ ] No unused import warnings for `LinearVelocity` in any file

#### Manual Verification:
- [ ] Units move correctly following flow field toward enemy fortress
- [ ] ORCA avoidance works — units steer around each other, no overlapping clumps
- [ ] Units engage and attack targets correctly (no movement regressions)
- [ ] F3 debug overlay shows green (preferred) and cyan (adjusted) velocity arrows
- [ ] No oscillation, no stuck units, no "explosion" effects on death transitions
- [ ] Performance is comparable to before (no visible frame drops)

**Implementation Note**: After completing all changes and automated verification passes, pause for manual testing confirmation before marking the ticket done.

---

## Testing Strategy

### Updated Tests (no new test files needed):

**`avoidance/mod.rs` integration tests** (4 tests):
- `lone_unit_keeps_preferred_velocity` — reads `AdjustedVelocity` instead of `LinearVelocity`
- `head_on_units_steer_apart` — same change
- `zero_preferred_stays_zero` — same change
- `distant_units_no_avoidance` — same change

**`movement.rs` tests** (6 tests):
- No changes needed — these only test `PreferredVelocity` output, not `LinearVelocity`

### New Test:

**`apply_movement` unit test** in `avoidance/mod.rs`:
```rust
#[test]
fn apply_movement_updates_transform() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_systems(Update, apply_movement);
    app.update(); // init time

    let unit = app.world_mut().spawn((
        Unit,
        AdjustedVelocity(Vec2::new(100.0, 50.0)),
        Transform::from_xyz(0.0, 0.0, 0.0),
    )).id();

    app.update();

    let transform = app.world().get::<Transform>(unit).unwrap();
    // Transform should have moved in the direction of AdjustedVelocity
    assert!(transform.translation.x > 0.0, "X should increase");
    assert!(transform.translation.y > 0.0, "Y should increase");
}
```

## Performance Considerations

- `apply_movement` is O(n) with minimal work per entity (one multiply + two additions) — negligible cost
- Removing physics integration for units reduces avian2d's workload (no more integrating velocity for n dynamic bodies)
- `RigidBody::Kinematic` still participates in collision detection but not the solver — reduced physics cost
- Net performance should be equal or slightly better

## References

- Linear ticket: [GAM-61](https://linear.app/tayhu-games/issue/GAM-61/decouple-orca-from-physics-direct-transform-movement)
- Research: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md` (Section 9, Ticket 4)
- Blocked by: [GAM-60](https://linear.app/tayhu-games/issue/GAM-60/flow-field-infrastructure-remove-navmesh) (Done)
- Blocks: [GAM-62](https://linear.app/tayhu-games/issue/GAM-62/remove-avian2d-physics-engine) (Remove avian2d)

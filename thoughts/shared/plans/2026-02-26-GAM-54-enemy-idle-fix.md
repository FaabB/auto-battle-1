# GAM-54: Enemy Units Idle Around Fortress Fix

## Overview

Enemy units spawn near the enemy fortress and idle in place instead of advancing toward player buildings/fortress. The root cause is in pathfinding, not targeting: the path destination (target's center position) is inside a NavObstacle-carved area of the navmesh, so `navmesh.path(from, to)` returns `None`.

## Current State Analysis

**Targeting works correctly**: `find_target` (`src/gameplay/ai.rs:51`) queries all `With<Target>` entities from the opposing team. Player fortresses and buildings have `Target` + `Team::Player`. Tests confirm enemy units target these entities.

**Pathfinding fails for NavObstacle targets**: `compute_paths` (`src/gameplay/units/pathfinding.rs:86`) computes the destination as the target entity's center position (`target_transform.translation().xy()`). Fortresses and buildings are `NavObstacle`s — their footprints are carved out of the navmesh. The `navmesh.path(from, to)` call returns `None` because the destination is inside the carved area. The built-in snap radius is only 0.2px (default `search_delta=0.1, search_steps=2`), far too small to bridge the ~70px gap to the mesh edge.

### Key Discoveries:
- Player fortress at `(64, 320)` has `NavObstacle` — carved area extends ~70px from center (64px half-width + 6px agent_radius)
- Enemy fortress at `(5184, 320)` same — spawned units must path around it
- Buildings also have `NavObstacle` (`placement.rs:146`)
- Units do NOT have `NavObstacle` — paths to unit targets work fine
- The built-in polyanya snap (`search_delta * search_steps = 0.2px`) is 350x too small
- `navmesh.is_in_mesh()` returns false for points inside carved areas

**Affected entities**: Any unit targeting a fortress or building (all `NavObstacle` entities). When no opposing units exist, ALL targets are NavObstacles, making the issue most visible.

## Desired End State

Enemy units always advance toward opposing targets, regardless of whether those targets are NavObstacle entities (buildings, fortresses). Units should:
1. Receive a valid target via `find_target` (already works)
2. Compute a navmesh path to a navigable point near the target (currently broken)
3. Follow the path toward the target and stop within attack range (already works once path exists)

### Verification:
- `make check` passes
- `make test` passes (including new tests)
- In-game: enemy units march toward player fortress when no player units exist
- In-game: units path correctly to buildings placed in the build zone

## What We're NOT Doing

- Changing the AI targeting logic — it's correct
- Modifying global navmesh search parameters (search_delta/search_steps) — too many side effects
- Adding direct-movement fallback — navmesh pathing is the right solution once destinations are on-mesh
- Fixing spawn placement — units spawn in navigable positions

## Implementation Approach

Add a `snap_to_mesh()` helper that walks from an off-mesh target position toward the unit until it finds a navigable point. Use this snapped position as the path destination in `compute_paths`.

---

## Phase 1: Add `snap_to_mesh` and Fix `compute_paths`

### Overview
Add the snap helper and modify path computation to handle off-mesh targets.

### Changes Required:

#### 1. Add snap constants and helper function
**File**: `src/gameplay/units/pathfinding.rs`
**Changes**: Add constants and `snap_to_mesh` function before `compute_paths`

```rust
/// Step size in pixels when searching for a navigable point near an off-mesh target.
const SNAP_STEP_SIZE: f32 = 8.0;

/// Maximum search distance = SNAP_STEP_SIZE * SNAP_MAX_STEPS = 160px.
/// Covers the largest obstacle (fortress: 64px half-width + 6px agent_radius = 70px).
const SNAP_MAX_STEPS: u32 = 20;

/// Find the nearest navigable point to `target` by walking toward `from`.
///
/// Returns `target` unchanged if it's already on the mesh.
/// When the target is inside a carved obstacle (e.g., a fortress or building with
/// `NavObstacle`), steps along the direction from `target` toward `from` until
/// an on-mesh point is found.
///
/// Returns `None` if no navigable point is found within the search distance.
fn snap_to_mesh(navmesh: &NavMesh, target: Vec2, from: Vec2) -> Option<Vec2> {
    if navmesh.is_in_mesh(target) {
        return Some(target);
    }

    let dir = (from - target).normalize_or_zero();
    if dir == Vec2::ZERO {
        return None;
    }

    #[allow(clippy::cast_precision_loss)] // step is at most 20
    for step in 1..=SNAP_MAX_STEPS {
        let candidate = target + dir * (SNAP_STEP_SIZE * step as f32);
        if navmesh.is_in_mesh(candidate) {
            return Some(candidate);
        }
    }

    None
}
```

#### 2. Modify `compute_paths` to use snap
**File**: `src/gameplay/units/pathfinding.rs`
**Changes**: Add snap call before `navmesh.path()`

Current code (lines 130-138):
```rust
let from = transform.translation().xy();
let to = target_transform.translation().xy();

if let Some(path) = navmesh.path(from, to) {
    nav_path.set(path.path, current_target.0);
} else {
    // No valid path — store empty waypoints, unit stops until next refresh
    nav_path.set(Vec::new(), current_target.0);
}
```

New code:
```rust
let from = transform.translation().xy();
let to = target_transform.translation().xy();

// Snap off-mesh destinations to nearest navigable point. Targets like
// fortresses and buildings are NavObstacles — their centers are carved
// out of the navmesh. Walking toward the unit finds the obstacle's
// nearest mesh edge on the correct approach side.
let destination = snap_to_mesh(navmesh, to, from).unwrap_or(to);

if let Some(path) = navmesh.path(from, destination) {
    nav_path.set(path.path, current_target.0);
} else {
    // No valid path — store empty waypoints, unit stops until next refresh
    nav_path.set(Vec::new(), current_target.0);
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (no clippy/compile errors)
- [x] `make test` passes (existing tests still pass)

#### Manual Verification:
- [x] Start a game, don't place buildings, wait for enemy wave — enemies march toward player fortress
- [x] Place a building, observe enemy units path toward it correctly
- [x] Enemy units stop at attack range and begin attacking

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Phase 2: Tests

### Overview
Add tests to prevent regression and verify the snap behavior.

### Changes Required:

#### 1. Unit tests for `snap_to_mesh`
**File**: `src/gameplay/units/pathfinding.rs` (in `#[cfg(test)] mod tests`)
**Changes**: Add tests using a manually built `NavMesh`

```rust
#[test]
fn snap_to_mesh_returns_target_when_on_mesh() {
    // Build a simple navmesh (single triangle covering a rectangular area)
    let navmesh = build_test_navmesh();
    let target = Vec2::new(100.0, 100.0); // clearly on mesh
    let from = Vec2::new(500.0, 100.0);

    let result = snap_to_mesh(&navmesh, target, from);
    assert_eq!(result, Some(target));
}

#[test]
fn snap_to_mesh_finds_nearest_navigable_point() {
    // Build navmesh with a carved-out center (simulating a NavObstacle)
    let navmesh = build_test_navmesh();
    let target = Vec2::new(-50.0, 100.0); // off-mesh (outside left boundary)
    let from = Vec2::new(500.0, 100.0); // unit to the right

    let result = snap_to_mesh(&navmesh, target, from);
    assert!(result.is_some(), "Should find an on-mesh point");
    let snapped = result.unwrap();
    assert!(navmesh.is_in_mesh(snapped), "Snapped point should be on mesh");
    // Snapped point should be between target and from
    assert!(snapped.x > target.x, "Snapped point should be to the right of target");
}

#[test]
fn snap_to_mesh_returns_none_when_unreachable() {
    let navmesh = build_test_navmesh();
    // Target very far from the mesh, beyond SNAP_MAX_STEPS * SNAP_STEP_SIZE
    let target = Vec2::new(-500.0, 100.0);
    let from = Vec2::new(-400.0, 100.0); // from is also off-mesh

    let result = snap_to_mesh(&navmesh, target, from);
    assert!(result.is_none());
}
```

The `build_test_navmesh()` helper creates a simple rectangular NavMesh using polyanya's triangulation API. The exact implementation will depend on the available construction API.

#### 2. Integration test for enemy pathing to fortress
**File**: `src/gameplay/units/pathfinding.rs` (or a new integration test section)

This test verifies the end-to-end flow: unit targeting a NavObstacle entity gets a non-empty `NavPath`.

Note: This may require setting up a full navmesh with obstacles, which is complex under `MinimalPlugins`. If navmesh setup is impractical in tests, this test can be deferred and covered by manual verification.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes (all new and existing tests pass)
- [x] New `snap_to_mesh` tests verify on-mesh, off-mesh, and unreachable cases

#### Manual Verification:
- [ ] Same manual checks as Phase 1

---

## Testing Strategy

### Unit Tests:
- `snap_to_mesh` with on-mesh target (returns unchanged)
- `snap_to_mesh` with off-mesh target (snaps to nearest navigable point toward unit)
- `snap_to_mesh` with unreachable target (returns `None`)
- `snap_to_mesh` with `from == target` (returns `None` — zero direction)

### Manual Testing Steps:
1. Start game, do NOT place buildings, wait for enemy wave → enemies march left toward player fortress
2. Place a barracks in the build zone → enemy units path to building and attack it
3. Observe that enemy units stop at attack range and deal damage
4. Verify player units still path correctly to enemy targets (no regression)

## Performance Considerations

`snap_to_mesh` only runs when `navmesh.is_in_mesh(target)` returns false — i.e., when the target is a NavObstacle. For most targets (opposing units), this check is the only overhead (returns `true` immediately). For obstacle targets, the walk search is O(20) `is_in_mesh` calls at worst — negligible compared to the navmesh path computation itself.

## References

- Linear ticket: [GAM-54](https://linear.app/tayhu-games/issue/GAM-54/enemy-units-idle-around-their-fortress-when-no-player-units-exist)
- AI targeting system: `src/gameplay/ai.rs:51` (`find_target`)
- Pathfinding system: `src/gameplay/units/pathfinding.rs:86` (`compute_paths`)
- NavObstacle entities: `src/gameplay/battlefield/renderer.rs:97,179` (fortresses), `src/gameplay/building/placement.rs:146` (buildings)
- vleue_navigator snap API: `search_delta=0.1, search_steps=2` → 0.2px snap radius (too small)
- polyanya `get_closest_point`: `polyanya-0.16.1/src/lib.rs:684` (available but not used — global config)

# Navmesh Navigation Fixes (GAM-36 + GAM-46) Implementation Plan

## Overview

Both GAM-36 (corner oscillation) and GAM-46 (steering through buildings) stem from the same architectural gap: the movement system still treats direct-to-target steering as a valid fallback, and uses a waypoint threshold that's too generous for corner geometry. These three changes fully commit to navmesh-based navigation.

## Current State Analysis

**Data flow**: `compute_paths` (GameSet::Ai) writes `NavPath` waypoints → `unit_movement` (GameSet::Movement) reads waypoints, writes `PreferredVelocity` → ORCA computes `LinearVelocity` → Avian2d physics resolves collisions.

**GAM-36 root cause**: `WAYPOINT_REACHED_DISTANCE = 8px` is too generous. At building corners, the unit advances the waypoint when it's still 8px away — before it has actually rounded the corner. The next segment then steers through the building corner, physics blocks it, and the unit oscillates. Decreasing to 4px forces the unit to be further along the corner before transitioning, so the next segment clears the building.

**GAM-46 root cause**: When `NavPath` has no waypoints (pathfinding failed or all consumed), `unit_movement` falls back to steering directly toward `target_xy` (line 75). This can point through buildings.

### Key Discoveries:
- `vleue_navigator` uses a true Minkowski sum with rounded corners (5 arc segments) — corner waypoints are at exactly `agent_radius = 6px` from building surface (`pathfinding.rs:125`, verified in polyanya source)
- ORCA handles unit-to-unit avoidance only; buildings are NOT ORCA obstacles (`avoidance/orca.rs:4`)
- Path refresh timer is 0.5s repeating (`pathfinding.rs:11`)
- `NavPath.set(Vec::new(), target)` stores the target with empty waypoints when pathfinding fails (`pathfinding.rs:129`)

## Desired End State

After implementation:
1. Units never steer directly toward their target — they always follow navmesh paths or stop
2. When all waypoints are consumed but the unit isn't in attack range, the path is recomputed next frame (not after 0.5s)
3. Units round building corners smoothly without oscillation — the decreased waypoint threshold ensures they're at the correct position before transitioning to the next path segment
4. Narrow-passage behavior is improved (less path deviation with a tighter threshold)

## What We're NOT Doing

- Changing `agent_radius` or navmesh construction settings
- Adding ORCA static obstacles for buildings
- Implementing path smoothing or pure pursuit
- Changing the 0.5s refresh interval for normal path refresh
- Refactoring the movement/pathfinding architecture

## Implementation Approach

Three targeted changes in two files. All changes are in the movement pipeline — no changes to navmesh construction, ORCA, or physics.

---

## Phase 1: Decrease Waypoint Threshold (GAM-36)

### Overview
Decrease `WAYPOINT_REACHED_DISTANCE` from 8px to 4px to prevent premature waypoint advancement at building corners.

### Changes Required:

#### 1. Decrease constant
**File**: `src/gameplay/units/movement.rs`
**Change**: Line 13

```rust
// Before:
const WAYPOINT_REACHED_DISTANCE: f32 = 8.0;

// After:
const WAYPOINT_REACHED_DISTANCE: f32 = 4.0;
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — existing test `unit_advances_to_next_waypoint` still passes (waypoint at (102, 100), unit at (100, 100), distance = 2px < 4px ✓)

#### Manual Verification:
- [ ] Place two adjacent barracks, spawn units — units round building corners without oscillation
- [ ] Units in narrow passages (between adjacent buildings) navigate smoothly

---

## Phase 2: Remove Direct-to-Target Fallback and Add Immediate Re-path (GAM-46)

### Overview
Stop the unit when no valid waypoints exist instead of steering through buildings. Trigger immediate path recomputation when all waypoints are consumed but the unit isn't at its target yet.

### Changes Required:

#### 1. Add `is_path_consumed()` method to NavPath
**File**: `src/gameplay/units/pathfinding.rs`
**Change**: Add method to `impl NavPath` (after `needs_recompute`, around line 74)

```rust
/// Whether a non-empty path has been fully consumed (all waypoints visited).
/// Used to trigger immediate re-pathing when the unit hasn't reached its target yet.
#[must_use]
pub fn is_path_consumed(&self) -> bool {
    !self.waypoints.is_empty() && self.current_index >= self.waypoints.len()
}
```

#### 2. Add immediate re-path condition to `compute_paths`
**File**: `src/gameplay/units/pathfinding.rs`
**Change**: Modify the skip condition at lines 107-110

```rust
// Before:
// Skip recomputation if target hasn't changed and no periodic refresh
if !target_changed && !refresh_due {
    continue;
}

// After:
// Recompute if: target changed, periodic refresh due, or path fully consumed
let path_consumed = nav_path.is_path_consumed();
if !target_changed && !refresh_due && !path_consumed {
    continue;
}
```

#### 3. Remove direct-to-target fallback in `unit_movement`
**File**: `src/gameplay/units/movement.rs`
**Change**: Replace lines 74-89

```rust
// Before:
// Determine steering target: next waypoint or direct to target
let steer_toward = nav_path.current_waypoint().map_or(target_xy, |waypoint| {
    // Check if we've reached the current waypoint
    let dist_to_waypoint = current_xy.distance(waypoint);
    if dist_to_waypoint < WAYPOINT_REACHED_DISTANCE {
        // Advance to next waypoint
        if nav_path.advance() {
            nav_path.current_waypoint().unwrap_or(target_xy)
        } else {
            // No more waypoints — steer directly to target
            target_xy
        }
    } else {
        waypoint
    }
});

// After:
// Determine steering target from navmesh waypoints — never steer direct to target
let Some(steer_toward) = nav_path.current_waypoint().map(|waypoint| {
    let dist_to_waypoint = current_xy.distance(waypoint);
    if dist_to_waypoint < WAYPOINT_REACHED_DISTANCE {
        if nav_path.advance() {
            nav_path.current_waypoint()
        } else {
            None // All waypoints consumed — stop, re-path next frame
        }
    } else {
        Some(waypoint)
    }
}).flatten() else {
    // No waypoints available — stop and wait for path computation
    preferred.0 = Vec2::ZERO;
    continue;
};
```

#### 4. Update comment on pathfinding failure
**File**: `src/gameplay/units/pathfinding.rs`
**Change**: Line 128

```rust
// Before:
// No valid path — clear waypoints, movement falls back to direct

// After:
// No valid path — store empty waypoints, unit stops until next refresh
```

#### 5. Update existing test
**File**: `src/gameplay/units/movement.rs`
**Change**: The test `unit_falls_back_to_direct_when_no_path` must be updated since we no longer fall back to direct steering.

```rust
// Before:
#[test]
fn unit_falls_back_to_direct_when_no_path() {
    let mut app = create_movement_test_app();
    let stats = unit_stats(UnitType::Soldier);

    let target = spawn_target_at(app.world_mut(), 500.0);
    let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));
    // NavPath is default (empty) — should go direct to target

    app.update();

    let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
    assert!(
        velocity.0.x > 0.0,
        "Unit with no path should move directly toward target, got vx={}",
        velocity.0.x
    );
}

// After:
#[test]
fn unit_stops_when_no_path() {
    let mut app = create_movement_test_app();
    let stats = unit_stats(UnitType::Soldier);

    let target = spawn_target_at(app.world_mut(), 500.0);
    let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));
    // NavPath is default (empty) — should stop, not steer direct

    app.update();

    let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
    assert!(
        velocity.0.length() < f32::EPSILON,
        "Unit with no path should stop, got {:?}",
        velocity.0
    );
}
```

#### 6. Add new tests
**File**: `src/gameplay/units/movement.rs`

```rust
#[test]
fn unit_stops_when_all_waypoints_consumed() {
    let mut app = create_movement_test_app();
    let stats = unit_stats(UnitType::Soldier);

    // Target far away (not in attack range)
    let target = spawn_target_at(app.world_mut(), 500.0);
    let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

    // Set a single waypoint very close to the unit so it's consumed immediately
    let mut nav_path = app.world_mut().get_mut::<NavPath>(unit).unwrap();
    nav_path.set(vec![Vec2::new(101.0, 100.0)], Some(target));

    app.update();

    // Waypoint consumed, but not in attack range — unit should stop
    let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
    assert!(
        velocity.0.length() < f32::EPSILON,
        "Unit should stop when all waypoints consumed, got {:?}",
        velocity.0
    );
}
```

**File**: `src/gameplay/units/pathfinding.rs`

```rust
#[test]
fn nav_path_is_path_consumed() {
    let mut path = NavPath::default();

    // Empty path is not "consumed" (was never set)
    assert!(!path.is_path_consumed());

    // Set a path with one waypoint
    path.set(vec![Vec2::new(1.0, 2.0)], None);
    assert!(!path.is_path_consumed()); // At index 0, not consumed

    // Advance past the last waypoint
    path.advance();
    assert!(path.is_path_consumed()); // All consumed

    // Clear resets
    path.clear();
    assert!(!path.is_path_consumed());
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes (all existing tests + new tests)
- [ ] New test `unit_stops_when_no_path` passes (replaces `unit_falls_back_to_direct_when_no_path`)
- [ ] New test `unit_stops_when_all_waypoints_consumed` passes
- [ ] New test `nav_path_is_path_consumed` passes

#### Manual Verification:
- [ ] Unit near a building with no path: unit stops (zero velocity), does not steer into the building
- [ ] Unit that consumed all waypoints: briefly stops, gets a new path next frame, continues
- [ ] Units still navigate to targets normally with navmesh paths
- [ ] No regression in combat — units still stop within attack range
- [ ] Wave enemies pathfind correctly from spawn to player buildings/fortress

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation that the manual testing was successful before marking complete.

---

## Testing Strategy

### Unit Tests:
- `nav_path_is_path_consumed` — verifies the new method on NavPath
- `unit_stops_when_no_path` — verifies no direct fallback when pathless
- `unit_stops_when_all_waypoints_consumed` — verifies stop behavior after consuming all waypoints
- Existing tests: `unit_follows_waypoint_instead_of_direct`, `unit_advances_to_next_waypoint`, `unit_stops_at_range_even_with_remaining_waypoints` still pass

### Manual Testing Steps:
1. Place two adjacent barracks and watch units round building corners — should be smooth, no oscillation
2. Place barracks with narrow gaps — units should navigate through cleanly
3. Observe units with no navmesh path (e.g., target behind an impassable obstacle) — should stop, not push into buildings
4. Watch wave enemies navigate from spawn to player buildings — should path normally

## References

- Linear ticket: [GAM-36](https://linear.app/tayhu-games/issue/GAM-36/units-oscillate-against-building-corners-when-following-navmesh-paths)
- Linear ticket: [GAM-46](https://linear.app/tayhu-games/issue/GAM-46/units-steer-into-buildings-when-navpath-is-empty)
- Movement system: `src/gameplay/units/movement.rs:29-102`
- Pathfinding system: `src/gameplay/units/pathfinding.rs:79-132`
- NavPath component: `src/gameplay/units/pathfinding.rs:31-75`
- Navmesh settings: `src/gameplay/battlefield/renderer.rs:209-226`

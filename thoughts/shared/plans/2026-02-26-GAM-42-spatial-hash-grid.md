# Spatial Hash Grid for find_target (GAM-42)

## Overview

Replace the O(n*m) brute-force nearest-target search in `find_target` with a spatial hash grid, reducing targeting to O(n*k) where k is the small number of entities in nearby cells. This is the #1 performance bottleneck at scale — at 4k entities the inner loop does up to 16M `surface_distance` (GJK) calls per frame.

The codebase already has a `SpatialHash` struct in the ORCA avoidance module (`gameplay/units/avoidance/spatial_hash.rs`). We extract it to a shared location and create a second instance for targeting.

## Current State Analysis

### The bottleneck (`ai.rs:51-113`)

`find_target` iterates ALL seekers × ALL targets. For each pair it calls `surface_distance` (GJK shape query via avian2d). The retarget throttle (10 slots, 15ms each) helps for units *with* a valid target, but units without a target skip the throttle and run the full O(m) inner scan every frame.

### Existing spatial hash (`avoidance/spatial_hash.rs`)

A `HashMap<(i32,i32), Vec<Entity>>` with `insert`, `query_neighbors(pos, radius)`, and `clear`. Currently:
- Derives `Resource` directly (only one instance allowed)
- Cell size: 150px (ORCA neighbor distance)
- Populated with `With<Unit>` entities only
- Used by `rebuild_spatial_hash` → `compute_avoidance` chain in `GameSet::Movement`

### Entity dimensions

| Entity | Collider | Half-extent |
|--------|----------|-------------|
| Soldier unit | `circle(6.0)` | 6px |
| Building | `rectangle(40.0, 40.0)` | 20px |
| Fortress | `rectangle(128.0, 128.0)` | 64px |

### Key Discoveries:
- `SpatialHash` is domain-agnostic — stores `(Entity, Vec2)`, no game logic (`avoidance/spatial_hash.rs:7-56`)
- `find_target` finds the globally nearest target regardless of attack range (`ai.rs:87-108`)
- Battlefield is 82×10 cells = 5248×640px (`battlefield/mod.rs:34-42`)
- CELL_SIZE = 64px — good spatial hash cell size (units fit in 1 cell, buildings in 1, fortresses span 2×2)

## Desired End State

- `SpatialHash` lives in `gameplay/spatial_hash.rs` as a shared, non-Resource struct
- Two resource newtypes: `AvoidanceSpatialHash` (existing usage) and `TargetSpatialHash` (new)
- `find_target` queries the target grid instead of iterating all targets
- Center-distance pre-filter skips expensive GJK calls for obviously-distant candidates
- All existing tests pass unchanged
- Frame time at 4k units dramatically improved

### How to verify:
- `make check` and `make test` pass
- Run the game with `dev` feature, spawn 4k+ units, observe no frame rate collapse during targeting
- All 8 existing `ai.rs` tests pass unchanged

## What We're NOT Doing

- **Not adding a third-party crate** — hand-rolled is equivalent to `bevy_sparse_grid_2d` and avoids the dependency
- **Not changing the retarget throttle** — the stagger slot system is orthogonal and works well
- **Not optimizing `surface_distance` itself** — the GJK call is fine, we just call it fewer times
- **Not using avian2d `SpatialQuery`** — it's designed for physics, not game-logic nearest-neighbor
- **Not switching to a flat array** — HashMap is fast enough and handles sparse entity distributions

## Implementation Approach

Extract the existing `SpatialHash` to a shared module, create newtype resource wrappers for each consumer (avoidance and targeting), then modify `find_target` to query the target grid with a two-pass strategy: small radius first (catches 90%+ of cases), full battlefield fallback for seekers with no nearby targets.

---

## Phase 1: Extract SpatialHash to Shared Module

### Overview
Move `SpatialHash` from the avoidance submodule to `gameplay/spatial_hash.rs` so both the avoidance and AI modules can use it. Replace the direct `Resource` derive with newtype wrappers in each consumer.

### Changes Required:

#### 1. Create shared module
**File**: `src/gameplay/spatial_hash.rs` (new)
**Changes**: Move the `SpatialHash` struct and its unit tests here. Remove `#[derive(Resource)]` — consumers will wrap it in their own resource newtypes.

```rust
//! Uniform-grid spatial hash for fast neighbor queries.

use bevy::prelude::*;
use std::collections::HashMap;

/// Spatial hash for O(1) neighbor lookups.
///
/// Buckets entities into grid cells by position. Designed to be rebuilt
/// every frame (call `clear()` then `insert()` for each entity).
/// Consumers wrap this in a newtype `Resource` (e.g., `TargetSpatialHash`).
#[derive(Debug)]
pub struct SpatialHash {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<Entity>>,
}

// ... same impl block as current, unchanged ...

#[cfg(test)]
mod tests {
    // ... same tests as current, unchanged ...
}
```

#### 2. Register the shared module
**File**: `src/gameplay/mod.rs`
**Changes**: Add `pub mod spatial_hash;` to the module declarations (after `pub mod ai;`).

#### 3. Update avoidance module — newtype resource
**File**: `src/gameplay/units/avoidance/mod.rs`
**Changes**:
- Remove `pub mod spatial_hash;` declaration
- Change import from `self::spatial_hash::SpatialHash` to `crate::gameplay::spatial_hash::SpatialHash`
- Add `AvoidanceSpatialHash` newtype resource with `Deref`/`DerefMut`:

```rust
/// Spatial hash for ORCA avoidance neighbor lookups.
/// Populated with `With<Unit>` entities each frame.
#[derive(Resource, Debug)]
pub struct AvoidanceSpatialHash(pub SpatialHash);

impl std::ops::Deref for AvoidanceSpatialHash {
    type Target = SpatialHash;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl std::ops::DerefMut for AvoidanceSpatialHash {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
```

- Update `rebuild_spatial_hash`: `ResMut<SpatialHash>` → `ResMut<AvoidanceSpatialHash>`
- Update `compute_avoidance`: `Res<SpatialHash>` → `Res<AvoidanceSpatialHash>`
- (Systems call `hash.clear()`, `hash.insert()`, `hash.query_neighbors()` — unchanged thanks to Deref)

#### 4. Delete old file
**File**: `src/gameplay/units/avoidance/spatial_hash.rs`
**Changes**: Delete this file (moved to `gameplay/spatial_hash.rs`).

#### 5. Update units plugin resource registration
**File**: `src/gameplay/units/mod.rs`
**Changes**:
- Change import from `self::avoidance::spatial_hash::SpatialHash` to `crate::gameplay::spatial_hash::SpatialHash`
- Change `app.insert_resource(SpatialHash::new(...))` to `app.insert_resource(AvoidanceSpatialHash(SpatialHash::new(...)))`
- Update the `AvoidanceSpatialHash` import from `self::avoidance::AvoidanceSpatialHash`

#### 6. Update avoidance integration tests
**File**: `src/gameplay/units/avoidance/mod.rs` (test section)
**Changes**: Update `create_avoidance_test_app()` to use `AvoidanceSpatialHash` instead of bare `SpatialHash`.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (no compile errors, no clippy warnings)
- [ ] `make test` passes (all existing tests, including avoidance tests)

#### Manual Verification:
- [ ] Game runs normally — ORCA avoidance behavior unchanged
- [ ] Units still avoid each other when moving

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Phase 2: Add TargetSpatialHash and Rebuild System

### Overview
Create the target spatial hash resource and a system that rebuilds it each frame with all `Target` entities. Wire it before `find_target` in `GameSet::Ai`. No change to `find_target` yet — it still uses the brute-force scan.

### Changes Required:

#### 1. Add TargetSpatialHash and rebuild system
**File**: `src/gameplay/ai.rs`
**Changes**:

Add imports:
```rust
use super::spatial_hash::SpatialHash;
use super::battlefield::CELL_SIZE;
```

Add newtype resource:
```rust
/// Spatial hash for target lookups. Populated with all `With<Target>` entities
/// each frame. Queried by `find_target` to find nearby candidates.
#[derive(Resource, Debug)]
pub struct TargetSpatialHash(SpatialHash);

impl std::ops::Deref for TargetSpatialHash {
    type Target = SpatialHash;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl std::ops::DerefMut for TargetSpatialHash {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}
```

Add rebuild system:
```rust
/// Rebuild the target spatial hash with all targetable entities.
/// Runs every frame before `find_target`.
fn rebuild_target_grid(
    mut grid: ResMut<TargetSpatialHash>,
    targets: Query<(Entity, &GlobalTransform), With<Target>>,
) {
    grid.clear();
    for (entity, transform) in &targets {
        grid.insert(entity, transform.translation().xy());
    }
}
```

#### 2. Register resource and wire system
**File**: `src/gameplay/ai.rs` (plugin function)
**Changes**:

```rust
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<RetargetTimer>();
    app.insert_resource(TargetSpatialHash(SpatialHash::new(CELL_SIZE)));
    app.register_type::<RetargetTimer>();
    app.add_systems(OnEnter(GameState::InGame), reset_retarget_timer);
    app.add_systems(
        Update,
        (rebuild_target_grid, find_target)
            .chain_ignore_deferred()
            .in_set(GameSet::Ai)
            .run_if(gameplay_running),
    );
}
```

Note: `chain_ignore_deferred()` ensures `rebuild_target_grid` runs before `find_target` without the overhead of `ApplyDeferred` between them (no commands are issued in `rebuild_target_grid`).

#### 3. Update test helper
**File**: `src/gameplay/ai.rs` (test section)
**Changes**: Update `create_ai_test_app()` to init the new resource and chain the rebuild system:

```rust
fn create_ai_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<RetargetTimer>();
    app.insert_resource(TargetSpatialHash(SpatialHash::new(
        crate::gameplay::battlefield::CELL_SIZE,
    )));
    app.add_systems(
        Update,
        (rebuild_target_grid, find_target).chain_ignore_deferred(),
    );
    app
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes (all existing tests — find_target still uses brute force, grid is just built alongside)

#### Manual Verification:
- [ ] Game runs normally — targeting behavior unchanged

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Phase 3: Modify find_target to Use Spatial Grid

### Overview
Replace the brute-force `all_targets` iteration with a two-pass spatial grid query: small radius first (catches nearby targets), full battlefield fallback if nothing found. Add center-distance pre-filter to skip expensive GJK calls.

### Changes Required:

#### 1. Add constants
**File**: `src/gameplay/ai.rs`
**Changes**: Add constants for the search strategy:

```rust
/// Initial search radius for nearby targets. 8 cells = 512px.
/// Covers most practical targeting scenarios (units near enemies).
const INITIAL_SEARCH_RADIUS: f32 = 8.0 * super::battlefield::CELL_SIZE;

/// Maximum half-extent of any entity collider (fortress = 128px, half = 64px).
/// Entities whose center is just outside the search radius may still have
/// their surface within range, so we pad the query by this amount.
const MAX_ENTITY_HALF_EXTENT: f32 = 64.0;

/// Diagonal of the full battlefield — used as fallback search radius.
/// Guarantees finding all targets regardless of position.
const BATTLEFIELD_DIAGONAL: f32 = 5300.0; // > sqrt(5248^2 + 640^2) ≈ 5287
```

#### 2. Rewrite find_target inner loop
**File**: `src/gameplay/ai.rs`
**Changes**: Replace the inner `for (candidate, ...) in &all_targets` loop with a grid-based search.

The new `find_target` signature adds `Res<TargetSpatialHash>` and keeps `all_targets` for component lookups:

```rust
pub fn find_target(
    time: Res<Time>,
    mut retarget_timer: ResMut<RetargetTimer>,
    grid: Res<TargetSpatialHash>,
    mut seekers: Query<(
        Entity,
        &Team,
        &GlobalTransform,
        &Collider,
        &mut CurrentTarget,
        Option<&Movement>,
    )>,
    all_targets: Query<(Entity, &Team, &GlobalTransform, &Collider), With<Target>>,
) {
    // Retarget timer logic unchanged (lines 64-81)
    retarget_timer.timer.tick(time.delta());
    let slot_advanced = retarget_timer.timer.just_finished();
    if slot_advanced {
        retarget_timer.current_slot = (retarget_timer.current_slot + 1) % RETARGET_SLOTS;
    }

    for (entity, team, transform, seeker_collider, mut current_target, movement) in &mut seekers {
        let has_valid_target = current_target.0.is_some_and(|e| all_targets.get(e).is_ok());

        if has_valid_target {
            if !slot_advanced {
                continue;
            }
            let entity_slot = entity.index().index() % RETARGET_SLOTS;
            if entity_slot != retarget_timer.current_slot {
                continue;
            }
        }

        let my_pos = transform.translation().xy();
        let opposing_team = team.opposing();

        // Two-pass spatial search: nearby first, full battlefield fallback
        let nearest = find_nearest_target(
            &grid,
            entity,
            my_pos,
            seeker_collider,
            &opposing_team,
            movement.is_some(),
            team,
            &all_targets,
        );

        current_target.0 = nearest;
    }
}
```

#### 3. Add find_nearest_target helper
**File**: `src/gameplay/ai.rs`
**Changes**: Extract the inner search logic into a helper function for clarity:

```rust
/// Search the spatial grid for the nearest valid target.
///
/// Two-pass strategy:
/// 1. Search within `INITIAL_SEARCH_RADIUS` (catches most cases)
/// 2. If nothing found, search the full battlefield
///
/// Within each pass, uses center-distance as a cheap pre-filter before
/// calling `surface_distance` (GJK) on close candidates.
#[allow(clippy::too_many_arguments)]
fn find_nearest_target(
    grid: &TargetSpatialHash,
    seeker_entity: Entity,
    seeker_pos: Vec2,
    seeker_collider: &Collider,
    opposing_team: &Team,
    is_mobile: bool,
    seeker_team: &Team,
    all_targets: &Query<(Entity, &Team, &GlobalTransform, &Collider), With<Target>>,
) -> Option<Entity> {
    // First pass: nearby targets
    let result = search_radius(
        grid,
        INITIAL_SEARCH_RADIUS + MAX_ENTITY_HALF_EXTENT,
        seeker_entity,
        seeker_pos,
        seeker_collider,
        opposing_team,
        is_mobile,
        seeker_team,
        all_targets,
    );

    if result.is_some() {
        return result;
    }

    // Fallback: full battlefield
    search_radius(
        grid,
        BATTLEFIELD_DIAGONAL,
        seeker_entity,
        seeker_pos,
        seeker_collider,
        opposing_team,
        is_mobile,
        seeker_team,
        all_targets,
    )
}

#[allow(clippy::too_many_arguments)]
fn search_radius(
    grid: &TargetSpatialHash,
    radius: f32,
    seeker_entity: Entity,
    seeker_pos: Vec2,
    seeker_collider: &Collider,
    opposing_team: &Team,
    is_mobile: bool,
    seeker_team: &Team,
    all_targets: &Query<(Entity, &Team, &GlobalTransform, &Collider), With<Target>>,
) -> Option<Entity> {
    let candidates = grid.query_neighbors(seeker_pos, radius);

    // Phase 1: Filter and compute center distances (cheap)
    let mut valid_candidates: Vec<(Entity, Vec2, &Collider, f32)> = Vec::new();
    for candidate_entity in candidates {
        let Ok((cand_entity, cand_team, cand_transform, cand_collider)) =
            all_targets.get(candidate_entity)
        else {
            continue;
        };

        if cand_entity == seeker_entity || *cand_team != *opposing_team {
            continue;
        }

        let cand_pos = cand_transform.translation().xy();

        // Backtrack filter (mobile entities only)
        if is_mobile {
            let behind = match seeker_team {
                Team::Player => seeker_pos.x - cand_pos.x,
                Team::Enemy => cand_pos.x - seeker_pos.x,
            };
            if behind > BACKTRACK_DISTANCE {
                continue;
            }
        }

        let center_dist = seeker_pos.distance(cand_pos);
        valid_candidates.push((cand_entity, cand_pos, cand_collider, center_dist));
    }

    if valid_candidates.is_empty() {
        return None;
    }

    // Phase 2: Find nearest by surface distance
    // Use center-distance to skip GJK for obviously-distant candidates.
    // Sort is not needed — just track the minimum.
    let min_center_dist = valid_candidates
        .iter()
        .map(|(_, _, _, d)| *d)
        .fold(f32::MAX, f32::min);

    // Only compute surface_distance for candidates whose center is close
    // enough that they could beat the current best surface distance.
    // Cutoff: min_center_dist + 2 * MAX_ENTITY_HALF_EXTENT covers the
    // worst case where both entities have maximum collider extent.
    let center_cutoff = min_center_dist + 2.0 * MAX_ENTITY_HALF_EXTENT;

    let mut nearest: Option<(Entity, f32)> = None;
    for (cand_entity, cand_pos, cand_collider, center_dist) in &valid_candidates {
        if *center_dist > center_cutoff {
            if let Some((_, best_surf)) = nearest {
                // Tighten cutoff as we find better candidates
                if *center_dist > best_surf + 2.0 * MAX_ENTITY_HALF_EXTENT {
                    continue;
                }
            } else {
                continue;
            }
        }

        let surf_dist =
            surface_distance(seeker_collider, seeker_pos, cand_collider, *cand_pos);
        if nearest.is_none_or(|(_, d)| surf_dist < d) {
            nearest = Some((*cand_entity, surf_dist));
        }
    }

    nearest.map(|(e, _)| e)
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — **all 8 existing ai.rs tests pass unchanged**

#### Manual Verification:
- [ ] Game runs normally — units still target the nearest enemy
- [ ] Fortresses still target the nearest enemy
- [ ] Backtrack limit still works (units don't chase far behind)
- [ ] Retarget throttle still works (units switch to closer targets over time)
- [ ] At 4k units, no visible frame time spike from targeting

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Phase 4: New Tests

### Overview
Add tests that exercise the spatial grid integration — expanding search, cross-boundary entities, and empty grid scenarios.

### Changes Required:

#### 1. Add spatial targeting tests
**File**: `src/gameplay/ai.rs` (test section)
**Changes**: Add new tests:

```rust
#[test]
fn targets_enemy_across_large_distance() {
    // Tests the fallback search (enemy far away, beyond initial radius)
    let mut app = create_ai_test_app();
    let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
    let far_enemy = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 4000.0, 100.0);
    app.update();
    let ct = app.world().get::<CurrentTarget>(player).unwrap();
    assert_eq!(ct.0, Some(far_enemy));
}

#[test]
fn prefers_nearby_over_distant() {
    // Nearby enemy should be chosen even with a distant enemy in the grid
    let mut app = create_ai_test_app();
    let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
    let _far = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 3000.0, 100.0);
    let near = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 200.0, 100.0);
    app.update();
    let ct = app.world().get::<CurrentTarget>(player).unwrap();
    assert_eq!(ct.0, Some(near));
}

#[test]
fn no_targets_gives_none() {
    // Seeker with no enemies at all
    let mut app = create_ai_test_app();
    let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
    // Only spawn friendly targets
    let _friendly = crate::testing::spawn_test_target(app.world_mut(), Team::Player, 200.0, 100.0);
    app.update();
    let ct = app.world().get::<CurrentTarget>(player).unwrap();
    assert_eq!(ct.0, None);
}
```

#### 2. Add spatial_hash module-level tests
**File**: `src/gameplay/spatial_hash.rs`
**Changes**: The existing tests from `avoidance/spatial_hash.rs` are already moved here in Phase 1. Verify they all pass.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — all existing + new tests green
- [ ] New tests specifically exercise: far-distance targeting, nearby preference, empty grid

---

## Testing Strategy

### Unit Tests:
- `SpatialHash` struct tests (moved from avoidance, unchanged)
- `TargetSpatialHash` rebuild correctness (all Target entities indexed)

### Integration Tests:
- All 8 existing `ai.rs` tests pass unchanged (behavior-preserving refactor)
- New tests for edge cases: large distance, nearby preference, no targets

### Manual Testing Steps:
1. Run the game with `cargo run`
2. Place barracks, let units spawn and engage enemies
3. Verify units target the nearest enemy and switch targets when closer ones spawn
4. Verify fortresses target enemies approaching them
5. Observe for any targeting anomalies (units ignoring nearby enemies, targeting through backtrack limit)

## Performance Considerations

- **Rebuild cost**: O(n) per frame to insert all Target entities into the grid. With 2000 targets, this is ~2000 HashMap inserts — trivial.
- **Query cost per seeker**: O(cells_checked + candidates_in_cells) instead of O(all_targets). With 64px cells and 512px search radius, checks ~170 cells, finding ~50 candidates typically. `surface_distance` called on ~5-10 candidates (center-distance pre-filter).
- **Worst case** (fallback to full scan): equivalent to the old O(m) brute force, but only for seekers with no nearby targets — rare during normal gameplay.
- **Memory**: HashMap grows to ~820 entries (82×10 cells). `Vec<Entity>` per cell averages ~5 entries. Total ~10KB.

## References

- Linear ticket: [GAM-42](https://linear.app/tayhu-games/issue/GAM-42/spatial-hash-grid-for-find-target-on²-on)
- Existing spatial hash: `src/gameplay/units/avoidance/spatial_hash.rs`
- Target system: `src/gameplay/ai.rs:51-113`
- `surface_distance` wrapper: `src/third_party/avian.rs:50-52`
- Blocked by this: GAM-45 (Profile and optimize remaining O(n) hotspots)

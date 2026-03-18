# Fix Flow Field Destroyed When Building Is Removed (GAM-66)

## Overview

When an enemy unit destroys a building, the flow field becomes entirely `Vec2::ZERO` and all enemy units stop moving permanently. Two bugs:

1. **Asymmetry in fortress blocking**: `setup_flow_fields` leaves fortresses unblocked, but `on_building_removed` rebuilds the cost grid with fortresses marked via `mark_building` (which adds inflation). The inflated ring blocks Dijkstra propagation from goal cells.
2. **Fortresses should be blocked consistently**: Fortresses are physical objects — units not targeting them should path around them. But they must be blocked **without inflation** since their cells are also Dijkstra goal cells.

## Current State Analysis

### The Bug

Three code paths modify the cost grid:

| Path | Fortresses blocked? | File:Line |
|---|---|---|
| `setup_flow_fields` (initial) | No — empty grid | `flow_field.rs:520` |
| `mark_building_placed` (placement) | No — only marks new building | `flow_field.rs:566` |
| `on_building_removed` (removal) | **Yes, with inflation — BUG** | `flow_field.rs:641-647` |

The observer at `flow_field.rs:614` queries both remaining buildings AND fortresses, then passes all AABBs to `rebuild_cost_grid_from_buildings`, which calls `mark_building` on each. `mark_building` inflates by `UNIT_RADIUS` (6px) creating a `COST_BLOCKED` ring around fortress cells. Since fortress cells are the Dijkstra goal cells (integration cost 0), blocking their neighbors prevents Dijkstra propagation entirely.

### Key Discoveries

- Fortresses do NOT have the `Building` component (`battlefield/renderer.rs:62-92, 139-169`) — the observer cannot fire for fortress death
- Goal cells are computed once in `setup_flow_fields` and stored permanently in `GoalRegistry.{player,enemy}_fortress.goal_cells`
- `strip_buildings_before_despawn` (`building/mod.rs:184-191`) also triggers this observer on `OnExit(GameState::InGame)`, but the flow field resource is cleaned up by `DespawnOnExit` anyway
- Dijkstra seeds goal cells at integration cost 0 **regardless of cost grid value** — so marking goal cells as `COST_BLOCKED` is fine as long as their neighbors aren't also blocked

### Why Blocking Without Inflation Works

Dijkstra flow field computation (`flow_field.rs:294-415`):
1. Goal cells are seeded at integration cost 0, pushed to the heap
2. When expanding FROM a goal cell, each neighbor's cost is checked: `cost_grid.costs[n_idx]`
3. If the neighbor is NOT blocked (outside the fortress footprint), it gets an integration cost and the wave propagates normally
4. Only the inflation ring (cells adjacent to but outside the fortress) would block propagation — without inflation, these cells stay `COST_OPEN`

Result: fortress cells are blocked (units path around), but Dijkstra propagates outward from goal cells through their non-blocked neighbors.

## Desired End State

- Fortress cells are consistently blocked in the cost grid (without inflation) across all code paths
- After a building is destroyed, the cost grid is rebuilt with buildings (inflated) + fortress goal cells (exact footprint, no inflation)
- Flow field recomputes correctly — units continue moving
- Units not targeting a fortress path around it

### Verification

- Automated: new regression test proves flow field has non-zero directions after building removal with fortress blocking
- Manual: destroy a building in-game, confirm enemy units keep moving

## What We're NOT Doing

- Not using `mark_building` for fortresses (inflation breaks Dijkstra)
- Not refactoring the cost grid rebuild to be incremental (full rebuild is correct for removal)
- Not adding a `COST_ADJACENT` ring around fortresses (would slow units approaching their goal fortress)

## Implementation Approach

Single-phase fix with three changes:
1. Mark fortress goal cells as `COST_BLOCKED` in `setup_flow_fields` before computing flow fields
2. In `on_building_removed`, remove the fortress query but re-mark stored goal cells as `COST_BLOCKED` after the rebuild clears the grid
3. Add regression tests

## Phase 1: Consistent Fortress Blocking + Fix

### Changes Required

#### 1. Mark Fortress Cells in `setup_flow_fields`
**File**: `src/gameplay/flow_field.rs`
**Changes**: After computing goal cells and before computing flow fields, mark fortress goal cells as `COST_BLOCKED` in the cost grid.

```rust
fn setup_flow_fields(
    mut commands: Commands,
    player_fort: Single<&Transform, With<crate::gameplay::battlefield::PlayerFortress>>,
    enemy_fort: Single<&Transform, With<crate::gameplay::battlefield::EnemyFortress>>,
) {
    let mut cost_grid = CostGrid::new(FLOW_COLS, FLOW_ROWS);
    let algorithm = Box::new(DijkstraFlowField);

    let pf_cells = fortress_cells_from_world(player_fort.translation.xy());
    let ef_cells = fortress_cells_from_world(enemy_fort.translation.xy());

    // Mark fortress footprints as blocked (no inflation — they're Dijkstra goals,
    // so propagation works outward through unblocked neighbors)
    for &(col, row) in &pf_cells {
        cost_grid.set(col, row, COST_BLOCKED);
    }
    for &(col, row) in &ef_cells {
        cost_grid.set(col, row, COST_BLOCKED);
    }

    let player_ff = algorithm.compute(&cost_grid, &pf_cells);
    let enemy_ff = algorithm.compute(&cost_grid, &ef_cells);

    commands.insert_resource(GoalRegistry {
        player_fortress: GoalFlowField {
            flow_field: player_ff,
            goal_cells: pf_cells,
        },
        enemy_fortress: GoalFlowField {
            flow_field: enemy_ff,
            goal_cells: ef_cells,
        },
        cost_grid,
        algorithm,
    });
    commands.insert_resource(FlowFieldDirty(false));
}
```

#### 2. Fix `on_building_removed` Observer
**File**: `src/gameplay/flow_field.rs`
**Changes**: Remove the fortress query. After rebuilding from buildings, re-mark fortress goal cells using stored goal cells from the registry.

```rust
fn on_building_removed(
    _trigger: On<Remove, crate::gameplay::building::Building>,
    mut registry: Option<ResMut<GoalRegistry>>,
    mut dirty: Option<ResMut<FlowFieldDirty>>,
    buildings: Query<(&Transform, &EntityExtent), With<crate::gameplay::building::Building>>,
) {
    let (Some(registry), Some(dirty)) = (registry.as_deref_mut(), dirty.as_deref_mut()) else {
        return;
    };

    let mut aabbs: Vec<(Vec2, Vec2)> = Vec::new();

    // Collect remaining buildings
    for (transform, extent) in &buildings {
        let pos = transform.translation.xy();
        if let EntityExtent::Rect(hw, hh) = extent {
            aabbs.push((pos - Vec2::new(*hw, *hh), pos + Vec2::new(*hw, *hh)));
        }
    }

    rebuild_cost_grid_from_buildings(registry, dirty, &aabbs);

    // Re-mark fortress goal cells as blocked (cleared during rebuild).
    // No inflation — fortress cells are Dijkstra goals, propagation works
    // outward through their unblocked neighbors.
    for &(col, row) in &registry.player_fortress.goal_cells {
        registry.cost_grid.set(col, row, COST_BLOCKED);
    }
    for &(col, row) in &registry.enemy_fortress.goal_cells {
        registry.cost_grid.set(col, row, COST_BLOCKED);
    }
}
```

#### 3. Add Regression Tests
**File**: `src/gameplay/flow_field.rs` (in `mod tests`)

**Test 1**: Verify flow field works with fortress cells blocked (no inflation).

```rust
#[test]
fn dijkstra_propagates_from_blocked_goal_cells() {
    // Simulate fortress: goal cells are COST_BLOCKED but seeded at integration 0
    let mut cost_grid = CostGrid::new(10, 10);
    // Block a 2x2 goal region
    for row in 0..2u32 {
        for col in 0..2u32 {
            cost_grid.set(col, row, COST_BLOCKED);
        }
    }

    let goals: Vec<(u32, u32)> = vec![(0, 0), (0, 1), (1, 0), (1, 1)];
    let algo = DijkstraFlowField;
    let flow = algo.compute(&cost_grid, &goals);

    // Cells adjacent to the blocked goals should have directions (propagation works)
    let dir = flow.directions[flow.index(2, 0)];
    assert_ne!(dir, Vec2::ZERO, "cell adjacent to blocked goal should be reachable");

    // Far corner should be reachable
    let dir = flow.directions[flow.index(9, 9)];
    assert_ne!(dir, Vec2::ZERO, "far corner should be reachable");
}
```

**Test 2**: Simulate the full bug scenario — place building, remove it, rebuild with fortress blocking, verify flow field.

```rust
#[test]
fn rebuild_with_fortress_blocking_preserves_flow_field() {
    let mut cost_grid = CostGrid::new(FLOW_COLS, FLOW_ROWS);
    let algo = Box::new(DijkstraFlowField);
    let pf_cells = fortress_cells_from_world(Vec2::new(64.0, 320.0));
    let ef_cells = fortress_cells_from_world(Vec2::new(5184.0, 320.0));

    // Mark fortress cells as blocked (matching new setup_flow_fields behavior)
    for &(col, row) in &pf_cells {
        cost_grid.set(col, row, COST_BLOCKED);
    }
    for &(col, row) in &ef_cells {
        cost_grid.set(col, row, COST_BLOCKED);
    }

    let player_ff = algo.compute(&cost_grid, &pf_cells);
    let enemy_ff = algo.compute(&cost_grid, &ef_cells);

    let mut registry = GoalRegistry {
        player_fortress: GoalFlowField {
            flow_field: player_ff,
            goal_cells: pf_cells.clone(),
        },
        enemy_fortress: GoalFlowField {
            flow_field: enemy_ff,
            goal_cells: ef_cells.clone(),
        },
        cost_grid,
        algorithm: algo,
    };
    let mut dirty = FlowFieldDirty(false);

    // Place a building
    mark_building_placed(
        &mut registry,
        &mut dirty,
        Vec2::new(200.0, 200.0),
        Vec2::new(240.0, 240.0),
    );

    // Remove the building (rebuild with empty list)
    rebuild_cost_grid_from_buildings(&mut registry, &mut dirty, &[]);

    // Re-mark fortress cells (matching new on_building_removed behavior)
    for &(col, row) in &registry.player_fortress.goal_cells {
        registry.cost_grid.set(col, row, COST_BLOCKED);
    }
    for &(col, row) in &registry.enemy_fortress.goal_cells {
        registry.cost_grid.set(col, row, COST_BLOCKED);
    }

    // Recompute flow fields
    registry.player_fortress.flow_field =
        registry.algorithm.compute(&registry.cost_grid, &pf_cells);
    registry.enemy_fortress.flow_field =
        registry.algorithm.compute(&registry.cost_grid, &ef_cells);

    // Flow fields should have directions (not all zero)
    let non_zero: usize = registry
        .enemy_fortress
        .flow_field
        .directions
        .iter()
        .filter(|d| **d != Vec2::ZERO)
        .count();
    let total = (FLOW_COLS * FLOW_ROWS) as usize;
    assert!(
        non_zero > total / 2,
        "Flow field should have directions after building removal, got {non_zero}/{total}"
    );

    // Fortress cells should be blocked in cost grid
    for &(col, row) in &pf_cells {
        assert_eq!(
            registry.cost_grid.get(col, row),
            COST_BLOCKED,
            "Fortress cell ({col}, {row}) should be blocked"
        );
    }
}
```

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes (clippy + compile)
- [ ] `make test` passes (all existing + new regression tests)
- [ ] New test `dijkstra_propagates_from_blocked_goal_cells` passes
- [ ] New test `rebuild_with_fortress_blocking_preserves_flow_field` passes

#### Manual Verification:
- [ ] Start a game and place buildings
- [ ] Let enemy units destroy a building
- [ ] Confirm all enemy units continue moving toward the player fortress
- [ ] Confirm units still path around remaining buildings correctly
- [ ] Confirm units path around fortresses they're not targeting

## References

- Linear ticket: [GAM-66](https://linear.app/tayhu-games/issue/GAM-66/flow-field-destroyed-when-building-is-removed)
- Key file: `src/gameplay/flow_field.rs` (lines 515-650)

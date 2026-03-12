# Fix Flow Field Destroyed When Building Is Removed (GAM-66)

## Overview

When an enemy unit destroys a building, the flow field becomes entirely `Vec2::ZERO` and all enemy units stop moving permanently. The root cause is an asymmetry between how the initial cost grid is built (fortresses not blocked) and how the cost grid is rebuilt after a building removal (fortresses marked as blocked). Since fortress cells are also the Dijkstra goal cells, blocking them prevents propagation.

## Current State Analysis

### The Bug

Three code paths modify the cost grid:

| Path | Fortresses blocked? | File:Line |
|---|---|---|
| `setup_flow_fields` (initial) | No — empty grid | `flow_field.rs:520` |
| `mark_building_placed` (placement) | No — only marks new building | `flow_field.rs:566` |
| `on_building_removed` (removal) | **Yes — BUG** | `flow_field.rs:641-647` |

The observer at `flow_field.rs:614` queries both remaining buildings AND fortresses, then passes all AABBs to `rebuild_cost_grid_from_buildings`. `mark_building` inflates each AABB by `UNIT_RADIUS` (6px) and marks cells as `COST_BLOCKED`. Since the fortress cells overlap the Dijkstra goal cells (both are computed from the same world position), the goal cells' immediate neighbors become blocked, preventing Dijkstra propagation entirely.

### Key Discoveries

- Fortresses do NOT have the `Building` component (`battlefield/renderer.rs:62-92, 139-169`) — the observer cannot fire for fortress death
- The fortress query in `on_building_removed` (lines 619-625) exists solely to include fortress AABBs in the rebuild
- Goal cells are computed once in `setup_flow_fields` and stored permanently in `GoalRegistry` — never re-queried
- `strip_buildings_before_despawn` (`building/mod.rs:184-191`) also triggers this observer on `OnExit(GameState::InGame)`, but the flow field resource is cleaned up by `DespawnOnExit` anyway

## Desired End State

After a building is destroyed, the cost grid is rebuilt from only the remaining buildings (no fortresses), matching the behavior of `setup_flow_fields` and `mark_building_placed`. The flow field recomputes correctly and units continue moving.

### Verification

- Automated: new regression test proves flow field has non-zero directions after building removal
- Manual: destroy a building in-game, confirm enemy units keep moving

## What We're NOT Doing

- Not changing how fortresses interact with pathing at a higher level (they're goals, not obstacles)
- Not refactoring the cost grid rebuild to be incremental (full rebuild is correct for removal)
- Not adding fortress blocking with goal-cell exemptions (unnecessary complexity)

## Implementation Approach

Single-phase fix: remove the fortress query from the observer and add a regression test.

## Phase 1: Fix and Test

### Changes Required

#### 1. Remove Fortress Query from `on_building_removed`
**File**: `src/gameplay/flow_field.rs`
**Changes**: Remove the fortress query parameter and the fortress collection loop.

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

    // Collect remaining buildings (fortresses are goal cells, not obstacles)
    for (transform, extent) in &buildings {
        let pos = transform.translation.xy();
        if let EntityExtent::Rect(hw, hh) = extent {
            aabbs.push((pos - Vec2::new(*hw, *hh), pos + Vec2::new(*hw, *hh)));
        }
    }

    rebuild_cost_grid_from_buildings(registry, dirty, &aabbs);
}
```

#### 2. Add Regression Test
**File**: `src/gameplay/flow_field.rs` (in `mod tests`)
**Changes**: Add a test that simulates the bug scenario — mark a building, rebuild with no buildings (simulating removal of the only building), recompute, and verify the flow field has non-zero directions.

```rust
#[test]
fn rebuild_without_fortresses_preserves_flow_field() {
    let cost_grid = CostGrid::new(FLOW_COLS, FLOW_ROWS);
    let algo = Box::new(DijkstraFlowField);
    let pf_cells = fortress_cells_from_world(Vec2::new(64.0, 320.0));
    let ef_cells = fortress_cells_from_world(Vec2::new(5184.0, 320.0));
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

    // Remove the building (rebuild with empty list — no fortresses!)
    rebuild_cost_grid_from_buildings(&mut registry, &mut dirty, &[]);

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
}
```

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes (clippy + compile)
- [ ] `make test` passes (all existing + new regression test)
- [ ] New test `rebuild_without_fortresses_preserves_flow_field` passes

#### Manual Verification:
- [ ] Start a game and place buildings
- [ ] Let enemy units destroy a building
- [ ] Confirm all enemy units continue moving toward the player fortress
- [ ] Confirm units still path around remaining buildings correctly

## References

- Linear ticket: [GAM-66](https://linear.app/tayhu-games/issue/GAM-66/flow-field-destroyed-when-building-is-removed)
- Key file: `src/gameplay/flow_field.rs` (lines 614-650)

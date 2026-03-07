# Flow Field Infrastructure + Remove Navmesh (GAM-60) Implementation Plan

## Overview

Replace per-unit navmesh pathfinding with a precomputed flow field grid. Each team gets one flow field (82x10 cells, 64px each) computed via Dijkstra from their goal (enemy fortress). Units read their cell's direction vector — O(1) per unit instead of O(n) A* calls. Remove `vleue_navigator` dependency entirely.

## Current State Analysis

Three systems form the movement pipeline:

1. **`compute_paths`** (`units/pathfinding.rs:122`) — queries navmesh A* for each unit with a target, writes `NavPath` waypoints. Runs in `GameSet::Ai` after `find_target`. Triggered by: target change, 0.5s periodic refresh, or path consumed.
2. **`unit_movement`** (`units/movement.rs:28`) — follows `NavPath` waypoints, writes `PreferredVelocity`. Stops when in attack range or no waypoints. Runs in `GameSet::Movement`.
3. **`compute_avoidance`** (`units/avoidance/mod.rs:114`) — ORCA reads `PreferredVelocity`, writes `LinearVelocity`. Survives this ticket.

Navmesh infrastructure:
- `NavMeshSettings` entity spawned in `renderer.rs:210-224`
- `NavmeshUpdaterPlugin::<Collider, NavObstacle>` auto-rebuilds on `NavObstacle` add/remove (`third_party/vleue_navigator.rs:32`)
- `NavObstacle` on fortresses (`renderer.rs:98,181`) and buildings (`placement.rs:147`)
- `snap_to_mesh()` helper walks off-mesh targets to nearest mesh edge
- `random_navigable_spawn()` validates spawn points against navmesh

### Key Discoveries

- `TargetingState::Moving` variant already exists (`gameplay/mod.rs:78`) with correct doc comment — unused by any system
- `PreferredVelocity` defined in `avoidance/mod.rs:27-29`, written by `unit_movement`, read by `compute_avoidance`
- No observer or event on building placement — `handle_building_placement` (`placement.rs:70`) is a plain system in `GameSet::Input`
- `SpatialHash` at `gameplay/spatial_hash.rs` is generic and reusable (used by avoidance + targeting)
- Battlefield constants in `battlefield/mod.rs`: `TOTAL_COLS=82`, `BATTLEFIELD_ROWS=10`, `CELL_SIZE=64.0`
- Dev tools (`dev_tools/mod.rs`) use F3 toggle with `NavMeshesDebug` marker resource for navmesh + path overlays

## Desired End State

- `FlowField` struct computes Dijkstra direction grid from goal position
- `GoalRegistry` resource holds one flow field per team, recomputed on topology change
- `unit_movement` rewritten: no-target units follow flow field, engaged units steer direct
- No `vleue_navigator` dependency, no `NavPath`, no `NavObstacle`, no `PathRefreshTimer`
- `PreferredVelocity` still written by movement, read by ORCA (bridge until GAM-61)
- F3 dev toggle shows flow field arrows instead of navmesh overlay
- Unit ejection on building placement prevents stuck units

### Verification

- `make check` and `make test` pass
- Units visually flow around buildings toward enemy fortress
- Building placement triggers visible flow field recalculation (F3 overlay)
- No units stuck inside buildings after placement

## What We're NOT Doing

- NOT adding `AssignedGoal` component — derive from `Team::opposing()` for now
- NOT using `TargetingState::Moving` actively — units stay `Seeking`, optimization deferred
- NOT optimizing for 40k+ units — that's Ticket 6 (profiling)
- NOT removing ORCA or `PreferredVelocity` — that's GAM-61
- NOT removing avian2d physics — that's GAM-62
- NOT adding `EngagementLeash` behavior — exists as component, behavior deferred

## Implementation Approach

De-risked four-phase strategy: build flow field infrastructure, integrate into movement, add building hooks + ejection, then remove navmesh. Each phase leaves the game working.

---

## Phase 1: FlowField Struct + Math

### Overview

Define `FlowField` struct with cost field, Dijkstra computation, and direction lookup. Pure data + algorithms, no Bevy integration yet. Thorough unit tests.

### Changes Required

#### 1. New file: `gameplay/flow_field.rs`

```rust
//! Precomputed flow field for macro unit movement.
//!
//! Each cell stores a direction vector pointing toward the goal via the lowest-cost
//! path. Units read their cell — O(1) per unit. Recomputed only on topology change
//! (building placed/destroyed).

use std::collections::BinaryHeap;
use std::cmp::Ordering;

use bevy::prelude::*;

use super::battlefield::{BATTLEFIELD_ROWS, CELL_SIZE, TOTAL_COLS};

// === Cost Constants ===

/// Movement cost for an open cell.
const COST_OPEN: f32 = 1.0;

/// Movement cost for cells adjacent to a building (8-neighbor ring).
/// Discourages corner-hugging, prevents clipping.
const COST_ADJACENT: f32 = 3.0;

/// Diagonal move multiplier (sqrt(2)).
const DIAGONAL_COST: f32 = 1.414;

/// 8-connected neighbor offsets: (dx, dy).
const NEIGHBORS: [(i32, i32); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1,  0),          (1,  0),
    (-1,  1), (0,  1), (1,  1),
];

// === FlowField ===

/// Precomputed direction grid for unit movement.
///
/// The grid covers the full battlefield (TOTAL_COLS x BATTLEFIELD_ROWS).
/// Each cell stores a normalized direction vector pointing toward the goal
/// via the lowest-cost path.
#[derive(Debug, Clone, Reflect)]
pub struct FlowField {
    /// Width in cells.
    width: u32,
    /// Height in cells.
    height: u32,
    /// Per-cell movement cost. `f32::INFINITY` = impassable.
    costs: Vec<f32>,
    /// Per-cell direction vector (normalized). `Vec2::ZERO` for goal cell or unreachable.
    directions: Vec<Vec2>,
}

impl FlowField {
    /// Create a new flow field with all cells open (cost = 1.0).
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            width,
            height,
            costs: vec![COST_OPEN; size],
            directions: vec![Vec2::ZERO; size],
        }
    }

    /// Create a flow field with standard battlefield dimensions.
    #[must_use]
    pub fn new_battlefield() -> Self {
        Self::new(u32::from(TOTAL_COLS), u32::from(BATTLEFIELD_ROWS))
    }

    /// Set a cell as blocked (building cell). Cost = INFINITY.
    pub fn set_blocked(&mut self, col: u32, row: u32) {
        if let Some(idx) = self.index(col, row) {
            self.costs[idx] = f32::INFINITY;
        }
    }

    /// Set a cell's cost to the adjacent-to-building penalty.
    /// Only applies if the cell is currently open (doesn't downgrade a block).
    pub fn set_adjacent(&mut self, col: u32, row: u32) {
        if let Some(idx) = self.index(col, row) {
            if self.costs[idx] < COST_ADJACENT {
                self.costs[idx] = COST_ADJACENT;
            }
        }
    }

    /// Reset all costs to open.
    pub fn reset_costs(&mut self) {
        self.costs.fill(COST_OPEN);
    }

    /// Check if a cell is blocked (cost = INFINITY).
    #[must_use]
    pub fn is_blocked(&self, col: u32, row: u32) -> bool {
        self.index(col, row)
            .is_some_and(|idx| self.costs[idx] >= f32::INFINITY)
    }

    /// Get the direction vector at a world-space position.
    /// Returns `Vec2::ZERO` if out of bounds.
    #[must_use]
    pub fn direction_at(&self, world_pos: Vec2) -> Vec2 {
        let (col, row) = self.world_to_cell(world_pos);
        self.index(col, row)
            .map_or(Vec2::ZERO, |idx| self.directions[idx])
    }

    /// Convert world-space position to grid cell coordinates.
    #[must_use]
    pub fn world_to_cell(&self, world_pos: Vec2) -> (u32, u32) {
        let col = (world_pos.x / CELL_SIZE).floor().max(0.0) as u32;
        let row = (world_pos.y / CELL_SIZE).floor().max(0.0) as u32;
        (col.min(self.width - 1), row.min(self.height - 1))
    }

    /// Convert grid cell to world-space center position.
    #[must_use]
    pub fn cell_to_world(&self, col: u32, row: u32) -> Vec2 {
        Vec2::new(
            col as f32 * CELL_SIZE + CELL_SIZE / 2.0,
            row as f32 * CELL_SIZE + CELL_SIZE / 2.0,
        )
    }

    /// Compute directions via Dijkstra from a goal world-space position.
    ///
    /// After this call, each reachable cell's `directions[idx]` points toward
    /// the neighboring cell with the lowest integrated cost.
    pub fn compute(&mut self, goal_world: Vec2) {
        let (goal_col, goal_row) = self.world_to_cell(goal_world);
        let goal_idx = self.index(goal_col, goal_row).expect("goal in bounds");
        let size = self.costs.len();

        // Integrated cost from goal to each cell
        let mut integrated: Vec<f32> = vec![f32::INFINITY; size];
        integrated[goal_idx] = 0.0;

        // Dijkstra priority queue
        let mut heap = BinaryHeap::new();
        heap.push(DijkstraNode { cost: 0.0, col: goal_col, row: goal_row });

        // Reset directions
        self.directions.fill(Vec2::ZERO);

        while let Some(node) = heap.pop() {
            let idx = self.flat_index(node.col, node.row);
            if node.cost > integrated[idx] {
                continue; // Stale entry
            }

            for &(dx, dy) in &NEIGHBORS {
                let nx = node.col as i32 + dx;
                let ny = node.row as i32 + dy;

                if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                    continue;
                }

                let ncol = nx as u32;
                let nrow = ny as u32;
                let nidx = self.flat_index(ncol, nrow);

                // Skip blocked cells
                if self.costs[nidx] >= f32::INFINITY {
                    continue;
                }

                // Corner-cutting prevention: diagonal moves require both cardinals passable
                if dx != 0 && dy != 0 {
                    let cx = self.flat_index((node.col as i32 + dx) as u32, node.row);
                    let cy = self.flat_index(node.col, (node.row as i32 + dy) as u32);
                    if self.costs[cx] >= f32::INFINITY || self.costs[cy] >= f32::INFINITY {
                        continue;
                    }
                }

                let move_cost = if dx != 0 && dy != 0 {
                    DIAGONAL_COST * self.costs[nidx]
                } else {
                    self.costs[nidx]
                };

                let new_cost = node.cost + move_cost;
                if new_cost < integrated[nidx] {
                    integrated[nidx] = new_cost;
                    heap.push(DijkstraNode { cost: new_cost, col: ncol, row: nrow });
                }
            }
        }

        // Compute direction vectors: each cell points toward the neighbor with lowest integrated cost.
        // Goal cell and immediate neighbors point at the goal world-space center.
        let goal_world_center = self.cell_to_world(goal_col, goal_row);

        for row in 0..self.height {
            for col in 0..self.width {
                let idx = self.flat_index(col, row);
                if integrated[idx] >= f32::INFINITY {
                    continue; // Unreachable
                }
                if idx == goal_idx {
                    continue; // Goal cell stays Vec2::ZERO
                }

                // Check if this is an immediate neighbor of the goal
                let is_goal_neighbor = (col as i32 - goal_col as i32).unsigned_abs() <= 1
                    && (row as i32 - goal_row as i32).unsigned_abs() <= 1;

                if is_goal_neighbor {
                    // Point directly at goal world center for smooth final approach
                    let cell_center = self.cell_to_world(col, row);
                    let dir = (goal_world_center - cell_center).normalize_or_zero();
                    self.directions[idx] = dir;
                    continue;
                }

                // Find the neighbor with the lowest integrated cost
                let mut best_cost = integrated[idx];
                let mut best_dir = Vec2::ZERO;

                for &(dx, dy) in &NEIGHBORS {
                    let nx = col as i32 + dx;
                    let ny = row as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                        continue;
                    }
                    let ncol = nx as u32;
                    let nrow = ny as u32;
                    let nidx = self.flat_index(ncol, nrow);

                    // Corner-cutting prevention
                    if dx != 0 && dy != 0 {
                        let cx = self.flat_index((col as i32 + dx) as u32, row);
                        let cy = self.flat_index(col, (row as i32 + dy) as u32);
                        if self.costs[cx] >= f32::INFINITY || self.costs[cy] >= f32::INFINITY {
                            continue;
                        }
                    }

                    if integrated[nidx] < best_cost {
                        best_cost = integrated[nidx];
                        best_dir = Vec2::new(dx as f32, dy as f32).normalize();
                    }
                }

                self.directions[idx] = best_dir;
            }
        }
    }

    /// Flat array index from (col, row). Panics if out of bounds.
    fn flat_index(&self, col: u32, row: u32) -> usize {
        (row * self.width + col) as usize
    }

    /// Flat array index from (col, row), returning None if out of bounds.
    fn index(&self, col: u32, row: u32) -> Option<usize> {
        if col < self.width && row < self.height {
            Some(self.flat_index(col, row))
        } else {
            None
        }
    }
}

/// Priority queue node for Dijkstra.
#[derive(Debug, Clone, PartialEq)]
struct DijkstraNode {
    cost: f32,
    col: u32,
    row: u32,
}

impl Eq for DijkstraNode {}

impl Ord for DijkstraNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for DijkstraNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
```

#### 2. `gameplay/mod.rs` — Register module

Add `pub mod flow_field;` after existing module declarations (line 27).

#### 3. Unit tests in `flow_field.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_battlefield_dimensions() {
        let ff = FlowField::new_battlefield();
        assert_eq!(ff.width, 82);
        assert_eq!(ff.height, 10);
        assert_eq!(ff.costs.len(), 820);
        assert_eq!(ff.directions.len(), 820);
    }

    #[test]
    fn world_to_cell_basic() {
        let ff = FlowField::new_battlefield();
        assert_eq!(ff.world_to_cell(Vec2::new(32.0, 32.0)), (0, 0));
        assert_eq!(ff.world_to_cell(Vec2::new(96.0, 32.0)), (1, 0));
        assert_eq!(ff.world_to_cell(Vec2::new(0.0, 0.0)), (0, 0));
    }

    #[test]
    fn world_to_cell_clamps() {
        let ff = FlowField::new_battlefield();
        assert_eq!(ff.world_to_cell(Vec2::new(-100.0, -100.0)), (0, 0));
        assert_eq!(ff.world_to_cell(Vec2::new(99999.0, 99999.0)), (81, 9));
    }

    #[test]
    fn cell_to_world_centers() {
        let ff = FlowField::new_battlefield();
        assert_eq!(ff.cell_to_world(0, 0), Vec2::new(32.0, 32.0));
        assert_eq!(ff.cell_to_world(1, 0), Vec2::new(96.0, 32.0));
    }

    #[test]
    fn set_blocked_makes_impassable() {
        let mut ff = FlowField::new(4, 4);
        ff.set_blocked(2, 1);
        assert!(ff.is_blocked(2, 1));
        assert!(!ff.is_blocked(0, 0));
    }

    #[test]
    fn set_adjacent_raises_cost() {
        let mut ff = FlowField::new(4, 4);
        ff.set_adjacent(1, 1);
        let idx = ff.flat_index(1, 1);
        assert!((ff.costs[idx] - COST_ADJACENT).abs() < f32::EPSILON);
    }

    #[test]
    fn set_adjacent_does_not_downgrade_block() {
        let mut ff = FlowField::new(4, 4);
        ff.set_blocked(1, 1);
        ff.set_adjacent(1, 1); // Should not change INFINITY to 3.0
        assert!(ff.is_blocked(1, 1));
    }

    #[test]
    fn compute_simple_rightward_flow() {
        // 4x1 grid, goal at right edge
        let mut ff = FlowField::new(4, 1);
        ff.compute(Vec2::new(3.0 * CELL_SIZE + 32.0, 32.0)); // Goal at col 3

        // All cells should point right (positive x)
        for col in 0..3 {
            let dir = ff.directions[col as usize];
            assert!(dir.x > 0.0, "col {col} should point right, got {dir}");
        }
        // Goal cell should be zero
        assert_eq!(ff.directions[3], Vec2::ZERO);
    }

    #[test]
    fn compute_routes_around_obstacle() {
        // 5x3 grid with a wall at col 2, rows 0-1. Goal at col 4.
        let mut ff = FlowField::new(5, 3);
        ff.set_blocked(2, 0);
        ff.set_blocked(2, 1);
        ff.compute(Vec2::new(4.0 * CELL_SIZE + 32.0, 32.0)); // Goal at (4, 0)

        // Cell (1, 0) is left of the wall — should route through row 2
        let dir = ff.directions[ff.flat_index(1, 0)];
        // Should have a downward component (positive y) to go around
        assert!(dir.y > 0.0 || dir.x > 0.0,
            "Cell (1,0) should route around obstacle, got {dir}");

        // Cell (3, 0) is right of the wall — should point right toward goal
        let dir = ff.directions[ff.flat_index(3, 0)];
        assert!(dir.x > 0.0, "Cell (3,0) should point right, got {dir}");
    }

    #[test]
    fn compute_blocked_cells_have_zero_direction() {
        let mut ff = FlowField::new(4, 4);
        ff.set_blocked(2, 2);
        ff.compute(Vec2::new(3.0 * CELL_SIZE + 32.0, 32.0));
        assert_eq!(ff.directions[ff.flat_index(2, 2)], Vec2::ZERO);
    }

    #[test]
    fn direction_at_returns_correct_vector() {
        let mut ff = FlowField::new(4, 1);
        ff.compute(Vec2::new(3.0 * CELL_SIZE + 32.0, 32.0));

        let dir = ff.direction_at(Vec2::new(32.0, 32.0)); // col 0
        assert!(dir.x > 0.0, "Should point right toward goal");
    }

    #[test]
    fn direction_at_out_of_bounds_returns_zero() {
        let ff = FlowField::new(4, 4);
        assert_eq!(ff.direction_at(Vec2::new(-100.0, -100.0)), Vec2::ZERO);
    }

    #[test]
    fn reset_costs_clears_blocks() {
        let mut ff = FlowField::new(4, 4);
        ff.set_blocked(1, 1);
        ff.reset_costs();
        assert!(!ff.is_blocked(1, 1));
    }

    #[test]
    fn corner_cutting_prevention() {
        // 3x3 grid with blocks at (1,0) and (0,1). Diagonal (1,1) should be blocked.
        let mut ff = FlowField::new(3, 3);
        ff.set_blocked(1, 0);
        ff.set_blocked(0, 1);
        ff.compute(Vec2::new(2.0 * CELL_SIZE + 32.0, 2.0 * CELL_SIZE + 32.0));

        // Cell (0,0) should NOT point diagonally toward (1,1) since both cardinals are blocked
        let dir = ff.directions[ff.flat_index(0, 0)];
        // It should be zero (unreachable) or point along an available cardinal
        assert!(
            dir == Vec2::ZERO || dir.x.abs() < f32::EPSILON || dir.y.abs() < f32::EPSILON,
            "Cell (0,0) should not cut through blocked corner, got {dir}"
        );
    }
}
```

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — new unit tests for FlowField math

#### Manual Verification:
- [ ] None needed — pure data + math, no visual behavior

---

## Phase 2: GoalRegistry + Bevy Integration

### Overview

Add `GoalRegistry` resource, `FlowFieldDirty` marker, and systems to compute/recompute flow fields. Hook into building placement for the dirty flag.

### Changes Required

#### 1. `gameplay/flow_field.rs` — Add Bevy resources and systems

Append to the file:

```rust
use super::battlefield::{
    BATTLEFIELD_HEIGHT, BATTLEFIELD_WIDTH, BUILD_ZONE_START_COL, BUILD_ZONE_COLS,
    EnemyFortress, PlayerFortress,
};
use super::building::Building;
use super::Team;
use crate::screens::GameState;
use crate::GameSet;

// === Resources ===

/// Holds one flow field per team. Player flow field routes toward enemy fortress,
/// enemy flow field routes toward player fortress.
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct GoalRegistry {
    /// Flow field for player units (goal = enemy fortress).
    pub player_field: FlowField,
    /// Flow field for enemy units (goal = player fortress).
    pub enemy_field: FlowField,
}

/// Marker resource: when present, flow fields need recomputation.
/// Inserted on building placement/destruction. Consumed by `recompute_flow_fields`.
#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct FlowFieldDirty;

// === Systems ===

/// Initial flow field computation on entering InGame.
/// Reads fortress positions and computes both flow fields.
fn initialize_flow_fields(
    mut commands: Commands,
    player_fortress: Single<&Transform, With<PlayerFortress>>,
    enemy_fortress: Single<&Transform, With<EnemyFortress>>,
    buildings: Query<&Transform, With<Building>>,
) {
    let mut registry = GoalRegistry {
        player_field: FlowField::new_battlefield(),
        enemy_field: FlowField::new_battlefield(),
    };

    apply_building_costs(&mut registry.player_field, &buildings);
    apply_building_costs(&mut registry.enemy_field, &buildings);

    // Apply fortress blocks (both fortresses block both flow fields)
    apply_fortress_blocks(&mut registry.player_field, &player_fortress, &enemy_fortress);
    apply_fortress_blocks(&mut registry.enemy_field, &player_fortress, &enemy_fortress);

    registry.player_field.compute(enemy_fortress.translation.xy());
    registry.enemy_field.compute(player_fortress.translation.xy());

    commands.insert_resource(registry);
}

/// Recompute flow fields when the dirty flag is set (building placed/destroyed).
fn recompute_flow_fields(
    mut commands: Commands,
    dirty: Option<Res<FlowFieldDirty>>,
    mut registry: ResMut<GoalRegistry>,
    player_fortress: Single<&Transform, With<PlayerFortress>>,
    enemy_fortress: Single<&Transform, With<EnemyFortress>>,
    buildings: Query<&Transform, With<Building>>,
) {
    if dirty.is_none() {
        return;
    }

    // Reset and reapply costs
    registry.player_field.reset_costs();
    registry.enemy_field.reset_costs();

    apply_building_costs(&mut registry.player_field, &buildings);
    apply_building_costs(&mut registry.enemy_field, &buildings);

    apply_fortress_blocks(&mut registry.player_field, &player_fortress, &enemy_fortress);
    apply_fortress_blocks(&mut registry.enemy_field, &player_fortress, &enemy_fortress);

    registry.player_field.compute(enemy_fortress.translation.xy());
    registry.enemy_field.compute(player_fortress.translation.xy());

    commands.remove_resource::<FlowFieldDirty>();
}

/// Apply building positions as blocked cells + adjacent cost ring to a flow field.
fn apply_building_costs(field: &mut FlowField, buildings: &Query<&Transform, With<Building>>) {
    for transform in buildings {
        let pos = transform.translation.xy();
        let (col, row) = field.world_to_cell(pos);

        // Block the building cell
        field.set_blocked(col, row);

        // Set adjacent cells (8-neighbor ring) to higher cost
        for &(dx, dy) in &NEIGHBORS {
            let nx = col as i32 + dx;
            let ny = row as i32 + dy;
            if nx >= 0 && ny >= 0 {
                field.set_adjacent(nx as u32, ny as u32);
            }
        }
    }
}

/// Apply fortress positions as blocked cells to a flow field.
/// Fortresses are 2x2 cells.
fn apply_fortress_blocks(
    field: &mut FlowField,
    player_fortress: &Transform,
    enemy_fortress: &Transform,
) {
    for fortress_transform in [player_fortress, enemy_fortress] {
        let pos = fortress_transform.translation.xy();
        // Fortress is 2x2 cells. Find the top-left cell from center.
        let center_col = (pos.x / CELL_SIZE).floor() as u32;
        let center_row = (pos.y / CELL_SIZE).floor() as u32;

        // The fortress center is at the center of a 2x2 block.
        // zone_center_x(start_col, 2) = start_col * 64 + 64 = (start_col + 1) * 64
        // So the fortress occupies start_col and start_col+1.
        // We need to figure out which 2x2 cells from the center position.
        // Fortress size = 128x128, so it spans 2 cols and 2 rows.
        // Center is between the 2 cells. Let's compute from FORTRESS_ROWS/COLS.
        use super::battlefield::{FORTRESS_COLS, FORTRESS_ROWS};
        let half_w = f32::from(FORTRESS_COLS) / 2.0;
        let half_h = f32::from(FORTRESS_ROWS) / 2.0;
        let start_col = ((pos.x / CELL_SIZE) - half_w).floor().max(0.0) as u32;
        let start_row = ((pos.y / CELL_SIZE) - half_h).floor().max(0.0) as u32;

        for dc in 0..u32::from(FORTRESS_COLS) {
            for dr in 0..u32::from(FORTRESS_ROWS) {
                field.set_blocked(start_col + dc, start_row + dr);
            }
        }

        // Adjacent cost ring around fortress
        let end_col = start_col + u32::from(FORTRESS_COLS);
        let end_row = start_row + u32::from(FORTRESS_ROWS);
        for col in start_col.saturating_sub(1)..=(end_col) {
            for row in start_row.saturating_sub(1)..=(end_row) {
                field.set_adjacent(col, row);
            }
        }
    }
}
```

#### 2. `gameplay/flow_field.rs` — Plugin function

```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<GoalRegistry>()
        .register_type::<FlowFieldDirty>();

    // Initialize flow fields after battlefield is spawned
    app.add_systems(
        OnEnter(GameState::InGame),
        initialize_flow_fields
            .after(super::battlefield::BattlefieldSetup),
    );

    // Recompute when dirty, at the start of Ai set (before find_target)
    app.add_systems(
        Update,
        recompute_flow_fields
            .in_set(GameSet::Ai)
            .run_if(crate::gameplay_running),
    );
}
```

#### 3. `gameplay/mod.rs` — Register flow_field plugin

Add `flow_field::plugin,` to the plugin list (after `ai::plugin,`).

#### 4. `gameplay/building/placement.rs` — Set dirty flag on placement

In `handle_building_placement`, after spawning the building entity, add:

```rust
commands.insert_resource(crate::gameplay::flow_field::FlowFieldDirty);
```

#### 5. Unit ejection on building placement

Add to `handle_building_placement` after spawning the building, before the dirty flag:

```rust
// Eject units from the building cell
let building_pos = Vec2::new(world_x, world_y);
for (unit_entity, unit_transform) in units_query.iter() {
    let unit_pos = unit_transform.translation.xy();
    let dist = unit_pos.distance(building_pos);
    if dist < CELL_SIZE / 2.0 {
        // Push radially outward from building center
        let push_dir = (unit_pos - building_pos).normalize_or_zero();
        let push_dir = if push_dir == Vec2::ZERO { Vec2::X } else { push_dir };
        let new_pos = building_pos + push_dir * CELL_SIZE;
        commands.entity(unit_entity).insert(
            Transform::from_xyz(new_pos.x, new_pos.y, unit_transform.translation.z),
        );
    }
}
```

This requires adding `units_query: Query<(Entity, &Transform), With<Unit>>` to the system signature and importing `Unit`.

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes

#### Manual Verification:
- [ ] Flow fields computed without errors on game start (no panic/crash)
- [ ] Building placement triggers recompute (verify via debug logging or F3 overlay in Phase 4)

**Implementation Note**: Pause for manual verification before proceeding to Phase 3.

---

## Phase 3: Rewrite Movement + Dev Tools

### Overview

Rewrite `unit_movement` to use flow field directions instead of `NavPath` waypoints. Replace navmesh debug overlay with flow field arrow visualization.

### Changes Required

#### 1. `gameplay/units/movement.rs` — Rewrite `unit_movement`

```rust
//! Unit movement: flow field macro-movement + direct steering for engaged targets.

use bevy::prelude::*;

use super::avoidance::PreferredVelocity;
use super::{CombatStats, Movement, TargetingState, Unit};
use crate::gameplay::flow_field::GoalRegistry;
use crate::gameplay::{EntityExtent, Team, extent_distance};

/// Sets unit `PreferredVelocity` based on `TargetingState`:
///
/// - **No target** (Seeking/Moving): follow flow field direction toward team's goal.
/// - **Engaging**: steer directly toward target entity.
/// - **Attacking**: velocity = 0 (in range, attack system fires).
///
/// The downstream `compute_avoidance` system reads `PreferredVelocity`
/// and writes the final `LinearVelocity`.
///
/// Runs in `GameSet::Movement`.
pub(super) fn unit_movement(
    mut units: Query<
        (
            &TargetingState,
            &Movement,
            &CombatStats,
            &GlobalTransform,
            &EntityExtent,
            &Team,
            &mut PreferredVelocity,
        ),
        With<Unit>,
    >,
    targets: Query<(&GlobalTransform, &EntityExtent)>,
    goals: Option<Res<GoalRegistry>>,
) {
    let Some(goals) = goals else {
        return; // Flow fields not yet initialized
    };

    for (targeting_state, movement, stats, global_transform, unit_extent, team, mut preferred) in
        &mut units
    {
        let current_xy = global_transform.translation().xy();

        match targeting_state {
            TargetingState::Moving | TargetingState::Seeking => {
                // Follow flow field toward team's goal
                let field = match team {
                    Team::Player => &goals.player_field,
                    Team::Enemy => &goals.enemy_field,
                };
                let direction = field.direction_at(current_xy);
                preferred.0 = direction * movement.speed;
            }
            TargetingState::Engaging(target_entity) => {
                let Ok((target_pos, target_extent)) = targets.get(*target_entity) else {
                    preferred.0 = Vec2::ZERO;
                    continue;
                };
                let target_xy = target_pos.translation().xy();
                let distance =
                    extent_distance(unit_extent, current_xy, target_extent, target_xy);

                if distance <= stats.range {
                    // In attack range — stop
                    preferred.0 = Vec2::ZERO;
                } else {
                    // Steer directly toward target
                    let diff = target_xy - current_xy;
                    let dist = diff.length();
                    if dist < f32::EPSILON {
                        preferred.0 = Vec2::ZERO;
                    } else {
                        preferred.0 = (diff / dist) * movement.speed;
                    }
                }
            }
            TargetingState::Attacking(_) => {
                preferred.0 = Vec2::ZERO;
            }
        }
    }
}
```

#### 2. `gameplay/units/mod.rs` — Remove NavPath from unit archetype

In `spawn_unit` (line 131), remove `pathfinding::NavPath::default(),`.

Remove `pathfinding::NavPath::default(),` from the `.insert(...)` block.

Add `Team` query access: the unit already has `team` in the first `.spawn(...)` block, so movement can read it.

#### 3. `gameplay/units/mod.rs` — Remove pathfinding system registration

Remove from the `add_systems(Update, ...)` block:
```rust
pathfinding::compute_paths
    .in_set(GameSet::Ai)
    .after(crate::gameplay::ai::find_target),
```

Remove `reset_path_refresh_timer` from `OnEnter(GameState::InGame)`.

Remove registrations:
```rust
.register_type::<pathfinding::NavPath>()
.register_type::<pathfinding::PathRefreshTimer>()
.init_resource::<pathfinding::PathRefreshTimer>()
```

Remove `pub mod pathfinding;` declaration (line 5).
Remove `use vleue_navigator::prelude::NavMesh;` (line 10).
Remove `reset_path_refresh_timer` function.

#### 4. `src/testing.rs` — Remove NavPath from test helper

Remove `NavPath::default(),` from `spawn_test_unit` (line 185).
Remove `use crate::gameplay::units::pathfinding::NavPath;` (line 14).

#### 5. `dev_tools/mod.rs` — Replace navmesh overlay with flow field arrows

```rust
//! Development tools — only included with `cargo run --features dev`.

use bevy::prelude::*;

use avian2d::prelude::LinearVelocity;

use crate::gameplay::flow_field::GoalRegistry;
use crate::gameplay::units::Unit;
use crate::gameplay::units::avoidance::PreferredVelocity;
use crate::gameplay::Team;
use crate::gameplay::battlefield::CELL_SIZE;

/// Marker resource: when present, the world inspector is shown.
#[derive(Resource)]
struct ShowWorldInspector;

/// Marker resource: when present, flow field debug overlay is drawn.
#[derive(Resource)]
pub struct ShowFlowFieldDebug;

pub fn plugin(app: &mut App) {
    if app.is_plugin_added::<bevy::render::RenderPlugin>() {
        app.add_plugins(bevy_inspector_egui::bevy_egui::EguiPlugin::default());
        app.add_plugins(
            bevy_inspector_egui::quick::WorldInspectorPlugin::default()
                .run_if(resource_exists::<ShowWorldInspector>),
        );
        app.add_systems(Update, toggle_world_inspector);
    }

    app.add_systems(Update, toggle_flow_field_debug);
    app.add_systems(
        Update,
        (debug_draw_flow_field, debug_draw_avoidance)
            .run_if(crate::gameplay_running.and(resource_exists::<ShowFlowFieldDebug>)),
    );
}

fn toggle_world_inspector(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    existing: Option<Res<ShowWorldInspector>>,
) {
    if input.just_pressed(KeyCode::F4) {
        if existing.is_some() {
            commands.remove_resource::<ShowWorldInspector>();
        } else {
            commands.insert_resource(ShowWorldInspector);
        }
    }
}

fn toggle_flow_field_debug(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    existing: Option<Res<ShowFlowFieldDebug>>,
) {
    if input.just_pressed(KeyCode::F3) {
        if existing.is_some() {
            commands.remove_resource::<ShowFlowFieldDebug>();
        } else {
            commands.insert_resource(ShowFlowFieldDebug);
        }
    }
}

/// Draw flow field direction arrows for the player team's flow field.
fn debug_draw_flow_field(
    goals: Option<Res<GoalRegistry>>,
    mut gizmos: Gizmos,
) {
    let Some(goals) = goals else { return };
    let field = &goals.player_field;

    for row in 0..field.height() {
        for col in 0..field.width() {
            let dir = field.direction_at_cell(col, row);
            if dir == Vec2::ZERO {
                continue;
            }
            let center = field.cell_to_world(col, row);
            let arrow_len = CELL_SIZE * 0.35;
            gizmos.arrow_2d(
                center,
                center + dir * arrow_len,
                Color::srgba(1.0, 0.5, 0.0, 0.4),
            );
        }
    }
}

/// Draw ORCA debug visualization: green = preferred velocity, cyan = actual (ORCA-adjusted).
fn debug_draw_avoidance(
    units: Query<(&GlobalTransform, &LinearVelocity, &PreferredVelocity), With<Unit>>,
    mut gizmos: Gizmos,
) {
    let scale = 0.5;
    for (transform, velocity, preferred) in &units {
        let pos = transform.translation().xy();
        if preferred.0.length_squared() > f32::EPSILON {
            gizmos.arrow_2d(pos, pos + preferred.0 * scale, Color::srgb(0.0, 1.0, 0.0));
        }
        if velocity.0.length_squared() > f32::EPSILON {
            gizmos.arrow_2d(pos, pos + velocity.0 * scale, Color::srgb(0.0, 1.0, 1.0));
        }
    }
}
```

This requires adding `pub fn height(&self) -> u32` and `pub fn width(&self) -> u32` and `pub fn direction_at_cell(&self, col: u32, row: u32) -> Vec2` accessors to `FlowField`.

#### 6. Update movement tests

The existing tests in `movement.rs` reference `NavPath`. Rewrite them to test the flow field-based movement:

- `unit_sets_velocity_toward_target` → unit with Engaging target gets velocity toward it
- `unit_stops_at_attack_range` → Engaging unit in range gets zero velocity
- `unit_zero_velocity_without_target` → Seeking unit with no GoalRegistry gets zero velocity
- `unit_follows_flow_field_when_seeking` → Seeking unit with GoalRegistry follows flow field direction
- `unit_stops_when_attacking` → Attacking state gets zero velocity
- `unit_steers_direct_when_engaging` → Engaging unit steers toward target, not flow field

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — rewritten movement tests pass

#### Manual Verification:
- [ ] Units visually flow toward enemy fortress when no target
- [ ] Units correctly engage and steer toward enemies
- [ ] F3 toggle shows flow field arrows
- [ ] Units route around buildings (place a building, watch flow)

**Implementation Note**: Pause for manual verification before proceeding to Phase 4.

---

## Phase 4: Remove Navmesh

### Overview

Delete all navmesh infrastructure now that flow fields are verified working.

### Changes Required

#### 1. Delete `gameplay/units/pathfinding.rs`

Entire file removed.

#### 2. Delete `third_party/vleue_navigator.rs`

Entire file removed.

#### 3. `third_party/mod.rs` — Remove vleue_navigator

Remove:
- `mod vleue_navigator;` (line 4)
- `pub use self::vleue_navigator::NavObstacle;` (line 6)
- `vleue_navigator::plugin` from the plugin tuple (line 12)

#### 4. `gameplay/battlefield/renderer.rs` — Remove navmesh entity + NavObstacle

Remove:
- `use vleue_navigator::prelude::*;` (line 21)
- `use crate::third_party::{NavObstacle, solid_entity_layers};` → `use crate::third_party::solid_entity_layers;`
- `NavObstacle,` from player fortress spawn (line 98)
- `NavObstacle,` from enemy fortress spawn (line 181)
- The entire NavMesh entity spawn block (lines 207-224)

#### 5. `gameplay/building/placement.rs` — Remove NavObstacle

Remove:
- `NavObstacle` from the import: `use crate::third_party::{NavObstacle, solid_entity_layers};` → `use crate::third_party::solid_entity_layers;`
- `NavObstacle,` from the building spawn bundle (line 147)

#### 6. `gameplay/units/spawn.rs` — Remove navmesh from enemy spawner

Remove:
- `use vleue_navigator::prelude::*;` (line 4)
- `navmeshes` and `navmesh_query` parameters from `tick_enemy_spawner` (lines 84-85)
- The `navmesh` extraction block (lines 98-102)
- Change `random_navigable_spawn` call to pass `None` for navmesh (line 104): `super::random_navigable_spawn(fortress_pos.xy(), FORTRESS_SPAWN_RADIUS, None)`

#### 7. `gameplay/units/mod.rs` — Remove navmesh from `random_navigable_spawn`

Change function signature: remove `navmesh: Option<&NavMesh>` parameter, remove the navmesh validation inside. Since there's no navmesh anymore, always accept the random point:

```rust
pub fn random_navigable_spawn(center: Vec2, radius: f32) -> Vec2 {
    use rand::Rng;
    let mut rng = rand::rng();
    let angle = rng.random_range(0.0..std::f32::consts::TAU);
    Vec2::new(
        radius.mul_add(angle.cos(), center.x),
        radius.mul_add(angle.sin(), center.y),
    )
}
```

Later, we could validate against the flow field's blocked cells, but that's an optimization for later.

#### 8. `Cargo.toml` — Remove dependencies

- Remove `vleue_navigator = { ... }` from `[dependencies]` (line 14)
- Remove `"vleue_navigator/debug-with-gizmos"` from `dev` features (line 9)
- Remove `polyanya = "0.16"` from `[dev-dependencies]` (line 20)

#### 9. Update test that checks `random_navigable_spawn`

In `gameplay/units/mod.rs` tests, update `random_navigable_spawn_correct_distance_without_navmesh` to call without the navmesh parameter.

### Success Criteria

#### Automated Verification:
- [ ] `make check` passes (no unused imports, no missing modules)
- [ ] `make test` passes — all tests work without navmesh
- [ ] `cargo build` succeeds without `vleue_navigator` in dependency tree

#### Manual Verification:
- [ ] Game runs identically to Phase 3 (flow fields already active)
- [ ] No compile warnings about dead code from navmesh removal
- [ ] Enemy spawner still works (spawns near fortress without navmesh validation)

---

## Testing Strategy

### Unit Tests (Phase 1)
- `FlowField::new_battlefield()` — correct dimensions
- `world_to_cell` / `cell_to_world` — coordinate conversion
- `set_blocked` / `set_adjacent` / `reset_costs` — cost manipulation
- `compute` — simple rightward flow, routing around obstacles, corner-cutting prevention
- `direction_at` — correct lookup, out-of-bounds returns zero

### Integration Tests (Phase 2-3)
- Rewritten movement tests: flow field following, direct steering, attack range stop
- Existing AI, combat, death tests should pass unchanged (they don't reference pathfinding)

### Tests Removed (Phase 4)
- All `pathfinding.rs` tests (NavPath, snap_to_mesh, etc.) — deleted with the file
- Movement tests that referenced NavPath — rewritten in Phase 3

## Performance Considerations

- Flow field grid: 820 cells (82×10), ~23KB including directions + costs — fits in L1/L2 cache
- Dijkstra computation: O(grid_size × log(grid_size)) ≈ O(820 × 10) — ~microseconds, only on topology change
- Per-unit movement: O(1) flow field lookup (cell index + array read) vs O(n) A* calls
- Adjacent-cost ring: 3.0x penalty adds ~8 cells per building — negligible recomputation cost

## Verified API Patterns (Bevy 0.18)

These were verified against the actual codebase:

- `Single<&Transform, With<PlayerFortress>>` — system param for exactly-one-entity queries
- `Option<Res<T>>` — system param for optional resources
- `commands.insert_resource(T)` / `commands.remove_resource::<T>()` — marker resource toggle pattern
- `DespawnOnExit(GameState::InGame)` — state-scoped cleanup, in prelude
- `GameSet::Ai` / `GameSet::Movement` — existing system ordering sets
- `resource_exists::<T>` — run condition for debug toggle
- `BattlefieldSetup` system set — ordering anchor for `OnEnter` systems
- `gameplay_running` — run condition checking `InGame` + `Menu::None`

## References

- Linear ticket: [GAM-60](https://linear.app/tayhu-games/issue/GAM-60/flow-field-infrastructure-remove-navmesh)
- Research: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md` (Section 4.2, Section 9 Ticket 3)
- Blocked by: [GAM-59](https://linear.app/tayhu-games/issue/GAM-59) (EntityExtent) — DONE
- Blocks: [GAM-61](https://linear.app/tayhu-games/issue/GAM-61) (Separation force + remove ORCA)
- Supersedes: [GAM-43](https://linear.app/tayhu-games/issue/GAM-43) (stagger navmesh paths), [GAM-55](https://linear.app/tayhu-games/issue/GAM-55) (navmesh edge gap)

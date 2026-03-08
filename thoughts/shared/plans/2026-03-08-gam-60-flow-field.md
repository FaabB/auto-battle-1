# Flow Field Infrastructure + Remove Navmesh (GAM-60)

## Overview

Replace per-unit navmesh pathfinding with a shared flow field system. Each team gets a precomputed direction grid (Dijkstra on 328×40 cells at 16px resolution) pointing toward their goal fortress. Units read their team's flow field for O(1) movement direction instead of individual A* paths. Remove `vleue_navigator` dependency and all navmesh code.

## Current State Analysis

Units follow navmesh waypoints via `compute_paths` → `NavPath` → `unit_movement` → `PreferredVelocity` → ORCA → `LinearVelocity`. Buildings are marked `NavObstacle` which triggers async navmesh rebuilds. The pipeline works but scales poorly: every unit runs individual A* every 0.5s.

### Key Discoveries:
- Movement chain: `unit_movement` → `rebuild_spatial_hash` → `compute_avoidance` (chained in `GameSet::Movement`, `units/mod.rs:240-245`)
- `PreferredVelocity` written by `unit_movement`, read by ORCA — must survive this ticket (GAM-61 removes ORCA)
- `NavObstacle` added to buildings (`placement.rs:147`) and fortresses (`renderer.rs`)
- `NavmeshUpdaterPlugin::<Collider, NavObstacle>` auto-rebuilds navmesh on obstacle change (`vleue_navigator.rs:32`)
- Dev tools tie to `NavMeshesDebug` resource for F3 toggle (`dev_tools/mod.rs:36`)
- `pathfinding.rs` tests use `polyanya::Trimesh` — those tests get deleted with the file
- `testing.rs` spawns units with `NavPath::default()` and `AvoidanceAgent::default()` (`testing.rs:181-186`)
- Battlefield: 82×10 grid at 64px cells. Flow field uses 16px cells → 328×40 = 13,120 cells
- `TargetingState::Moving` exists but is currently unused — units start as `Seeking`

## Desired End State

- `FlowField` struct with Dijkstra computation, 16px cells, 328×40 grid
- `GoalRegistry` resource holding per-team flow fields
- `AssignedGoal` component on units (defaults to enemy fortress)
- `FlowFieldDirty` resource flag, set on building placement/destruction
- `recompute_flow_fields` system in `GameSet::Ai`
- `unit_movement` rewritten: `Moving`/`Seeking` follow flow field, `Engaging` steers direct
- Unit ejection on building placement
- Dev tool: F3 cycles flow field arrow overlays per team
- `vleue_navigator` fully removed (crate, plugin, `NavObstacle`, `NavPath`, `PathRefreshTimer`, `compute_paths`, `snap_to_mesh`)

### Verification:
- `make check` passes (no navmesh references remain)
- `make test` passes (all updated tests green)
- Manual: units navigate around buildings using flow field, dev overlay shows arrows

## What We're NOT Doing

- Removing ORCA/avoidance (GAM-61)
- Removing avian2d physics (GAM-62)
- Performance tuning or profiling (GAM-63)
- Multiple goal types (only fortress goals for now)
- Influence maps or strategic AI

## Implementation Approach

De-risked 4-phase approach: build flow field infrastructure first (pure data, fully testable), then integrate with Bevy (resources, systems), then rewrite movement to consume flow fields, and finally delete navmesh code. Each phase is independently verifiable.

## Phase 1: FlowField Struct + Math

### Overview
Pure data structures and algorithms, no Bevy integration. Fully unit-testable.

### Changes Required:

#### 1. New module: `gameplay/flow_field.rs`

**File**: `src/gameplay/flow_field.rs` (new)

```rust
use std::collections::BinaryHeap;
use std::cmp::Ordering;

use bevy::prelude::*;

use crate::gameplay::battlefield::{
    BATTLEFIELD_HEIGHT, BATTLEFIELD_WIDTH, CELL_SIZE, TOTAL_COLS, BATTLEFIELD_ROWS,
};
use crate::gameplay::units::UNIT_RADIUS;

// === Flow Field Constants ===

/// Flow field cell size in pixels. Finer than battlefield grid (64px) for smoother pathing.
pub const FLOW_CELL_SIZE: f32 = 16.0;

/// Flow field grid width in cells.
pub const FLOW_COLS: u32 = (BATTLEFIELD_WIDTH / FLOW_CELL_SIZE) as u32; // 328

/// Flow field grid height in cells.
pub const FLOW_ROWS: u32 = (BATTLEFIELD_HEIGHT / FLOW_CELL_SIZE) as u32; // 40

/// Total cells in the flow field grid.
pub const FLOW_CELL_COUNT: usize = (FLOW_COLS * FLOW_ROWS) as usize; // 13,120

// === Cost Tiers ===

/// Open terrain cost.
pub const COST_OPEN: f32 = 1.0;

/// Adjacent-to-building cost — soft nudge away from building edges.
pub const COST_ADJACENT: f32 = 3.0;

/// Blocked cell cost (building interior + unit-radius inflation).
pub const COST_BLOCKED: f32 = f32::INFINITY;

/// Inflation radius around buildings (unit radius) to prevent corner clipping.
pub const INFLATION_RADIUS: f32 = UNIT_RADIUS; // 6.0

// === 8-Connected Neighbors ===

/// (dcol, drow) offsets for 8-connected grid traversal.
const NEIGHBORS_8: [(i32, i32); 8] = [
    (-1, -1), (0, -1), (1, -1),
    (-1,  0),          (1,  0),
    (-1,  1), (0,  1), (1,  1),
];

/// Diagonal movement cost multiplier.
const DIAGONAL_COST: f32 = std::f32::consts::SQRT_2;

// === CostGrid ===

/// Cost grid: per-cell traversal cost. Infinity = blocked.
#[derive(Debug, Clone)]
pub struct CostGrid {
    pub costs: Vec<f32>,
    pub cols: u32,
    pub rows: u32,
}

impl CostGrid {
    /// Create a new cost grid with all cells set to `COST_OPEN`.
    #[must_use]
    pub fn new(cols: u32, rows: u32) -> Self {
        Self {
            costs: vec![COST_OPEN; (cols * rows) as usize],
            cols,
            rows,
        }
    }

    /// Index into the flat array.
    #[must_use]
    #[inline]
    pub fn index(&self, col: u32, row: u32) -> usize {
        (row * self.cols + col) as usize
    }

    /// Get cost at (col, row).
    #[must_use]
    #[inline]
    pub fn get(&self, col: u32, row: u32) -> f32 {
        self.costs[self.index(col, row)]
    }

    /// Set cost at (col, row).
    #[inline]
    pub fn set(&mut self, col: u32, row: u32, cost: f32) {
        let idx = self.index(col, row);
        self.costs[idx] = cost;
    }

    /// Is cell in bounds?
    #[must_use]
    #[inline]
    pub fn in_bounds(&self, col: i32, row: i32) -> bool {
        col >= 0 && row >= 0 && (col as u32) < self.cols && (row as u32) < self.rows
    }

    /// Mark a rectangular region as blocked, with inflation for unit radius.
    /// `world_min` and `world_max` are the building's AABB corners in world space.
    /// Also marks adjacent cells (within one flow cell of the inflated region) as COST_ADJACENT.
    pub fn mark_building(&mut self, world_min: Vec2, world_max: Vec2) {
        // Inflate by unit radius to prevent corner clipping
        let inflated_min = world_min - Vec2::splat(INFLATION_RADIUS);
        let inflated_max = world_max + Vec2::splat(INFLATION_RADIUS);

        let col_min = (inflated_min.x / FLOW_CELL_SIZE).floor().max(0.0) as u32;
        let col_max = ((inflated_max.x / FLOW_CELL_SIZE).ceil() as u32).min(self.cols);
        let row_min = (inflated_min.y / FLOW_CELL_SIZE).floor().max(0.0) as u32;
        let row_max = ((inflated_max.y / FLOW_CELL_SIZE).ceil() as u32).min(self.rows);

        // Mark blocked cells
        for row in row_min..row_max {
            for col in col_min..col_max {
                self.set(col, row, COST_BLOCKED);
            }
        }

        // Mark adjacent ring as high-cost (one cell border around blocked region)
        let adj_col_min = col_min.saturating_sub(1);
        let adj_col_max = (col_max + 1).min(self.cols);
        let adj_row_min = row_min.saturating_sub(1);
        let adj_row_max = (row_max + 1).min(self.rows);

        for row in adj_row_min..adj_row_max {
            for col in adj_col_min..adj_col_max {
                let idx = self.index(col, row);
                // Only upgrade open cells to adjacent, don't downgrade blocked
                if self.costs[idx] < COST_ADJACENT {
                    self.costs[idx] = COST_ADJACENT;
                }
            }
        }
    }

    /// Reset all cells to COST_OPEN.
    pub fn clear(&mut self) {
        self.costs.fill(COST_OPEN);
    }
}

// === FlowField ===

/// Precomputed direction grid. Each cell holds a normalized Vec2 pointing
/// toward the lowest-cost neighbor (toward the goal).
/// Unreachable cells hold Vec2::ZERO.
#[derive(Debug, Clone)]
pub struct FlowField {
    pub directions: Vec<Vec2>,
    pub cols: u32,
    pub rows: u32,
    pub cell_size: f32,
}

impl FlowField {
    /// Create an empty flow field (all zero directions).
    #[must_use]
    pub fn new(cols: u32, rows: u32, cell_size: f32) -> Self {
        Self {
            directions: vec![Vec2::ZERO; (cols * rows) as usize],
            cols,
            rows,
            cell_size,
        }
    }

    /// Index into the flat array.
    #[must_use]
    #[inline]
    pub fn index(&self, col: u32, row: u32) -> usize {
        (row * self.cols + col) as usize
    }

    /// Convert world position to flow field cell coordinates.
    #[must_use]
    #[inline]
    pub fn world_to_cell(&self, pos: Vec2) -> (i32, i32) {
        (
            (pos.x / self.cell_size).floor() as i32,
            (pos.y / self.cell_size).floor() as i32,
        )
    }

    /// Get the direction at a world position.
    /// Returns Vec2::ZERO if out of bounds.
    #[must_use]
    pub fn direction_at(&self, pos: Vec2) -> Vec2 {
        let (col, row) = self.world_to_cell(pos);
        if col < 0 || row < 0 || col as u32 >= self.cols || row as u32 >= self.rows {
            return Vec2::ZERO;
        }
        self.directions[self.index(col as u32, row as u32)]
    }

    /// Check if a cell is blocked (has zero direction and is not a goal).
    #[must_use]
    pub fn is_blocked_at(&self, pos: Vec2) -> bool {
        let (col, row) = self.world_to_cell(pos);
        if col < 0 || row < 0 || col as u32 >= self.cols || row as u32 >= self.rows {
            return true;
        }
        self.directions[self.index(col as u32, row as u32)] == Vec2::ZERO
    }
}

// === Dijkstra Algorithm ===

/// Trait for flow field computation algorithms.
/// Implement this to swap between Dijkstra, A*, or custom strategies.
pub trait FlowFieldAlgorithm: Send + Sync {
    /// Compute flow field directions from the cost grid and goal cells.
    /// Goal cells are specified as (col, row) pairs.
    fn compute(&self, cost_grid: &CostGrid, goals: &[(u32, u32)]) -> FlowField;
}

/// Standard Dijkstra flow field computation.
#[derive(Debug, Default)]
pub struct DijkstraFlowField;

/// Priority queue entry for Dijkstra.
#[derive(Debug)]
struct DijkstraEntry {
    cost: f32,
    col: u32,
    row: u32,
}

impl PartialEq for DijkstraEntry {
    fn eq(&self, other: &Self) -> bool {
        self.cost.to_bits() == other.cost.to_bits()
    }
}

impl Eq for DijkstraEntry {}

impl PartialOrd for DijkstraEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DijkstraEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min-heap (BinaryHeap is a max-heap)
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(Ordering::Equal)
    }
}

impl FlowFieldAlgorithm for DijkstraFlowField {
    fn compute(&self, cost_grid: &CostGrid, goals: &[(u32, u32)]) -> FlowField {
        let cols = cost_grid.cols;
        let rows = cost_grid.rows;
        let total = (cols * rows) as usize;

        // Integration field: best known cost to reach each cell from a goal
        let mut integration = vec![f32::INFINITY; total];
        let mut flow = FlowField::new(cols, rows, FLOW_CELL_SIZE);

        let mut heap = BinaryHeap::new();

        // Seed goals with zero cost
        for &(gc, gr) in goals {
            let idx = (gr * cols + gc) as usize;
            integration[idx] = 0.0;
            heap.push(DijkstraEntry {
                cost: 0.0,
                col: gc,
                row: gr,
            });
        }

        // Dijkstra expansion
        while let Some(current) = heap.pop() {
            let idx = (current.row * cols + current.col) as usize;
            if current.cost > integration[idx] {
                continue; // Stale entry
            }

            for &(dc, dr) in &NEIGHBORS_8 {
                let nc = current.col as i32 + dc;
                let nr = current.row as i32 + dr;

                if !cost_grid.in_bounds(nc, nr) {
                    continue;
                }

                let nc = nc as u32;
                let nr = nr as u32;
                let n_idx = (nr * cols + nc) as usize;

                let cell_cost = cost_grid.costs[n_idx];
                if cell_cost.is_infinite() {
                    continue; // Blocked
                }

                // Corner-cutting prevention: for diagonal moves, both adjacent
                // cardinal cells must be passable
                let is_diagonal = dc != 0 && dr != 0;
                if is_diagonal {
                    let adj1_cost = cost_grid.get(current.col, nr);
                    let adj2_cost = cost_grid.get(nc, current.row);
                    if adj1_cost.is_infinite() || adj2_cost.is_infinite() {
                        continue;
                    }
                }

                let move_cost = if is_diagonal {
                    cell_cost * DIAGONAL_COST
                } else {
                    cell_cost
                };

                let new_cost = current.cost + move_cost;
                if new_cost < integration[n_idx] {
                    integration[n_idx] = new_cost;
                    heap.push(DijkstraEntry {
                        cost: new_cost,
                        col: nc,
                        row: nr,
                    });
                }
            }
        }

        // Build direction field: each cell points toward its lowest-cost neighbor
        for row in 0..rows {
            for col in 0..cols {
                let idx = (row * cols + col) as usize;
                if integration[idx].is_infinite() {
                    continue; // Unreachable — stays Vec2::ZERO
                }
                if integration[idx] == 0.0 {
                    continue; // Goal cell — stays Vec2::ZERO (arrived)
                }

                let mut best_cost = integration[idx];
                let mut best_dir = Vec2::ZERO;

                for &(dc, dr) in &NEIGHBORS_8 {
                    let nc = col as i32 + dc;
                    let nr = row as i32 + dr;

                    if !cost_grid.in_bounds(nc, nr) {
                        continue;
                    }

                    // Corner-cutting prevention in direction selection too
                    let is_diagonal = dc != 0 && dr != 0;
                    if is_diagonal {
                        let adj1_cost = cost_grid.get(col, nr as u32);
                        let adj2_cost = cost_grid.get(nc as u32, row);
                        if adj1_cost.is_infinite() || adj2_cost.is_infinite() {
                            continue;
                        }
                    }

                    let n_idx = (nr as u32 * cols + nc as u32) as usize;
                    if integration[n_idx] < best_cost {
                        best_cost = integration[n_idx];
                        best_dir = Vec2::new(dc as f32, dr as f32);
                    }
                }

                flow.directions[idx] = best_dir.normalize_or_zero();
            }
        }

        flow
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] Unit tests for `CostGrid`: creation, mark_building, inflation, clear
- [ ] Unit tests for `FlowField`: world_to_cell, direction_at, out-of-bounds
- [ ] Unit tests for `DijkstraFlowField`: simple grid, blocked cells, diagonal prevention, disconnected regions return ZERO
- [ ] Tests verify 328×40 grid dimensions from battlefield constants

#### Manual Verification:
- [ ] N/A — pure data structures, no visual output yet

**Implementation Note**: After completing this phase and all automated verification passes, pause here for confirmation before proceeding to Phase 2.

---

## Phase 2: Bevy Integration — GoalRegistry, Dirty Flag, Building Hook

### Overview
Wire flow fields into Bevy's ECS: resources, components, systems for recomputation on building change, and unit ejection.

### Changes Required:

#### 1. Add Bevy types to `gameplay/flow_field.rs`

Append to the existing file:

```rust
use crate::gameplay::{Team, Movement};
use crate::gameplay::battlefield::{
    PlayerFortress, EnemyFortress,
    FORTRESS_COLS, FORTRESS_ROWS,
    PLAYER_FORT_START_COL, ENEMY_FORT_START_COL,
    col_to_world_x, row_to_world_y,
};

// === Goal System ===

/// Identifies which goal a unit is marching toward.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub enum AssignedGoal {
    /// Head toward the enemy fortress (default for player units).
    EnemyFortress,
    /// Head toward the player fortress (default for enemy units).
    PlayerFortress,
}

/// Per-team flow field data.
#[derive(Debug)]
pub struct GoalFlowField {
    pub flow_field: FlowField,
    pub goal_cells: Vec<(u32, u32)>,
}

/// Resource holding all active flow fields, keyed by goal.
#[derive(Resource, Debug)]
pub struct GoalRegistry {
    pub player_fortress: GoalFlowField,
    pub enemy_fortress: GoalFlowField,
    pub cost_grid: CostGrid,
    pub algorithm: Box<dyn FlowFieldAlgorithm>,
}

/// Resource flag: set to true when building topology changes.
/// Flow field recomputation reads and clears this flag.
#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct FlowFieldDirty(pub bool);
```

#### 2. Goal cell computation

```rust
/// Compute the flow field cells that a fortress occupies.
/// Fortress is FORTRESS_COLS × FORTRESS_ROWS at 64px cells.
/// Flow field cells are 16px, so each 64px cell = 4×4 flow cells.
fn fortress_goal_cells(start_col: u16, start_row: u16) -> Vec<(u32, u32)> {
    let cells_per_bf_cell = (CELL_SIZE / FLOW_CELL_SIZE) as u32; // 4
    let mut cells = Vec::new();
    for br in 0..FORTRESS_ROWS {
        for bc in 0..FORTRESS_COLS {
            let base_fc = u32::from(start_col + bc) * cells_per_bf_cell;
            let base_fr = u32::from(start_row + br) * cells_per_bf_cell;
            for fr in base_fr..base_fr + cells_per_bf_cell {
                for fc in base_fc..base_fc + cells_per_bf_cell {
                    cells.push((fc, fr));
                }
            }
        }
    }
    cells
}
```

The player fortress occupies columns 0–1, rows 4–5 (centered vertically). The enemy fortress occupies columns 80–81, rows 4–5. These are computed from the fortress `Transform` positions at runtime during `setup_flow_fields`.

#### 3. Systems

```rust
/// Initialize flow fields on entering InGame.
/// Reads fortress positions to determine goal cells.
fn setup_flow_fields(
    mut commands: Commands,
    player_fort: Single<&Transform, With<PlayerFortress>>,
    enemy_fort: Single<&Transform, With<EnemyFortress>>,
) {
    let cost_grid = CostGrid::new(FLOW_COLS, FLOW_ROWS);
    let algorithm = Box::new(DijkstraFlowField);

    // Compute goal cells from fortress world positions
    let pf_pos = player_fort.translation.xy();
    let ef_pos = enemy_fort.translation.xy();

    // Convert fortress center to top-left grid cell, then to flow cells
    let pf_cells = fortress_cells_from_world(pf_pos);
    let ef_cells = fortress_cells_from_world(ef_pos);

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

/// Recompute flow fields when dirty flag is set.
fn recompute_flow_fields(mut registry: ResMut<GoalRegistry>, mut dirty: ResMut<FlowFieldDirty>) {
    if !dirty.0 {
        return;
    }
    dirty.0 = false;

    let pf_goals = registry.player_fortress.goal_cells.clone();
    let ef_goals = registry.enemy_fortress.goal_cells.clone();

    registry.player_fortress.flow_field =
        registry.algorithm.compute(&registry.cost_grid, &pf_goals);
    registry.enemy_fortress.flow_field =
        registry.algorithm.compute(&registry.cost_grid, &ef_goals);
}

/// Mark flow field dirty and update cost grid when a building is placed.
/// Called from building placement after spawning the building entity.
pub fn mark_building_placed(
    registry: &mut GoalRegistry,
    dirty: &mut FlowFieldDirty,
    world_min: Vec2,
    world_max: Vec2,
) {
    registry.cost_grid.mark_building(world_min, world_max);
    dirty.0 = true;
}

/// Clear a building's cells from the cost grid when it's destroyed.
/// Since we can't easily "unmark" individual buildings, rebuild the entire
/// cost grid from scratch on building removal.
pub fn rebuild_cost_grid_from_buildings(
    registry: &mut GoalRegistry,
    dirty: &mut FlowFieldDirty,
    buildings: &[(Vec2, Vec2)], // (world_min, world_max) for each remaining building + fortress
) {
    registry.cost_grid.clear();
    for &(wmin, wmax) in buildings {
        registry.cost_grid.mark_building(wmin, wmax);
    }
    dirty.0 = true;
}
```

#### 4. Unit ejection on building placement

```rust
/// Eject units from cells that became blocked after building placement.
/// Teleports each affected unit to the nearest unblocked cell center.
pub fn eject_units_from_blocked_cells(
    mut units: Query<&mut Transform, With<Unit>>,
    registry: &GoalRegistry,
    building_min: Vec2,
    building_max: Vec2,
) {
    let inflated_min = building_min - Vec2::splat(INFLATION_RADIUS);
    let inflated_max = building_max + Vec2::splat(INFLATION_RADIUS);

    for mut transform in &mut units {
        let pos = transform.translation.xy();
        if pos.x >= inflated_min.x
            && pos.x <= inflated_max.x
            && pos.y >= inflated_min.y
            && pos.y <= inflated_max.y
        {
            // Find nearest unblocked cell center by spiraling outward
            if let Some(safe_pos) = find_nearest_unblocked(
                pos,
                &registry.cost_grid,
            ) {
                transform.translation.x = safe_pos.x;
                transform.translation.y = safe_pos.y;
            }
        }
    }
}

/// Find the nearest unblocked cell center to `pos`.
fn find_nearest_unblocked(pos: Vec2, cost_grid: &CostGrid) -> Option<Vec2> {
    let col = (pos.x / FLOW_CELL_SIZE).floor() as i32;
    let row = (pos.y / FLOW_CELL_SIZE).floor() as i32;

    // Spiral outward up to 10 cells
    for radius in 0..10 {
        for dr in -radius..=radius {
            for dc in -radius..=radius {
                if dr.abs() != radius && dc.abs() != radius {
                    continue; // Only check the ring perimeter
                }
                let nc = col + dc;
                let nr = row + dr;
                if cost_grid.in_bounds(nc, nr) && cost_grid.get(nc as u32, nr as u32) < COST_BLOCKED {
                    let center_x = (nc as f32 + 0.5) * FLOW_CELL_SIZE;
                    let center_y = (nr as f32 + 0.5) * FLOW_CELL_SIZE;
                    return Some(Vec2::new(center_x, center_y));
                }
            }
        }
    }
    None
}
```

#### 5. Hook into building placement (`building/placement.rs`)

After the building is spawned, call `mark_building_placed` and `eject_units_from_blocked_cells`. Replace `NavObstacle` insertion with flow field cost update.

#### 6. Hook into building destruction

On `On<Remove, Building>` observer, rebuild the cost grid from remaining buildings and fortresses.

#### 7. Plugin registration (`gameplay/flow_field.rs`)

```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<AssignedGoal>()
        .register_type::<FlowFieldDirty>();

    app.add_systems(
        OnEnter(GameState::InGame),
        setup_flow_fields.after(crate::gameplay::battlefield::BattlefieldSetup),
    );

    app.add_systems(
        Update,
        recompute_flow_fields
            .in_set(GameSet::Ai)
            .run_if(crate::gameplay_running),
    );
}
```

#### 8. Register in `gameplay/mod.rs`

Add `pub mod flow_field;` and `flow_field::plugin` to the plugin tuple.

#### 9. Add `AssignedGoal` to unit spawn

In `units/mod.rs` `spawn_unit()`, add `AssignedGoal` based on team:
- `Team::Player` → `AssignedGoal::EnemyFortress`
- `Team::Enemy` → `AssignedGoal::PlayerFortress`

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes
- [ ] Integration test: `GoalRegistry` resource exists after entering InGame
- [ ] Integration test: `FlowFieldDirty` starts as false
- [ ] Unit test: `fortress_goal_cells` returns correct number of cells
- [ ] Unit test: `mark_building_placed` sets dirty flag
- [ ] Unit test: `find_nearest_unblocked` finds correct cell
- [ ] Unit test: `eject_units_from_blocked_cells` moves units outside building

#### Manual Verification:
- [ ] Game still runs (flow fields computed, units still use navmesh movement)
- [ ] No performance regression on startup

**Implementation Note**: After completing this phase and all automated verification passes, pause here for confirmation before proceeding to Phase 3.

---

## Phase 3: Rewrite Movement + Dev Tools

### Overview
Replace navmesh waypoint following with flow field direction reading. Update dev tools from navmesh overlay to flow field arrow overlay.

### Changes Required:

#### 1. Rewrite `movement.rs`

**File**: `src/gameplay/units/movement.rs`

The new `unit_movement` system reads flow fields instead of `NavPath`:

```rust
use crate::gameplay::flow_field::{AssignedGoal, GoalRegistry};

pub(super) fn unit_movement(
    mut units: Query<(
        &TargetingState,
        &Movement,
        &CombatStats,
        &GlobalTransform,
        &EntityExtent,
        &AssignedGoal,
        &mut PreferredVelocity,
    ), With<Unit>>,
    targets: Query<(&GlobalTransform, &EntityExtent)>,
    registry: Option<Res<GoalRegistry>>,
) {
    let Some(registry) = registry else { return };

    for (targeting_state, movement, stats, global_transform, unit_extent, goal, mut preferred) in &mut units {
        let current_xy = global_transform.translation().xy();

        match *targeting_state {
            TargetingState::Moving | TargetingState::Seeking => {
                // Follow flow field
                let flow_field = match goal {
                    AssignedGoal::EnemyFortress => &registry.enemy_fortress.flow_field,
                    AssignedGoal::PlayerFortress => &registry.player_fortress.flow_field,
                };
                let direction = flow_field.direction_at(current_xy);
                preferred.0 = direction * movement.speed;
            }
            TargetingState::Engaging(target_entity) => {
                // Steer directly toward target
                let Ok((target_pos, target_extent)) = targets.get(target_entity) else {
                    preferred.0 = Vec2::ZERO;
                    continue;
                };
                let target_xy = target_pos.translation().xy();
                let distance = extent_distance(unit_extent, current_xy, target_extent, target_xy);

                if distance <= stats.range {
                    preferred.0 = Vec2::ZERO;
                    continue;
                }

                let diff = target_xy - current_xy;
                let dist = diff.length();
                if dist < f32::EPSILON {
                    preferred.0 = Vec2::ZERO;
                } else {
                    preferred.0 = (diff / dist) * movement.speed;
                }
            }
            TargetingState::Attacking(_) => {
                preferred.0 = Vec2::ZERO;
            }
        }
    }
}
```

#### 2. Remove `NavPath` from unit archetype

In `units/mod.rs` `spawn_unit()`:
- Remove `pathfinding::NavPath::default()` from the insert
- Add `AssignedGoal` based on team (if not already done in Phase 2)

In `units/mod.rs` plugin:
- Remove `compute_paths` from the system schedule
- Remove `PathRefreshTimer` init and reset

#### 3. Update `TargetingState` usage in `ai.rs`

Currently `find_target` transitions units to `Engaging(entity)` or `Seeking`. Units should now start as `Moving` (following flow field) and transition to `Seeking` when they detect nearby enemies, then `Engaging` when they lock onto one.

However, to minimize scope in this ticket: keep the existing `find_target` behavior. Units that have no target stay as `Seeking` (which follows the flow field in the new movement system — same as `Moving`). Units with a target become `Engaging`. The `Moving` state is used for units that haven't yet entered detection range — but since `find_target` already handles this transition, we can defer the `Moving` → `Seeking` distinction to a future refinement.

For now: `Seeking` and `Moving` both follow the flow field in the movement system.

#### 4. Rewrite dev tools

**File**: `src/dev_tools/mod.rs`

Replace navmesh debug with flow field overlay:

```rust
/// Marker resource: when present, flow field debug arrows are drawn.
#[derive(Resource, Debug)]
struct FlowFieldDebug {
    /// Which team's flow field to display.
    show_team: Team,
}

/// Toggle flow field debug overlay with F3. Cycle teams with repeated presses.
fn toggle_flow_field_debug(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    existing: Option<Res<FlowFieldDebug>>,
) {
    if input.just_pressed(KeyCode::F3) {
        if let Some(debug) = existing {
            match debug.show_team {
                Team::Player => {
                    commands.insert_resource(FlowFieldDebug { show_team: Team::Enemy });
                }
                Team::Enemy => {
                    commands.remove_resource::<FlowFieldDebug>();
                }
            }
        } else {
            commands.insert_resource(FlowFieldDebug { show_team: Team::Player });
        }
    }
}

/// Draw flow field direction arrows.
fn debug_draw_flow_field(
    debug: Res<FlowFieldDebug>,
    registry: Option<Res<GoalRegistry>>,
    mut gizmos: Gizmos,
) {
    let Some(registry) = registry else { return };

    let flow_field = match debug.show_team {
        // Player units go toward enemy fortress
        Team::Player => &registry.enemy_fortress.flow_field,
        // Enemy units go toward player fortress
        Team::Enemy => &registry.player_fortress.flow_field,
    };

    let color = match debug.show_team {
        Team::Player => Color::srgba(0.0, 0.5, 1.0, 0.6),
        Team::Enemy => Color::srgba(1.0, 0.3, 0.3, 0.6),
    };

    // Draw arrows at every 4th cell to avoid visual clutter (every 64px = battlefield grid)
    let step = 4u32;
    for row in (0..flow_field.rows).step_by(step as usize) {
        for col in (0..flow_field.cols).step_by(step as usize) {
            let idx = flow_field.index(col, row);
            let dir = flow_field.directions[idx];
            if dir == Vec2::ZERO {
                continue;
            }
            let center = Vec2::new(
                (col as f32 + 0.5) * flow_field.cell_size,
                (row as f32 + 0.5) * flow_field.cell_size,
            );
            gizmos.arrow_2d(center, center + dir * 20.0, color);
        }
    }
}
```

Remove `NavMeshesDebug` import, `toggle_navmesh_debug`, `debug_draw_unit_paths`, `debug_draw_avoidance` (avoidance debug stays until GAM-61 if desired, or remove now since it depends on `PreferredVelocity` which still exists). Keep avoidance debug for now — it's still useful.

Actually: keep `debug_draw_avoidance` (it shows `PreferredVelocity` vs `LinearVelocity`, still useful). Remove `debug_draw_unit_paths` (no more `NavPath`). Replace `toggle_navmesh_debug` + `NavMeshesDebug` with `toggle_flow_field_debug` + `FlowFieldDebug`.

#### 5. Update movement system chain in `units/mod.rs`

The chain `unit_movement → rebuild_spatial_hash → compute_avoidance` stays. But `unit_movement` no longer queries `NavPath`, and we remove `compute_paths` from `GameSet::Ai`.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes (movement tests rewritten for flow field)
- [ ] Movement tests: unit with `Moving`/`Seeking` gets flow field velocity
- [ ] Movement tests: unit with `Engaging` steers toward target
- [ ] Movement tests: unit with `Attacking` gets zero velocity
- [ ] Movement tests: unit in range of target gets zero velocity

#### Manual Verification:
- [ ] Units navigate around buildings using flow field directions
- [ ] Units converge on enemy fortress
- [ ] F3 shows flow field arrows (blue for player team, red for enemy team)
- [ ] F3 cycles: player → enemy → off
- [ ] Engaging units steer directly toward their target
- [ ] No units getting stuck in buildings

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual testing confirmation before proceeding to Phase 4.

---

## Phase 4: Remove Navmesh

### Overview
Delete all navmesh code now that flow fields are the sole movement system.

### Changes Required:

#### 1. Delete `pathfinding.rs`

**File**: `src/gameplay/units/pathfinding.rs` — DELETE entirely

#### 2. Delete `third_party/vleue_navigator.rs`

**File**: `src/third_party/vleue_navigator.rs` — DELETE entirely

#### 3. Update `third_party/mod.rs`

Remove `mod vleue_navigator;`, `pub use self::vleue_navigator::NavObstacle;`, and the plugin registration.

#### 4. Update `Cargo.toml`

Remove `vleue_navigator` from `[dependencies]`.
Remove `vleue_navigator/debug-with-gizmos` from `[features] dev`.
Remove `polyanya` from `[dev-dependencies]`.

#### 5. Remove `NavObstacle` from all spawn sites

- `building/placement.rs`: remove `NavObstacle` from building spawn
- `battlefield/renderer.rs`: remove `NavObstacle` from fortress spawn

#### 6. Update `units/mod.rs`

- Remove `pub mod pathfinding;` (or `mod pathfinding;`)
- Remove `use vleue_navigator::prelude::NavMesh;`
- Remove `pathfinding::NavPath` imports and registration
- Remove `pathfinding::PathRefreshTimer` init and registration
- Remove `reset_path_refresh_timer` system
- Remove `compute_paths` from system schedule
- Remove `NavPath::default()` from `spawn_unit` (if not already done)
- Remove `random_navigable_spawn`'s navmesh parameter (simplify to just random position — flow field handles routing)

#### 7. Update `units/spawn.rs`

Remove navmesh parameter from `random_navigable_spawn` calls.

#### 8. Update `testing.rs`

- Remove `use crate::gameplay::units::pathfinding::NavPath;`
- Remove `NavPath::default()` from `spawn_test_unit`

#### 9. Update `dev_tools/mod.rs`

- Remove `use vleue_navigator::prelude::NavMeshesDebug;`
- Remove `use crate::gameplay::units::pathfinding::NavPath;`
- Remove any remaining navmesh references

#### 10. Update integration tests

- `tests/integration/state_transitions.rs`: remove navmesh references if any

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes — no references to `vleue_navigator`, `NavPath`, `NavObstacle`, `PathRefreshTimer`, `compute_paths`, `snap_to_mesh`
- [ ] `make test` passes — all tests green
- [ ] `cargo tree` does not include `vleue_navigator` or `polyanya`

#### Manual Verification:
- [ ] Game runs identically to Phase 3 (flow field movement, dev overlay)
- [ ] Building placement updates flow field and units route around buildings
- [ ] Unit ejection works when building is placed on top of units

---

## Testing Strategy

### Unit Tests (Phase 1):
- `CostGrid::new` creates correct dimensions
- `CostGrid::mark_building` blocks correct cells and inflates adjacent
- `FlowField::world_to_cell` converts correctly
- `FlowField::direction_at` returns correct direction / ZERO for out-of-bounds
- `DijkstraFlowField::compute` on simple open grid → all cells point toward goal
- `DijkstraFlowField::compute` with blocked column → cells route around
- `DijkstraFlowField::compute` with disconnected region → unreachable cells have ZERO
- Corner-cutting prevention: diagonal blocked when adjacent cardinal blocked
- 328×40 grid from battlefield constants

### Integration Tests (Phase 2):
- `GoalRegistry` resource exists after InGame transition
- `FlowFieldDirty` starts false
- Building placement sets dirty flag
- Recompute system clears dirty flag and updates flow fields
- Unit ejection teleports units out of blocked cells

### Movement Tests (Phase 3):
- `Moving`/`Seeking` unit gets flow field direction × speed
- `Engaging` unit steers toward target
- `Attacking` unit gets zero velocity
- Unit in attack range gets zero velocity
- No target → zero velocity

### Manual Testing Steps:
1. Start game, verify units move toward enemy fortress
2. Place building in combat zone path — verify units route around it
3. Place building on top of units — verify they get ejected
4. Press F3 — verify flow field arrows appear (blue)
5. Press F3 again — verify enemy flow field arrows (red)
6. Press F3 again — verify arrows disappear
7. Destroy a building — verify flow field updates and units take new paths

## Performance Considerations

- 13,120 cells × 4 bytes = ~52KB per flow field direction grid — fits in L2 cache
- Dijkstra on 13,120 cells takes <1ms — only runs on building change, not per-frame
- Flow field lookup is O(1) per unit per frame (single array index)
- Two flow fields (one per team) = ~104KB total
- Cost grid rebuild is O(buildings), not O(cells) — building count is low (<20)

## References

- Linear ticket: [GAM-60](https://linear.app/tayhu-games/issue/GAM-60/flow-field-infrastructure-remove-navmesh)
- Research: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md` (Section 4.2, Section 9 Ticket 3)
- Depends on: [GAM-59](https://linear.app/tayhu-games/issue/GAM-59) (EntityExtent) — DONE
- Blocks: [GAM-61](https://linear.app/tayhu-games/issue/GAM-61/separation-force-remove-orca) (Separation force + remove ORCA)
- Supersedes: GAM-43 (stagger navmesh paths), GAM-55 (navmesh edge gap)

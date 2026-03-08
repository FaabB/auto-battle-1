//! Flow field pathfinding: shared direction grids computed via Dijkstra.
//!
//! Each team gets a precomputed flow field pointing toward their goal fortress.
//! Units read their team's flow field for O(1) movement direction instead of
//! individual A* paths.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use bevy::prelude::*;

use crate::GameSet;
use crate::gameplay::EntityExtent;
use crate::gameplay::battlefield::{
    BATTLEFIELD_HEIGHT, BATTLEFIELD_WIDTH, CELL_SIZE, FORTRESS_COLS, FORTRESS_ROWS,
};
use crate::gameplay::units::UNIT_RADIUS;
use crate::screens::GameState;

// === Flow Field Constants ===

/// Flow field cell size in pixels. Finer than battlefield grid (64px) for smoother pathing.
pub const FLOW_CELL_SIZE: f32 = 16.0;

/// Flow field grid width in cells.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub const FLOW_COLS: u32 = (BATTLEFIELD_WIDTH / FLOW_CELL_SIZE) as u32; // 328

/// Flow field grid height in cells.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub const FLOW_ROWS: u32 = (BATTLEFIELD_HEIGHT / FLOW_CELL_SIZE) as u32; // 40

/// Total cells in the flow field grid.
#[allow(dead_code)] // Used in tests
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
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
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

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
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
    pub const fn index(&self, col: u32, row: u32) -> usize {
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
    pub const fn in_bounds(&self, col: i32, row: i32) -> bool {
        col >= 0 && row >= 0 && (col as u32) < self.cols && (row as u32) < self.rows
    }

    /// Mark a rectangular region as blocked, with inflation for unit radius.
    /// `world_min` and `world_max` are the building's AABB corners in world space.
    /// Also marks adjacent cells (within one flow cell of the inflated region) as `COST_ADJACENT`.
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

    /// Reset all cells to `COST_OPEN`.
    pub fn clear(&mut self) {
        self.costs.fill(COST_OPEN);
    }
}

// === FlowField ===

/// Precomputed direction grid. Each cell holds a normalized `Vec2` pointing
/// toward the lowest-cost neighbor (toward the goal).
/// Unreachable cells hold `Vec2::ZERO`.
#[derive(Debug, Clone)]
pub struct FlowField {
    pub directions: Vec<Vec2>,
    pub cols: u32,
    pub rows: u32,
    pub cell_size: f32,
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
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
    pub const fn index(&self, col: u32, row: u32) -> usize {
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
    /// Returns `Vec2::ZERO` if out of bounds.
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
    #[allow(dead_code)] // Used in tests
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

impl std::fmt::Debug for dyn FlowFieldAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FlowFieldAlgorithm")
    }
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

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]
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
                // Goal cell — stays Vec2::ZERO (arrived)
                #[allow(clippy::float_cmp)]
                if integration[idx] == 0.0 {
                    continue;
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

/// Find the nearest unblocked cell center to `pos`.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn find_nearest_unblocked(pos: Vec2, cost_grid: &CostGrid) -> Option<Vec2> {
    let col = (pos.x / FLOW_CELL_SIZE).floor() as i32;
    let row = (pos.y / FLOW_CELL_SIZE).floor() as i32;

    // Spiral outward up to 10 cells
    for radius in 0i32..10 {
        for dr in -radius..=radius {
            for dc in -radius..=radius {
                if dr.abs() != radius && dc.abs() != radius {
                    continue; // Only check the ring perimeter
                }
                let nc = col + dc;
                let nr = row + dr;
                if cost_grid.in_bounds(nc, nr) && cost_grid.get(nc as u32, nr as u32) < COST_BLOCKED
                {
                    let center_x = (nc as f32 + 0.5) * FLOW_CELL_SIZE;
                    let center_y = (nr as f32 + 0.5) * FLOW_CELL_SIZE;
                    return Some(Vec2::new(center_x, center_y));
                }
            }
        }
    }
    None
}

// === Bevy Integration ===

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

// === Goal Cell Computation ===

/// Compute the flow field cells that a fortress occupies.
/// Fortress is `FORTRESS_COLS` x `FORTRESS_ROWS` at 64px cells.
/// Flow field cells are 16px, so each 64px cell = 4x4 flow cells.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn fortress_cells_from_world(center: Vec2) -> Vec<(u32, u32)> {
    let cells_per_bf_cell = (CELL_SIZE / FLOW_CELL_SIZE) as u32; // 4
    let half_w = f32::from(FORTRESS_COLS) * CELL_SIZE / 2.0;
    let half_h = f32::from(FORTRESS_ROWS) * CELL_SIZE / 2.0;
    let min = center - Vec2::new(half_w, half_h);

    let col_start = (min.x / FLOW_CELL_SIZE).floor().max(0.0) as u32;
    let row_start = (min.y / FLOW_CELL_SIZE).floor().max(0.0) as u32;
    let col_count = u32::from(FORTRESS_COLS) * cells_per_bf_cell;
    let row_count = u32::from(FORTRESS_ROWS) * cells_per_bf_cell;

    let mut cells = Vec::with_capacity((col_count * row_count) as usize);
    for row in row_start..row_start + row_count {
        for col in col_start..col_start + col_count {
            if col < FLOW_COLS && row < FLOW_ROWS {
                cells.push((col, row));
            }
        }
    }
    cells
}

// === Systems ===

/// Initialize flow fields on entering `InGame`.
/// Reads fortress positions to determine goal cells.
fn setup_flow_fields(
    mut commands: Commands,
    player_fort: Single<&Transform, With<crate::gameplay::battlefield::PlayerFortress>>,
    enemy_fort: Single<&Transform, With<crate::gameplay::battlefield::EnemyFortress>>,
) {
    let cost_grid = CostGrid::new(FLOW_COLS, FLOW_ROWS);
    let algorithm = Box::new(DijkstraFlowField);

    let pf_cells = fortress_cells_from_world(player_fort.translation.xy());
    let ef_cells = fortress_cells_from_world(enemy_fort.translation.xy());

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
    registry.enemy_fortress.flow_field = registry.algorithm.compute(&registry.cost_grid, &ef_goals);
}

/// Mark flow field dirty and update cost grid when a building is placed.
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

/// Eject units from cells that became blocked after building placement.
/// Teleports each affected unit to the nearest unblocked cell center.
pub fn eject_units_from_blocked<'a>(
    cost_grid: &CostGrid,
    building_min: Vec2,
    building_max: Vec2,
    unit_positions: impl Iterator<Item = Mut<'a, Transform>>,
) {
    let inflated_min = building_min - Vec2::splat(INFLATION_RADIUS);
    let inflated_max = building_max + Vec2::splat(INFLATION_RADIUS);

    for mut transform in unit_positions {
        let pos = transform.translation.xy();
        if pos.x >= inflated_min.x
            && pos.x <= inflated_max.x
            && pos.y >= inflated_min.y
            && pos.y <= inflated_max.y
        {
            if let Some(safe_pos) = find_nearest_unblocked(pos, cost_grid) {
                transform.translation.x = safe_pos.x;
                transform.translation.y = safe_pos.y;
            }
        }
    }
}

/// Rebuild cost grid when a building is destroyed.
/// Fires on `On<Remove, Building>` — collects all remaining buildings + fortresses
/// and rebuilds the cost grid from scratch.
fn on_building_removed(
    _trigger: On<Remove, crate::gameplay::building::Building>,
    mut registry: Option<ResMut<GoalRegistry>>,
    mut dirty: Option<ResMut<FlowFieldDirty>>,
    buildings: Query<(&Transform, &EntityExtent), With<crate::gameplay::building::Building>>,
    fortresses: Query<
        (&Transform, &EntityExtent),
        Or<(
            With<crate::gameplay::battlefield::PlayerFortress>,
            With<crate::gameplay::battlefield::EnemyFortress>,
        )>,
    >,
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

    // Collect fortresses
    for (transform, extent) in &fortresses {
        let pos = transform.translation.xy();
        if let EntityExtent::Rect(hw, hh) = extent {
            aabbs.push((pos - Vec2::new(*hw, *hh), pos + Vec2::new(*hw, *hh)));
        }
    }

    rebuild_cost_grid_from_buildings(registry, dirty, &aabbs);
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<AssignedGoal>()
        .register_type::<FlowFieldDirty>();

    app.add_observer(on_building_removed);

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

#[cfg(test)]
mod tests {
    use super::*;

    // === CostGrid Tests ===

    #[test]
    fn cost_grid_new_creates_correct_dimensions() {
        let grid = CostGrid::new(10, 5);
        assert_eq!(grid.cols, 10);
        assert_eq!(grid.rows, 5);
        assert_eq!(grid.costs.len(), 50);
        assert!(grid.costs.iter().all(|&c| c == COST_OPEN));
    }

    #[test]
    fn cost_grid_battlefield_dimensions() {
        let grid = CostGrid::new(FLOW_COLS, FLOW_ROWS);
        assert_eq!(FLOW_COLS, 328);
        assert_eq!(FLOW_ROWS, 40);
        assert_eq!(grid.costs.len(), FLOW_CELL_COUNT);
        assert_eq!(FLOW_CELL_COUNT, 13_120);
    }

    #[test]
    fn cost_grid_index_and_get_set() {
        let mut grid = CostGrid::new(4, 3);
        assert_eq!(grid.index(2, 1), 6); // row=1 * cols=4 + col=2
        grid.set(2, 1, 5.0);
        assert_eq!(grid.get(2, 1), 5.0);
        assert_eq!(grid.get(0, 0), COST_OPEN);
    }

    #[test]
    fn cost_grid_in_bounds() {
        let grid = CostGrid::new(4, 3);
        assert!(grid.in_bounds(0, 0));
        assert!(grid.in_bounds(3, 2));
        assert!(!grid.in_bounds(-1, 0));
        assert!(!grid.in_bounds(0, -1));
        assert!(!grid.in_bounds(4, 0));
        assert!(!grid.in_bounds(0, 3));
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    #[test]
    fn cost_grid_mark_building_blocks_cells() {
        let mut grid = CostGrid::new(10, 10);
        // A 32x32 building at world (32, 32) to (64, 64)
        grid.mark_building(Vec2::new(32.0, 32.0), Vec2::new(64.0, 64.0));

        // Inflated by INFLATION_RADIUS (6.0):
        // min = (32-6, 32-6) = (26, 26) → cell (1, 1)
        // max = (64+6, 64+6) = (70, 70) → cell ceil(70/16) = 5
        // So cells (1,1) to (4,4) should be blocked
        assert_eq!(grid.get(2, 2), COST_BLOCKED);
        assert_eq!(grid.get(3, 3), COST_BLOCKED);

        // Adjacent cells should have COST_ADJACENT
        let blocked_col_min = (26.0_f32 / 16.0).floor() as u32; // 1
        let blocked_col_max = (70.0_f32 / 16.0).ceil() as u32; // 5
        let blocked_row_min = (26.0_f32 / 16.0).floor() as u32; // 1
        let blocked_row_max = (70.0_f32 / 16.0).ceil() as u32; // 5

        // Cell (0, 0) should be adjacent (one cell before blocked region)
        assert_eq!(grid.get(0, 0), COST_ADJACENT);
        // Cell (blocked_col_max, blocked_row_max) should be adjacent
        assert_eq!(grid.get(blocked_col_max, blocked_row_max), COST_ADJACENT);

        // Cell far away should still be open
        assert_eq!(grid.get(9, 9), COST_OPEN);

        // Verify blocked region bounds
        assert_eq!(blocked_col_min, 1);
        assert_eq!(blocked_col_max, 5);
        assert_eq!(blocked_row_min, 1);
        assert_eq!(blocked_row_max, 5);
    }

    #[test]
    fn cost_grid_mark_building_doesnt_downgrade_blocked() {
        let mut grid = CostGrid::new(10, 10);
        // Mark two overlapping buildings
        grid.mark_building(Vec2::new(16.0, 16.0), Vec2::new(48.0, 48.0));
        grid.mark_building(Vec2::new(32.0, 32.0), Vec2::new(64.0, 64.0));

        // Overlapping blocked cells should still be blocked, not downgraded to adjacent
        assert_eq!(grid.get(2, 2), COST_BLOCKED);
    }

    #[test]
    fn cost_grid_clear_resets_all() {
        let mut grid = CostGrid::new(5, 5);
        grid.mark_building(Vec2::new(16.0, 16.0), Vec2::new(48.0, 48.0));
        grid.clear();
        assert!(grid.costs.iter().all(|&c| c == COST_OPEN));
    }

    // === FlowField Tests ===

    #[test]
    fn flow_field_new_creates_correct_dimensions() {
        let ff = FlowField::new(10, 5, 16.0);
        assert_eq!(ff.cols, 10);
        assert_eq!(ff.rows, 5);
        assert_eq!(ff.cell_size, 16.0);
        assert_eq!(ff.directions.len(), 50);
        assert!(ff.directions.iter().all(|&d| d == Vec2::ZERO));
    }

    #[test]
    fn flow_field_world_to_cell() {
        let ff = FlowField::new(10, 10, 16.0);
        assert_eq!(ff.world_to_cell(Vec2::new(0.0, 0.0)), (0, 0));
        assert_eq!(ff.world_to_cell(Vec2::new(15.9, 15.9)), (0, 0));
        assert_eq!(ff.world_to_cell(Vec2::new(16.0, 0.0)), (1, 0));
        assert_eq!(ff.world_to_cell(Vec2::new(32.5, 48.5)), (2, 3));
        // Negative positions
        assert_eq!(ff.world_to_cell(Vec2::new(-1.0, -1.0)), (-1, -1));
    }

    #[test]
    fn flow_field_direction_at_out_of_bounds() {
        let ff = FlowField::new(10, 10, 16.0);
        assert_eq!(ff.direction_at(Vec2::new(-5.0, 0.0)), Vec2::ZERO);
        assert_eq!(ff.direction_at(Vec2::new(200.0, 0.0)), Vec2::ZERO);
        assert_eq!(ff.direction_at(Vec2::new(0.0, 200.0)), Vec2::ZERO);
    }

    #[test]
    fn flow_field_direction_at_returns_stored_direction() {
        let mut ff = FlowField::new(10, 10, 16.0);
        let dir = Vec2::new(1.0, 0.0);
        let idx = ff.index(3, 4);
        ff.directions[idx] = dir;

        // Position in the middle of cell (3, 4) → world (3*16+8, 4*16+8) = (56, 72)
        assert_eq!(ff.direction_at(Vec2::new(56.0, 72.0)), dir);
    }

    #[test]
    fn flow_field_is_blocked_at() {
        let mut ff = FlowField::new(10, 10, 16.0);
        // All zero by default = "blocked" (or unreachable)
        assert!(ff.is_blocked_at(Vec2::new(8.0, 8.0)));

        // Set a direction
        let idx = ff.index(0, 0);
        ff.directions[idx] = Vec2::new(1.0, 0.0);
        assert!(!ff.is_blocked_at(Vec2::new(8.0, 8.0)));

        // Out of bounds = blocked
        assert!(ff.is_blocked_at(Vec2::new(-5.0, 0.0)));
    }

    // === DijkstraFlowField Tests ===

    #[test]
    fn dijkstra_simple_open_grid_all_point_toward_goal() {
        let cost_grid = CostGrid::new(5, 5);
        let algo = DijkstraFlowField;
        let flow = algo.compute(&cost_grid, &[(0, 0)]);

        // Cell (4, 0) should point left toward goal at (0, 0)
        let dir = flow.directions[flow.index(4, 0)];
        assert!(dir.x < 0.0, "should point left, got {dir:?}");
        assert!(
            dir.y.abs() < f32::EPSILON,
            "should have no y component, got {dir:?}"
        );

        // Cell (0, 4) should point up toward goal at (0, 0)
        let dir = flow.directions[flow.index(0, 4)];
        assert!(dir.y < 0.0, "should point up, got {dir:?}");
        assert!(
            dir.x.abs() < f32::EPSILON,
            "should have no x component, got {dir:?}"
        );

        // Goal cell should be Vec2::ZERO
        assert_eq!(flow.directions[flow.index(0, 0)], Vec2::ZERO);

        // All non-goal cells should have non-zero direction
        for row in 0..5u32 {
            for col in 0..5u32 {
                if col == 0 && row == 0 {
                    continue;
                }
                let dir = flow.directions[flow.index(col, row)];
                assert_ne!(dir, Vec2::ZERO, "cell ({col}, {row}) should have direction");
            }
        }
    }

    #[test]
    fn dijkstra_blocked_column_routes_around() {
        let mut cost_grid = CostGrid::new(7, 5);
        // Block column 3, rows 0-3 (leave row 4 open for routing)
        for row in 0..4 {
            cost_grid.set(3, row, COST_BLOCKED);
        }

        let algo = DijkstraFlowField;
        let flow = algo.compute(&cost_grid, &[(0, 2)]);

        // Cell (6, 2) should be reachable (can go around the wall at row 4)
        let dir = flow.directions[flow.index(6, 2)];
        assert_ne!(dir, Vec2::ZERO, "cell (6, 2) should be reachable");

        // Cell just to the right of the wall (4, 2) should also be reachable
        let dir = flow.directions[flow.index(4, 2)];
        assert_ne!(dir, Vec2::ZERO, "cell (4, 2) should be reachable");
    }

    #[test]
    fn dijkstra_disconnected_region_returns_zero() {
        let mut cost_grid = CostGrid::new(5, 5);
        // Completely wall off the right side with blocked column 2
        for row in 0..5 {
            cost_grid.set(2, row, COST_BLOCKED);
        }

        let algo = DijkstraFlowField;
        let flow = algo.compute(&cost_grid, &[(0, 0)]);

        // Cells on the right side of the wall should be unreachable
        assert_eq!(
            flow.directions[flow.index(3, 0)],
            Vec2::ZERO,
            "disconnected cell should have ZERO direction"
        );
        assert_eq!(
            flow.directions[flow.index(4, 4)],
            Vec2::ZERO,
            "disconnected cell should have ZERO direction"
        );

        // Cells on the left side should still have directions
        assert_ne!(
            flow.directions[flow.index(1, 0)],
            Vec2::ZERO,
            "reachable cell should have direction"
        );
    }

    #[test]
    fn dijkstra_corner_cutting_prevention() {
        let mut cost_grid = CostGrid::new(5, 5);
        // Block cells creating an L-shape corner:
        // (2, 1) and (1, 2) are blocked
        // Diagonal from (2, 2) to (1, 1) should NOT be allowed
        cost_grid.set(2, 1, COST_BLOCKED);
        cost_grid.set(1, 2, COST_BLOCKED);

        let algo = DijkstraFlowField;
        let flow = algo.compute(&cost_grid, &[(0, 0)]);

        // Cell (2, 2) should be reachable but NOT via diagonal through the corner
        let dir = flow.directions[flow.index(2, 2)];
        assert_ne!(dir, Vec2::ZERO, "cell (2, 2) should be reachable");
    }

    #[test]
    fn dijkstra_multiple_goals() {
        let cost_grid = CostGrid::new(5, 5);
        let algo = DijkstraFlowField;
        let flow = algo.compute(&cost_grid, &[(0, 0), (4, 4)]);

        // Cell (2, 2) is equidistant from both goals — should have a direction
        let dir = flow.directions[flow.index(2, 2)];
        assert_ne!(dir, Vec2::ZERO, "center cell should have direction");

        // Both goal cells should be Vec2::ZERO
        assert_eq!(flow.directions[flow.index(0, 0)], Vec2::ZERO);
        assert_eq!(flow.directions[flow.index(4, 4)], Vec2::ZERO);
    }

    #[test]
    fn dijkstra_adjacent_cost_preferred_over_adjacent_cells() {
        let mut cost_grid = CostGrid::new(5, 1);
        // Row of 5 cells, middle one is high cost
        cost_grid.set(2, 0, COST_ADJACENT);

        let algo = DijkstraFlowField;
        let flow = algo.compute(&cost_grid, &[(0, 0)]);

        // All cells should be reachable (COST_ADJACENT is not blocking)
        for col in 1..5u32 {
            assert_ne!(
                flow.directions[flow.index(col, 0)],
                Vec2::ZERO,
                "cell ({col}, 0) should have direction"
            );
        }
    }

    #[test]
    fn dijkstra_full_battlefield_dimensions() {
        let cost_grid = CostGrid::new(FLOW_COLS, FLOW_ROWS);
        let algo = DijkstraFlowField;

        // Goal at top-left corner
        let flow = algo.compute(&cost_grid, &[(0, 0)]);

        // Far corner should be reachable
        let dir = flow.directions[flow.index(FLOW_COLS - 1, FLOW_ROWS - 1)];
        assert_ne!(
            dir,
            Vec2::ZERO,
            "far corner should be reachable on open grid"
        );

        // Verify grid dimensions
        assert_eq!(flow.cols, 328);
        assert_eq!(flow.rows, 40);
    }

    // === find_nearest_unblocked Tests ===

    #[test]
    fn find_nearest_unblocked_on_open_cell() {
        let grid = CostGrid::new(5, 5);
        let result = find_nearest_unblocked(Vec2::new(40.0, 40.0), &grid);
        assert!(result.is_some());
        // Should return the center of the cell the position is in
        let pos = result.unwrap();
        assert_eq!(pos, Vec2::new(2.5 * FLOW_CELL_SIZE, 2.5 * FLOW_CELL_SIZE));
    }

    #[test]
    fn find_nearest_unblocked_on_blocked_cell() {
        let mut grid = CostGrid::new(5, 5);
        grid.set(2, 2, COST_BLOCKED);

        let result = find_nearest_unblocked(Vec2::new(40.0, 40.0), &grid);
        assert!(result.is_some());
        let pos = result.unwrap();
        // Should NOT be the blocked cell center
        assert_ne!(pos, Vec2::new(2.5 * FLOW_CELL_SIZE, 2.5 * FLOW_CELL_SIZE));
    }

    #[test]
    fn find_nearest_unblocked_all_blocked() {
        let mut grid = CostGrid::new(3, 3);
        for row in 0..3 {
            for col in 0..3 {
                grid.set(col, row, COST_BLOCKED);
            }
        }

        // Position in the center — all nearby cells blocked
        let result = find_nearest_unblocked(Vec2::new(24.0, 24.0), &grid);
        assert!(result.is_none());
    }

    // === Phase 2: Bevy Integration Tests ===

    #[test]
    fn fortress_cells_from_world_correct_count() {
        // Player fortress center at zone_center_x(0, 2), battlefield_center_y()
        // = 64.0, 320.0
        let cells = fortress_cells_from_world(Vec2::new(64.0, 320.0));
        // 2 fortress cols × 2 fortress rows × 4×4 flow cells per bf cell = 2*2*16 = 64
        let expected = u32::from(FORTRESS_COLS) * u32::from(FORTRESS_ROWS) * 4 * 4;
        assert_eq!(
            cells.len(),
            expected as usize,
            "Expected {expected} goal cells, got {}",
            cells.len()
        );
    }

    #[test]
    fn fortress_cells_from_world_all_within_bounds() {
        let cells = fortress_cells_from_world(Vec2::new(64.0, 320.0));
        for (col, row) in &cells {
            assert!(
                *col < FLOW_COLS && *row < FLOW_ROWS,
                "Cell ({col}, {row}) out of bounds"
            );
        }
    }

    #[test]
    fn mark_building_placed_sets_dirty() {
        let cost_grid = CostGrid::new(FLOW_COLS, FLOW_ROWS);
        let algo = Box::new(DijkstraFlowField);
        let pf_cells = fortress_cells_from_world(Vec2::new(64.0, 320.0));
        let ef_cells = fortress_cells_from_world(Vec2::new(5184.0, 320.0));
        let player_ff = algo.compute(&cost_grid, &pf_cells);
        let enemy_ff = algo.compute(&cost_grid, &ef_cells);

        let mut registry = GoalRegistry {
            player_fortress: GoalFlowField {
                flow_field: player_ff,
                goal_cells: pf_cells,
            },
            enemy_fortress: GoalFlowField {
                flow_field: enemy_ff,
                goal_cells: ef_cells,
            },
            cost_grid,
            algorithm: algo,
        };
        let mut dirty = FlowFieldDirty(false);

        mark_building_placed(
            &mut registry,
            &mut dirty,
            Vec2::new(200.0, 200.0),
            Vec2::new(240.0, 240.0),
        );

        assert!(
            dirty.0,
            "Dirty flag should be set after mark_building_placed"
        );

        // Verify cost grid was updated — cells in the building area should be blocked
        let col = (200.0_f32 / FLOW_CELL_SIZE).floor() as u32;
        let row = (200.0_f32 / FLOW_CELL_SIZE).floor() as u32;
        assert!(
            registry.cost_grid.get(col + 1, row + 1) >= COST_BLOCKED,
            "Cell in building area should be blocked"
        );
    }

    #[test]
    fn rebuild_cost_grid_clears_and_rebuilds() {
        let cost_grid = CostGrid::new(20, 20);
        let algo = Box::new(DijkstraFlowField);
        let ff = algo.compute(&cost_grid, &[(0, 0)]);

        let mut registry = GoalRegistry {
            player_fortress: GoalFlowField {
                flow_field: ff.clone(),
                goal_cells: vec![(0, 0)],
            },
            enemy_fortress: GoalFlowField {
                flow_field: ff,
                goal_cells: vec![(19, 19)],
            },
            cost_grid,
            algorithm: algo,
        };
        let mut dirty = FlowFieldDirty(false);

        // Mark a building then rebuild with no buildings
        mark_building_placed(
            &mut registry,
            &mut dirty,
            Vec2::new(32.0, 32.0),
            Vec2::new(64.0, 64.0),
        );
        dirty.0 = false; // Reset dirty

        rebuild_cost_grid_from_buildings(&mut registry, &mut dirty, &[]);

        assert!(dirty.0, "Dirty flag should be set after rebuild");
        // All cells should be open after rebuild with no buildings
        assert!(
            registry.cost_grid.costs.iter().all(|&c| c == COST_OPEN),
            "All cells should be open after empty rebuild"
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::testing::{create_base_test_app, transition_to_ingame};

    fn create_flow_field_test_app() -> App {
        let mut app = create_base_test_app();
        app.add_plugins(crate::gameplay::battlefield::plugin);
        app.add_plugins(plugin);
        transition_to_ingame(&mut app);
        app
    }

    #[test]
    fn goal_registry_exists_after_ingame() {
        let app = create_flow_field_test_app();
        assert!(
            app.world().get_resource::<GoalRegistry>().is_some(),
            "GoalRegistry should exist after entering InGame"
        );
    }

    #[test]
    fn flow_field_dirty_starts_false() {
        let app = create_flow_field_test_app();
        let dirty = app.world().resource::<FlowFieldDirty>();
        assert!(!dirty.0, "FlowFieldDirty should start as false");
    }

    #[test]
    fn goal_registry_has_valid_flow_fields() {
        let app = create_flow_field_test_app();
        let registry = app.world().resource::<GoalRegistry>();

        // Both flow fields should have correct dimensions
        assert_eq!(registry.player_fortress.flow_field.cols, FLOW_COLS);
        assert_eq!(registry.player_fortress.flow_field.rows, FLOW_ROWS);
        assert_eq!(registry.enemy_fortress.flow_field.cols, FLOW_COLS);
        assert_eq!(registry.enemy_fortress.flow_field.rows, FLOW_ROWS);

        // Both should have non-empty goal cells
        assert!(
            !registry.player_fortress.goal_cells.is_empty(),
            "Player fortress should have goal cells"
        );
        assert!(
            !registry.enemy_fortress.goal_cells.is_empty(),
            "Enemy fortress should have goal cells"
        );
    }

    #[test]
    fn flow_fields_have_directions() {
        let app = create_flow_field_test_app();
        let registry = app.world().resource::<GoalRegistry>();

        // On an open grid, most cells should have non-zero directions
        let non_zero_player: usize = registry
            .player_fortress
            .flow_field
            .directions
            .iter()
            .filter(|d| **d != Vec2::ZERO)
            .count();
        let non_zero_enemy: usize = registry
            .enemy_fortress
            .flow_field
            .directions
            .iter()
            .filter(|d| **d != Vec2::ZERO)
            .count();

        // Most cells should have directions (only goal cells + fortress-blocked are zero)
        let total = (FLOW_COLS * FLOW_ROWS) as usize;
        assert!(
            non_zero_player > total / 2,
            "Player flow field should have directions for most cells, got {non_zero_player}/{total}"
        );
        assert!(
            non_zero_enemy > total / 2,
            "Enemy flow field should have directions for most cells, got {non_zero_enemy}/{total}"
        );
    }
}

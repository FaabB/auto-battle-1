# GAM-32: Unit-to-Unit Local Avoidance (ORCA) Implementation Plan

## Overview

Implement ORCA (Optimal Reciprocal Collision Avoidance) so units steer around each other smoothly instead of relying on avian2d physics pushing. The system sits between pathfinding and physics: `unit_movement` computes a preferred velocity from NavPath waypoints, then `compute_avoidance` adjusts it via ORCA before avian2d integrates the result.

Built as ~500 lines of custom Rust (no `dodgy_2d` dependency). Agent-only ORCA — static obstacles are already handled by the navmesh.

## Current State Analysis

### The Pipeline Today

```
GameSet::Ai:       find_target → compute_paths (writes NavPath)
GameSet::Movement: unit_movement (reads NavPath → writes LinearVelocity directly)
[Avian2d]:         reads LinearVelocity → integrates position, resolves pushbox collisions
```

`unit_movement` (`src/gameplay/units/movement.rs:25-97`) sets `LinearVelocity = direction * speed` with zero avoidance. Units ram into each other and avian2d's pushbox collision separates them, causing jitter.

### Key Facts

| Item | Value | Location |
|------|-------|----------|
| `UNIT_RADIUS` | `6.0` px | `units/mod.rs:23` |
| Soldier `move_speed` | `50.0` px/s | `units/mod.rs:75` |
| `WAYPOINT_REACHED_DISTANCE` | `8.0` px | `movement.rs:12` |
| Unit physics | `RigidBody::Dynamic` + `Collider::circle(6.0)` + `LinearVelocity` | `units/mod.rs:129-136` |
| `GameSet` chain | `Input → Production → Ai → Movement → Combat → Death → Ui` | `lib.rs:73-84` |
| Debug toggle | F3 toggles navmesh overlay + path gizmos | `dev_tools/mod.rs:52-84` |

## Desired End State

### Pipeline After

```
GameSet::Ai:       find_target → compute_paths (writes NavPath)
GameSet::Movement: unit_movement (reads NavPath → writes PreferredVelocity)
                   → rebuild_spatial_hash (rebuilds SpatialHash resource)
                   → compute_avoidance (reads PreferredVelocity + LinearVelocity
                                         → ORCA → writes LinearVelocity)
[Avian2d]:         reads LinearVelocity → integrates position
```

### Verification

- Units navigate around each other without jittery physics pushing
- Units still follow navmesh paths around buildings
- Units in dense groups spread smoothly instead of stacking
- F3 debug overlay shows preferred vs actual velocity arrows
- `make check` and `make test` pass
- Performance: 2000 units at 60fps (spatial hash + capped neighbors)

## What We're NOT Doing

- **Static obstacle ORCA lines** — navmesh already routes around buildings/fortresses
- **Flow fields** — ORCA with spatial hash handles 2000 units fine
- **Formation movement** — separate future ticket
- **`dodgy_2d` crate** — building our own for full control and tighter Bevy integration
- **Removing pushbox colliders** — physics still provides a safety net for edge cases

## Implementation Approach

New module `src/gameplay/units/avoidance/` with three files:

| File | Lines (est.) | Contents |
|------|-------------|----------|
| `orca.rs` | ~250 | `OrcaLine`, LP solver, `compute_avoiding_velocity` — pure math, no Bevy |
| `spatial_hash.rs` | ~80 | `SpatialHash` resource, insert/query/clear |
| `mod.rs` | ~200 | `PreferredVelocity`, `AvoidanceAgent`, `AvoidanceConfig`, `compute_avoidance` system, plugin |

Two new components on units: `PreferredVelocity(Vec2)` and `AvoidanceAgent { radius, responsibility }`.

One modified system: `unit_movement` writes `PreferredVelocity` instead of `LinearVelocity`.

Key design decision: `LinearVelocity` on each unit retains its value from the previous frame's ORCA output until `compute_avoidance` overwrites it. This means `compute_avoidance` can read `LinearVelocity` as the agent's "current velocity" (needed for ORCA's relative velocity computation) and `PreferredVelocity` as the optimization target, then write the ORCA result back to `LinearVelocity`.

---

## Phase 1: ORCA Math Module

### Overview
Pure Rust ORCA implementation with no Bevy dependencies. All functions are deterministic and unit-testable.

### Changes Required

#### 1. Create `src/gameplay/units/avoidance/orca.rs`

**File**: `src/gameplay/units/avoidance/orca.rs` (new file)

The ORCA algorithm for agent-agent avoidance in 2D. No static obstacle handling (navmesh covers that). Based on the RVO2 reference implementation.

```rust
//! ORCA (Optimal Reciprocal Collision Avoidance) — pure math, no Bevy dependency.
//!
//! Computes collision-free velocities for agents moving in 2D.
//! Only handles agent-agent avoidance; static obstacles are handled by navmesh pathfinding.

use bevy::math::Vec2;

/// A half-plane constraint in velocity space.
/// All velocities on the side of the line where `direction` points left (via `perp()`) are valid.
#[derive(Debug, Clone, Copy)]
pub struct OrcaLine {
    /// A point on the boundary line in velocity space.
    pub point: Vec2,
    /// Unit direction vector along the line.
    pub direction: Vec2,
}

/// Snapshot of one agent's state for ORCA computation.
#[derive(Debug, Clone, Copy)]
pub struct AgentSnapshot {
    pub position: Vec2,
    pub velocity: Vec2,          // current velocity (from last frame's ORCA output)
    pub preferred: Vec2,         // desired velocity (from pathfinding)
    pub radius: f32,
    pub max_speed: f32,
    pub responsibility: f32,     // 0.0–1.0, typically 0.5
}

/// Compute the ORCA half-plane constraint for agent A avoiding agent B.
///
/// Returns `None` if agents are at the same position (degenerate case).
pub fn compute_orca_line(a: &AgentSnapshot, b: &AgentSnapshot, time_horizon: f32) -> Option<OrcaLine> {
    // ... truncated cone geometry, u-vector, half-plane construction ...
    // See detailed algorithm below
}

/// Compute the best collision-free velocity for an agent given ORCA constraints.
///
/// Finds the velocity closest to `preferred` that satisfies all half-plane
/// constraints and lies within the `max_speed` disc.
pub fn compute_avoiding_velocity(
    preferred: Vec2,
    max_speed: f32,
    lines: &[OrcaLine],
) -> Vec2 {
    let (result, fail_line) = linear_program_2(lines, preferred, max_speed);
    if fail_line < lines.len() {
        // Some constraints were infeasible — use fallback
        linear_program_3(lines, fail_line, result, max_speed)
    } else {
        result
    }
}
```

**Algorithm details for `compute_orca_line`:**

1. Compute relative position `rel_pos = b.position - a.position` and relative velocity `rel_vel = a.velocity - b.velocity`
2. Combined radius `combined_radius = a.radius + b.radius`
3. Distance `dist_sq = rel_pos.length_squared()`
4. If agents are not overlapping (`dist_sq > combined_radius²`):
   - Compute the truncated velocity obstacle cone with time horizon `tau`
   - The cone apex is at `rel_pos / tau`, legs are tangent to the Minkowski sum disc
   - Find `u` = minimum adjustment to exit the VO (closest point on cone boundary from `rel_vel`)
   - ORCA line: point = `a.velocity + a.responsibility * u`, direction = perpendicular to `u`
5. If agents ARE overlapping (`dist_sq <= combined_radius²`):
   - Use the cutoff circle at `time_step` boundary
   - `u` = direction to push apart, scaled by penetration depth
   - Higher urgency (larger `u` magnitude)

**LP solver functions:**

- `linear_program_1(lines, line_idx, preferred, max_speed, direction_opt) -> Option<f32>`: 1D optimization along a single constraint line, respecting all prior constraints. Returns the optimal parameter along the line, or `None` if infeasible.
- `linear_program_2(lines, preferred, max_speed) -> (Vec2, usize)`: 2D incremental LP. Processes constraints one by one. If the current solution violates a new constraint, projects onto that constraint via `linear_program_1`. Returns the result and the index of the first failed constraint (or `lines.len()` if all satisfied).
- `linear_program_3(lines, fail_line, current, max_speed) -> Vec2`: Infeasible fallback. Minimizes the maximum constraint violation by iteratively adjusting projected constraints. Used when agents are in a dense crowd and no perfectly collision-free velocity exists.

**Symmetry breaking:** When computing `u` for near-symmetric configurations, add a tiny perturbation based on the agent positions to break ties deterministically. This prevents two agents heading toward each other from choosing the same dodge direction.

### Unit Tests for Phase 1

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Head-on collision: two agents approaching each other
    #[test]
    fn head_on_produces_lateral_avoidance() { ... }

    // Perpendicular crossing: agents crossing at 90 degrees
    #[test]
    fn perpendicular_crossing_adjusts_velocity() { ... }

    // Same direction, different speeds: faster agent behind slower
    #[test]
    fn overtaking_agent_steers_around() { ... }

    // No conflict: agents moving apart
    #[test]
    fn diverging_agents_produce_no_constraint() { ... }

    // Overlapping agents: emergency separation
    #[test]
    fn overlapping_agents_push_apart() { ... }

    // LP solver: single constraint
    #[test]
    fn lp2_single_constraint_respects_half_plane() { ... }

    // LP solver: infeasible (contradictory constraints)
    #[test]
    fn lp3_infeasible_minimizes_violation() { ... }

    // Max speed constraint: result stays within speed disc
    #[test]
    fn result_within_max_speed() { ... }

    // Zero preferred velocity: stationary agent still dodges
    #[test]
    fn zero_preferred_still_avoids() { ... }

    // Responsibility asymmetry: agent with 1.0 takes full dodge
    #[test]
    fn full_responsibility_takes_all_adjustment() { ... }
}
```

### Success Criteria

#### Automated Verification
- [ ] `cargo test --lib gameplay::units::avoidance::orca` — all pure math tests pass
- [ ] `make check` — no clippy warnings

#### Manual Verification
- [ ] None (pure math, no visual output yet)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to Phase 2.

---

## Phase 2: Spatial Hash + Module Scaffold

### Overview
Create the spatial hash for neighbor queries and the avoidance module's components/config.

### Changes Required

#### 1. Create `src/gameplay/units/avoidance/spatial_hash.rs`

**File**: `src/gameplay/units/avoidance/spatial_hash.rs` (new file)

```rust
//! Uniform-grid spatial hash for fast neighbor queries.

use bevy::prelude::*;
use std::collections::HashMap;

/// Spatial hash for O(1) neighbor lookups. Rebuilt every frame.
#[derive(Resource, Debug)]
pub struct SpatialHash {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<Entity>>,
}

impl SpatialHash {
    pub fn new(cell_size: f32) -> Self { ... }

    /// Remove all entries. Called at the start of each frame's rebuild.
    pub fn clear(&mut self) { ... }

    /// Insert an entity at a world position.
    pub fn insert(&mut self, entity: Entity, position: Vec2) { ... }

    /// Query all entities within `radius` of `position`.
    /// Returns candidates — caller must still check actual distance.
    pub fn query_neighbors(&self, position: Vec2, radius: f32) -> Vec<Entity> {
        let min = self.cell_coords(position - Vec2::splat(radius));
        let max = self.cell_coords(position + Vec2::splat(radius));
        let mut result = Vec::new();
        for x in min.0..=max.0 {
            for y in min.1..=max.1 {
                if let Some(entities) = self.cells.get(&(x, y)) {
                    result.extend(entities);
                }
            }
        }
        result
    }

    fn cell_coords(&self, position: Vec2) -> (i32, i32) {
        (
            (position.x / self.cell_size).floor() as i32,
            (position.y / self.cell_size).floor() as i32,
        )
    }
}
```

#### 2. Create `src/gameplay/units/avoidance/mod.rs`

**File**: `src/gameplay/units/avoidance/mod.rs` (new file)

```rust
//! ORCA local avoidance for unit-to-unit collision prevention.

pub mod orca;
pub mod spatial_hash;

use bevy::prelude::*;

use super::{Movement, Unit, UNIT_RADIUS};
use self::orca::AgentSnapshot;
use self::spatial_hash::SpatialHash;

// === Constants ===

/// Default ORCA time horizon in seconds.
const DEFAULT_TIME_HORIZON: f32 = 3.0;
/// Maximum neighbors to consider per agent.
const DEFAULT_MAX_NEIGHBORS: u32 = 10;
/// Velocity smoothing blend factor (0.0 = keep old, 1.0 = fully ORCA).
const DEFAULT_VELOCITY_SMOOTHING: f32 = 0.85;

// === Components ===

/// The velocity the unit wants to move at (from pathfinding/movement logic).
/// Written by `unit_movement`, read by `compute_avoidance`.
#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component)]
pub struct PreferredVelocity(pub Vec2);

/// Per-unit ORCA parameters.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct AvoidanceAgent {
    /// Avoidance radius (typically matches collider radius).
    pub radius: f32,
    /// How much of the avoidance adjustment this agent absorbs (0.0–1.0).
    /// 0.5 = symmetric (both agents dodge equally). 1.0 = this agent takes full responsibility.
    pub responsibility: f32,
}

impl Default for AvoidanceAgent {
    fn default() -> Self {
        Self {
            radius: UNIT_RADIUS,
            responsibility: 0.5,
        }
    }
}

// === Resources ===

/// Global ORCA tuning parameters.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct AvoidanceConfig {
    /// How far ahead (seconds) agents predict collisions with each other.
    pub time_horizon: f32,
    /// Max neighbors to consider per agent. Caps ORCA constraint count.
    pub max_neighbors: u32,
    /// Search radius for neighbors (pixels). Should be >= max_speed * time_horizon.
    pub neighbor_distance: f32,
    /// Blend factor for velocity smoothing (0.0 = old velocity, 1.0 = raw ORCA result).
    pub velocity_smoothing: f32,
}

impl Default for AvoidanceConfig {
    fn default() -> Self {
        Self {
            time_horizon: DEFAULT_TIME_HORIZON,
            neighbor_distance: DEFAULT_TIME_HORIZON * 50.0, // max_speed * time_horizon
            max_neighbors: DEFAULT_MAX_NEIGHBORS,
            velocity_smoothing: DEFAULT_VELOCITY_SMOOTHING,
        }
    }
}
```

### Unit Tests for Phase 2

```rust
// In spatial_hash.rs
#[cfg(test)]
mod tests {
    #[test]
    fn insert_and_query_single_entity() { ... }

    #[test]
    fn query_returns_entities_within_radius() { ... }

    #[test]
    fn query_excludes_distant_entities() { ... }

    #[test]
    fn clear_removes_all_entries() { ... }

    #[test]
    fn entities_on_cell_boundary_found_by_neighbors() { ... }

    #[test]
    fn large_radius_covers_many_cells() { ... }
}
```

### Success Criteria

#### Automated Verification
- [ ] `cargo test --lib gameplay::units::avoidance` — all tests pass
- [ ] `make check` — no warnings

#### Manual Verification
- [ ] None (no visual output yet)

**Implementation Note**: After completing this phase, pause for confirmation before Phase 3.

---

## Phase 3: System Integration

### Overview
Wire everything together: modify `unit_movement` to write `PreferredVelocity`, add the avoidance systems, update spawning and plugin registration, and update all affected tests.

### Changes Required

#### 1. Add systems to `src/gameplay/units/avoidance/mod.rs`

Append the two avoidance systems:

```rust
// === Systems ===

/// Rebuild the spatial hash with all unit positions. Runs every frame.
pub fn rebuild_spatial_hash(
    mut hash: ResMut<SpatialHash>,
    agents: Query<(Entity, &GlobalTransform), With<Unit>>,
) {
    hash.clear();
    for (entity, transform) in &agents {
        hash.insert(entity, transform.translation().xy());
    }
}

/// Compute ORCA-adjusted velocities for all units.
///
/// Reads `PreferredVelocity` (desired direction from pathfinding) and
/// `LinearVelocity` (current velocity from last frame's ORCA output).
/// Writes the ORCA result to `LinearVelocity`.
pub fn compute_avoidance(
    config: Res<AvoidanceConfig>,
    hash: Res<SpatialHash>,
    mut agents: Query<
        (
            Entity,
            &GlobalTransform,
            &mut LinearVelocity,
            &PreferredVelocity,
            &AvoidanceAgent,
            &Movement,
        ),
        With<Unit>,
    >,
) {
    // Phase 1: Snapshot all agent data (immutable read via .iter())
    let snapshots: Vec<(Entity, AgentSnapshot)> = agents
        .iter()
        .map(|(entity, transform, velocity, preferred, avoidance, movement)| {
            (
                entity,
                AgentSnapshot {
                    position: transform.translation().xy(),
                    velocity: velocity.0,
                    preferred: preferred.0,
                    radius: avoidance.radius,
                    max_speed: movement.speed,
                    responsibility: avoidance.responsibility,
                },
            )
        })
        .collect();

    // Build entity → snapshot index lookup for neighbor access
    let index_map: HashMap<Entity, usize> = snapshots
        .iter()
        .enumerate()
        .map(|(i, (e, _))| (*e, i))
        .collect();

    // Phase 2: Compute ORCA velocity for each agent
    let results: Vec<(Entity, Vec2)> = snapshots
        .iter()
        .map(|(entity, agent)| {
            // Skip ORCA for stationary agents with zero preferred velocity
            // (optimization: no need to dodge if not moving)
            if agent.preferred.length_squared() < f32::EPSILON {
                return (*entity, Vec2::ZERO);
            }

            // Gather neighbor snapshots
            let mut lines = Vec::new();
            let candidates = hash.query_neighbors(agent.position, config.neighbor_distance);
            let mut neighbor_count = 0u32;

            for candidate_entity in candidates {
                if candidate_entity == *entity {
                    continue;
                }
                if neighbor_count >= config.max_neighbors {
                    break;
                }
                if let Some(&idx) = index_map.get(&candidate_entity) {
                    let neighbor = &snapshots[idx].1;
                    if let Some(line) = orca::compute_orca_line(
                        agent,
                        neighbor,
                        config.time_horizon,
                    ) {
                        lines.push(line);
                        neighbor_count += 1;
                    }
                }
            }

            // No neighbors nearby — use preferred velocity directly
            if lines.is_empty() {
                return (*entity, agent.preferred);
            }

            let orca_vel = orca::compute_avoiding_velocity(
                agent.preferred,
                agent.max_speed,
                &lines,
            );

            // Velocity smoothing: blend ORCA result with current velocity
            let smoothed = agent.velocity.lerp(orca_vel, config.velocity_smoothing);
            (*entity, smoothed)
        })
        .collect();

    // Phase 3: Write results
    for (entity, new_velocity) in results {
        if let Ok((.., mut linear_vel, ..)) = agents.get_mut(entity) {
            linear_vel.0 = new_velocity;
        }
    }
}
```

**Note**: The `get_mut` destructuring needs to match the query tuple exactly. The `..` patterns skip fields we don't need to write.

#### 2. Modify `src/gameplay/units/movement.rs`

**File**: `src/gameplay/units/movement.rs`
**Change**: Write to `PreferredVelocity` instead of `LinearVelocity`

Import change (line 6-8):
```rust
// Before
use super::{CombatStats, CurrentTarget, Movement, Unit};

// After
use super::avoidance::PreferredVelocity;
use super::{CombatStats, CurrentTarget, Movement, Unit};
```

System signature change (line 25-37):
```rust
// Before
pub(super) fn unit_movement(
    mut units: Query<
        (
            &CurrentTarget,
            &Movement,
            &CombatStats,
            &GlobalTransform,
            &Collider,
            &mut LinearVelocity,
            &mut NavPath,
        ),
        With<Unit>,
    >,
    targets: Query<(&GlobalTransform, &Collider)>,
)

// After
pub(super) fn unit_movement(
    mut units: Query<
        (
            &CurrentTarget,
            &Movement,
            &CombatStats,
            &GlobalTransform,
            &Collider,
            &mut PreferredVelocity,
            &mut NavPath,
        ),
        With<Unit>,
    >,
    targets: Query<(&GlobalTransform, &Collider)>,
)
```

Body changes — replace all `velocity.0 = ...` with `preferred.0 = ...`:
- Line 51: `velocity.0 = Vec2::ZERO;` → `preferred.0 = Vec2::ZERO;`
- Line 55: `velocity.0 = Vec2::ZERO;` → `preferred.0 = Vec2::ZERO;`
- Line 66: `velocity.0 = Vec2::ZERO;` → `preferred.0 = Vec2::ZERO;`
- Line 91: `velocity.0 = Vec2::ZERO;` → `preferred.0 = Vec2::ZERO;`
- Line 96: `velocity.0 = direction * movement.speed;` → `preferred.0 = direction * movement.speed;`

And rename the binding in the destructuring (line 46): `mut velocity` → `mut preferred`

Also remove the `use avian2d::prelude::*;` import if `LinearVelocity` is no longer used in this file.

#### 3. Modify `src/gameplay/units/mod.rs`

**File**: `src/gameplay/units/mod.rs`

Add the avoidance submodule (after line 4):
```rust
pub mod avoidance;
```

Add avoidance components to `spawn_unit` (insert block, line 127-137):
```rust
// Before
.insert((
    pathfinding::NavPath::default(),
    RigidBody::Dynamic,
    Collider::circle(UNIT_RADIUS),
    CollisionLayers::new(
        [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
        [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
    ),
    LockedAxes::ROTATION_LOCKED,
    LinearVelocity::ZERO,
))

// After
.insert((
    pathfinding::NavPath::default(),
    avoidance::PreferredVelocity::default(),
    avoidance::AvoidanceAgent::default(),
    RigidBody::Dynamic,
    Collider::circle(UNIT_RADIUS),
    CollisionLayers::new(
        [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
        [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
    ),
    LockedAxes::ROTATION_LOCKED,
    LinearVelocity::ZERO,
))
```

Update plugin registration (line 199-219):
```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<Unit>()
        .register_type::<UnitType>()
        .register_type::<pathfinding::NavPath>()
        .register_type::<pathfinding::PathRefreshTimer>()
        .register_type::<avoidance::PreferredVelocity>()
        .register_type::<avoidance::AvoidanceAgent>()
        .register_type::<avoidance::AvoidanceConfig>()
        .init_resource::<pathfinding::PathRefreshTimer>()
        .init_resource::<avoidance::AvoidanceConfig>()
        .insert_resource(avoidance::spatial_hash::SpatialHash::new(
            avoidance::AvoidanceConfig::default().neighbor_distance,
        ));

    app.add_systems(OnEnter(GameState::InGame), setup_unit_assets);

    spawn::plugin(app);

    app.add_systems(
        Update,
        (
            pathfinding::compute_paths
                .in_set(GameSet::Ai)
                .after(crate::gameplay::ai::find_target),
        )
            .run_if(gameplay_running),
    );

    // Movement pipeline: preferred velocity → spatial hash → ORCA → LinearVelocity
    app.add_systems(
        Update,
        (
            movement::unit_movement,
            avoidance::rebuild_spatial_hash,
            avoidance::compute_avoidance,
        )
            .chain_ignore_deferred()
            .in_set(GameSet::Movement)
            .run_if(gameplay_running),
    );
}
```

#### 4. Update `src/testing.rs`

**File**: `src/testing.rs`

Add `PreferredVelocity` and `AvoidanceAgent` to `spawn_test_unit` (line 156-181):

```rust
// Add import
use crate::gameplay::units::avoidance::{AvoidanceAgent, PreferredVelocity};

// Add to the spawn bundle (after NavPath::default())
pub fn spawn_test_unit(world: &mut World, team: Team, x: f32, y: f32) -> Entity {
    // ... existing code ...
    world
        .spawn((
            // ... existing components ...
            NavPath::default(),
            PreferredVelocity::default(),   // NEW
            AvoidanceAgent::default(),      // NEW
        ))
        .id()
}
```

#### 5. Update movement tests

**File**: `src/gameplay/units/movement.rs` (tests section, line 100-358)

All movement tests currently check `LinearVelocity`. They need to check `PreferredVelocity` instead, since `unit_movement` now writes to that component.

Changes per test:
- `create_movement_test_app()`: no changes needed (just runs `unit_movement`)
- `spawn_unit_at()`: needs to add `PreferredVelocity::default()` to spawned entities
- All assertions: replace `app.world().get::<LinearVelocity>(unit)` with `app.world().get::<PreferredVelocity>(unit)` and change field access from `velocity.0` to `preferred.0` / `preferred.x`

Example for the first test (`unit_sets_velocity_toward_target`):
```rust
// Before
let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
assert!(velocity.x > 0.0, ...);

// After
let preferred = app.world().get::<PreferredVelocity>(unit).unwrap();
assert!(preferred.0.x > 0.0, ...);
```

Similarly update all 8 movement tests. Also update `spawn_unit_at` helper:
```rust
fn spawn_unit_at(world: &mut World, x: f32, speed: f32, target: Option<Entity>) -> Entity {
    let id = crate::testing::spawn_test_unit(world, Team::Player, x, 100.0);
    world
        .entity_mut(id)
        .insert((Movement { speed }, CurrentTarget(target)));
    id
}
```
(No change needed here since `spawn_test_unit` already includes `PreferredVelocity` after the update to `testing.rs`.)

#### 6. Add avoidance integration tests

**File**: `src/gameplay/units/avoidance/mod.rs` (append test module)

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use avian2d::prelude::*;

    fn create_avoidance_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AvoidanceConfig>();
        app.insert_resource(SpatialHash::new(AvoidanceConfig::default().neighbor_distance));
        app.add_systems(
            Update,
            (rebuild_spatial_hash, compute_avoidance).chain_ignore_deferred(),
        );
        app.update(); // Initialize time
        app
    }

    fn spawn_avoidance_unit(
        world: &mut World,
        x: f32,
        y: f32,
        preferred: Vec2,
        current_vel: Vec2,
    ) -> Entity {
        world
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(x, y, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
                PreferredVelocity(preferred),
                AvoidanceAgent::default(),
                LinearVelocity(current_vel),
            ))
            .id()
    }

    #[test]
    fn lone_unit_keeps_preferred_velocity() {
        // A single unit with no neighbors should get its preferred velocity
        let mut app = create_avoidance_test_app();
        let unit = spawn_avoidance_unit(
            app.world_mut(), 100.0, 100.0,
            Vec2::new(50.0, 0.0), Vec2::new(50.0, 0.0),
        );
        app.update();
        let vel = app.world().get::<LinearVelocity>(unit).unwrap();
        assert!((vel.0 - Vec2::new(50.0, 0.0)).length() < 1.0);
    }

    #[test]
    fn head_on_units_steer_apart() {
        // Two units heading toward each other should get lateral velocity
        let mut app = create_avoidance_test_app();
        let a = spawn_avoidance_unit(
            app.world_mut(), 100.0, 100.0,
            Vec2::new(50.0, 0.0), Vec2::new(50.0, 0.0),
        );
        let b = spawn_avoidance_unit(
            app.world_mut(), 130.0, 100.0,
            Vec2::new(-50.0, 0.0), Vec2::new(-50.0, 0.0),
        );
        app.update();
        let vel_a = app.world().get::<LinearVelocity>(a).unwrap();
        let vel_b = app.world().get::<LinearVelocity>(b).unwrap();
        // Both should have some lateral (y) component to avoid each other
        assert!(vel_a.0.y.abs() > 0.1 || vel_b.0.y.abs() > 0.1,
            "Head-on units should steer laterally: a={:?}, b={:?}", vel_a.0, vel_b.0);
    }

    #[test]
    fn zero_preferred_stays_zero() {
        // A stationary unit with preferred = 0 should remain at zero
        let mut app = create_avoidance_test_app();
        let unit = spawn_avoidance_unit(
            app.world_mut(), 100.0, 100.0,
            Vec2::ZERO, Vec2::ZERO,
        );
        app.update();
        let vel = app.world().get::<LinearVelocity>(unit).unwrap();
        assert!(vel.0.length() < f32::EPSILON);
    }

    #[test]
    fn distant_units_no_avoidance() {
        // Two units far apart should not affect each other
        let mut app = create_avoidance_test_app();
        let a = spawn_avoidance_unit(
            app.world_mut(), 0.0, 0.0,
            Vec2::new(50.0, 0.0), Vec2::new(50.0, 0.0),
        );
        let _b = spawn_avoidance_unit(
            app.world_mut(), 1000.0, 1000.0,
            Vec2::new(-50.0, 0.0), Vec2::new(-50.0, 0.0),
        );
        app.update();
        let vel = app.world().get::<LinearVelocity>(a).unwrap();
        assert!((vel.0 - Vec2::new(50.0, 0.0)).length() < 1.0,
            "Distant agents should not affect each other");
    }
}
```

### Success Criteria

#### Automated Verification
- [ ] `make check` — no clippy warnings
- [ ] `make test` — all tests pass (existing + new)
- [ ] Existing movement tests pass with `PreferredVelocity` checks
- [ ] New avoidance integration tests pass

#### Manual Verification
- [ ] Run `cargo run`, place barracks, observe spawned units
- [ ] Units navigate around each other without physics jitter
- [ ] Units still path around buildings via navmesh
- [ ] Dense groups of units spread smoothly
- [ ] Units reaching attack range still stop correctly
- [ ] No visible performance degradation

**Implementation Note**: This is the critical phase. After automated checks pass, the human should manually play-test to verify units move naturally. Pause here for confirmation before Phase 4.

---

## Phase 4: Debug Visualization

### Overview
Add ORCA debug drawing to the dev tools, gated on F3.

### Changes Required

#### 1. Modify `src/dev_tools/mod.rs`

**File**: `src/dev_tools/mod.rs`

Add import for avoidance components (after line 10):
```rust
use crate::gameplay::units::avoidance::PreferredVelocity;
```

Register the new debug system in `plugin()` (after line 34):
```rust
app.add_systems(
    Update,
    debug_draw_avoidance
        .run_if(crate::gameplay_running.and(resource_exists::<NavMeshesDebug>)),
);
```

Add the drawing function:
```rust
/// Draw ORCA debug visualization: green = preferred velocity, cyan = actual (ORCA-adjusted).
fn debug_draw_avoidance(
    units: Query<(&GlobalTransform, &LinearVelocity, &PreferredVelocity), With<Unit>>,
    mut gizmos: Gizmos,
) {
    for (transform, velocity, preferred) in &units {
        let pos = transform.translation().xy();
        let scale = 0.5; // Scale arrows to be visible but not overwhelming

        // Green arrow: preferred velocity (where pathfinding wants to go)
        if preferred.0.length_squared() > f32::EPSILON {
            gizmos.arrow_2d(
                pos,
                pos + preferred.0 * scale,
                Color::srgb(0.0, 1.0, 0.0),
            );
        }

        // Cyan arrow: actual velocity (ORCA-adjusted)
        if velocity.0.length_squared() > f32::EPSILON {
            gizmos.arrow_2d(
                pos,
                pos + velocity.0 * scale,
                Color::srgb(0.0, 1.0, 1.0),
            );
        }
    }
}
```

When preferred (green) and actual (cyan) arrows diverge, ORCA is actively steering the unit. When they overlap, no avoidance is needed.

### Success Criteria

#### Automated Verification
- [ ] `make check` — no warnings
- [ ] `make test` — all tests pass

#### Manual Verification
- [ ] Press F3 in-game to see debug overlay
- [ ] Green arrows show pathfinding desired direction
- [ ] Cyan arrows show ORCA-adjusted direction
- [ ] Arrows diverge when units are near each other (ORCA active)
- [ ] Arrows converge when units have open space (no avoidance needed)

---

## Testing Strategy

### Unit Tests (pure math)
- ORCA line construction: head-on, perpendicular, overtaking, diverging, overlapping
- LP solver: single constraint, multiple constraints, infeasible, speed disc
- Spatial hash: insert, query, clear, boundary, large radius

### Integration Tests (Bevy systems)
- Lone unit keeps preferred velocity
- Head-on collision produces lateral avoidance
- Zero preferred stays zero
- Distant units unaffected
- Spatial hash correctly rebuilt each frame

### Updated Existing Tests
- All 8 movement tests updated to check `PreferredVelocity`
- `spawn_test_unit` updated with new components

### Manual Testing Steps
1. Build and run: `cargo run`
2. Start a game, place 2-3 barracks
3. Wait for waves — observe 20+ units in the field
4. Toggle F3 to see velocity arrows
5. Verify smooth unit flow without jitter
6. Place buildings in unit paths — verify pathfinding still works
7. Observe dense groups near fortress — should spread, not stack

## Performance Considerations

| Operation | Cost per frame | Notes |
|-----------|---------------|-------|
| Spatial hash rebuild | O(n) | 2000 inserts, ~microseconds |
| Neighbor queries | O(n × 9 cells) | 2000 agents × ~10 per cell = 180K candidates |
| ORCA LP solve | O(n × k) | 2000 agents × 10 constraints × O(k) LP = ~200K operations |
| Velocity write | O(n) | 2000 writes |

Total estimated: <1ms per frame for 2000 agents. The spatial hash eliminates the O(n²) bottleneck.

**Potential optimization if needed**: Sort spatial hash candidates by distance and only pass the closest `max_neighbors` to the ORCA solver (currently we break after `max_neighbors` without sorting).

## Verified API Patterns (Bevy 0.18)

- `Query::iter()` on a `&mut T` query returns **read-only** items — safe for the snapshot phase
- `Query::get_mut(entity)` after dropping the `iter()` borrow is valid — Rust borrow checker allows this
- `.chain_ignore_deferred()` orders systems without inserting `ApplyDeferred` between them
- `Gizmos::arrow_2d(from, to, color)` draws a 2D arrow
- `resource_exists::<T>` is a built-in run condition (no import needed, in prelude)

## References

- Linear ticket: [GAM-32](https://linear.app/tayhu-games/issue/GAM-32/unit-to-unit-local-avoidance-orca-steering)
- Research doc: `thoughts/shared/research/2026-02-04-tano-style-game-research.md`
- RVO2 reference: [Agent.cpp](https://github.com/mit-acl/Python-RVO2/blob/master/src/Agent.cpp)
- `dodgy_2d` crate (design reference): [docs.rs](https://docs.rs/dodgy_2d/latest/dodgy_2d/)
- ORCA paper: [UNC GAMMA Lab](https://gamma.cs.unc.edu/ORCA/)

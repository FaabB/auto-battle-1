# Separation Force + Remove ORCA (GAM-61) Implementation Plan

## Overview

Replace ORCA local avoidance with team-weighted separation force. Units move via direct `Transform` writes (from flow field or target steering) plus a separation force that prevents overlap. Remove all ORCA infrastructure: `avoidance/` module, `PreferredVelocity`, `AvoidanceAgent`, `AvoidanceConfig`, `AvoidanceSpatialHash`, and the `HashMap`-based `SpatialHash`.

## Current State Analysis

**Movement pipeline** (3 systems chained in `GameSet::Movement`):
1. `unit_movement` → reads flow field / target position → writes `PreferredVelocity`
2. `rebuild_spatial_hash` → clears + repopulates `AvoidanceSpatialHash` with all `Unit` positions
3. `compute_avoidance` → reads `PreferredVelocity` → runs ORCA LP solver → writes `LinearVelocity`

Avian2d physics then integrates `LinearVelocity` into `Transform`. This indirect pipeline (preferred → ORCA → physics) is replaced by direct `Transform` writes + separation force.

**Files to delete**: `avoidance/mod.rs`, `avoidance/orca.rs`, `spatial_hash.rs`
**Files to create**: `separation.rs`
**Files to modify**: `units/mod.rs`, `units/movement.rs`, `testing.rs`, `dev_tools/mod.rs`, `gameplay/mod.rs`

### Key Discoveries:
- `LinearVelocity` is only used on units by ORCA — safe to remove from unit archetype
- `PreferredVelocity` is only written by `unit_movement` and read by `compute_avoidance` — clean removal
- `gameplay/spatial_hash.rs` has no consumers besides `AvoidanceSpatialHash` — can be deleted
- Battlefield is 5248×640px. At 24px cells: 219×27 = 5,913 cells for the flat grid
- `TargetingState` has 4 variants: `Moving`, `Seeking`, `Engaging(Entity)`, `Attacking(Entity)`

## Desired End State

After this plan:
- Units move via direct `Transform` writes in `unit_movement` (flow field direction × speed × dt)
- `SeparationForce` component stores per-unit push vectors, applied as a separate `Transform` adjustment
- `UnitSpatialHash` is a flat `Vec<Vec<Entity>>` grid with 24px cells — zero hash overhead
- No ORCA code, no `PreferredVelocity`, no `AvoidanceAgent`, no `LinearVelocity` on units
- Cross-team lateral nudge (`perp()`) prevents head-on oscillation
- Only `Seeking`/`Engaging`/`Attacking` units receive separation; ALL units in the spatial hash
- `cargo test` passes, `make check` passes, separation visually works in-game

## What We're NOT Doing

- **Not removing avian2d** — that's GAM-62. `RigidBody`, `Collider`, `CollisionLayers` stay on units for now.
- **Not tuning constants** — that's GAM-63 (profiling pass). We use the research doc's starting values.
- **Not optimizing the flat grid further** (SIMD, parallel iteration) — premature until profiling.

## Implementation Approach

Single phase — the change is a clean swap. The old avoidance pipeline is replaced in one pass because:
1. `PreferredVelocity` is only the bridge between movement and ORCA — removing ORCA means removing the bridge
2. Direct `Transform` writes are simpler than the indirect velocity pipeline
3. The new separation systems are self-contained in one file

## Phase 1: Separation Force + Remove ORCA

### Overview
Create `separation.rs` with the flat spatial hash and two-system separation pipeline. Rewrite `unit_movement` to write `Transform` directly. Delete `avoidance/` module and `spatial_hash.rs`. Update all touch points.

### Changes Required:

#### 1. New file: `src/gameplay/units/separation.rs`

**File**: `src/gameplay/units/separation.rs`
**Purpose**: Team-weighted separation force with flat spatial grid

```rust
//! Team-weighted separation force for unit-to-unit collision prevention.

use bevy::prelude::*;

use super::Unit;
use crate::gameplay::{Team, TargetingState};
use crate::gameplay::battlefield::{BATTLEFIELD_HEIGHT, BATTLEFIELD_WIDTH};

// === Constants ===

/// Cell size for the unit spatial hash (pixels).
const UNIT_CELL_SIZE: f32 = 24.0;

/// Grid columns for the unit spatial hash.
const GRID_COLS: u32 = (BATTLEFIELD_WIDTH / UNIT_CELL_SIZE) as u32 + 1; // 219 + 1 for rounding

/// Grid rows for the unit spatial hash.
const GRID_ROWS: u32 = (BATTLEFIELD_HEIGHT / UNIT_CELL_SIZE) as u32 + 1; // 27 + 1 for rounding

/// Same-team separation strength.
const SAME_TEAM_STRENGTH: f32 = 30.0;

/// Cross-team separation strength.
const CROSS_TEAM_STRENGTH: f32 = 90.0;

/// Cross-team lateral slide (perpendicular nudge).
const CROSS_TEAM_SLIDE: f32 = 15.0;

/// Radius within which separation forces are applied.
const SEPARATION_RADIUS: f32 = 20.0;

/// Maximum separation force magnitude.
const SEPARATION_MAX: f32 = 60.0;

// === Components ===

/// Intermediate separation push vector, computed per frame.
/// Written by `compute_separation`, read by `apply_separation`.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
#[reflect(Component)]
pub struct SeparationForce(pub Vec2);

// === Resources ===

/// Flat-grid spatial hash for unit neighbor lookups.
/// Rebuilt every frame. Direct array index — zero hash overhead.
#[derive(Resource, Debug)]
pub struct UnitSpatialHash {
    cells: Vec<Vec<Entity>>,
    cols: u32,
    rows: u32,
}

impl Default for UnitSpatialHash {
    fn default() -> Self {
        let total = (GRID_COLS * GRID_ROWS) as usize;
        Self {
            cells: vec![Vec::new(); total],
            cols: GRID_COLS,
            rows: GRID_ROWS,
        }
    }
}

impl UnitSpatialHash {
    /// Clear all cells, retaining allocated capacity.
    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.clear();
        }
    }

    /// Insert an entity at a world position. Out-of-bounds positions are clamped.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn insert(&mut self, entity: Entity, position: Vec2) {
        if let Some(idx) = self.cell_index(position) {
            self.cells[idx].push(entity);
        }
    }

    /// Iterate over all entities within `radius` of `position` via callback.
    /// Zero-allocation — no Vec returned.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn for_each_neighbor(
        &self,
        position: Vec2,
        radius: f32,
        mut callback: impl FnMut(Entity),
    ) {
        let min_col = ((position.x - radius) / UNIT_CELL_SIZE)
            .floor()
            .max(0.0) as usize;
        let max_col = ((position.x + radius) / UNIT_CELL_SIZE)
            .floor()
            .min((self.cols - 1) as f32) as usize;
        let min_row = ((position.y - radius) / UNIT_CELL_SIZE)
            .floor()
            .max(0.0) as usize;
        let max_row = ((position.y + radius) / UNIT_CELL_SIZE)
            .floor()
            .min((self.rows - 1) as f32) as usize;

        for row in min_row..=max_row {
            for col in min_col..=max_col {
                for &entity in &self.cells[row * self.cols as usize + col] {
                    callback(entity);
                }
            }
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn cell_index(&self, position: Vec2) -> Option<usize> {
        let col = (position.x / UNIT_CELL_SIZE).floor() as i32;
        let row = (position.y / UNIT_CELL_SIZE).floor() as i32;
        if col < 0 || col >= self.cols as i32 || row < 0 || row >= self.rows as i32 {
            return None;
        }
        Some(row as usize * self.cols as usize + col as usize)
    }
}

// === Systems ===

/// Rebuild the unit spatial hash with all unit positions. Runs every frame.
pub fn rebuild_unit_spatial_hash(
    mut hash: ResMut<UnitSpatialHash>,
    units: Query<(Entity, &GlobalTransform), With<Unit>>,
) {
    hash.clear();
    for (entity, transform) in &units {
        hash.insert(entity, transform.translation().xy());
    }
}

/// Compute separation forces for units in contact-zone states.
/// Skips `Moving` units (they're spread on the flow field).
/// ALL units are in the spatial hash so Moving units still push others.
pub fn compute_separation(
    units: Query<(Entity, &GlobalTransform, &Team, &TargetingState), With<Unit>>,
    mut forces: Query<&mut SeparationForce>,
    hash: Res<UnitSpatialHash>,
) {
    for (entity, transform, team, state) in &units {
        // Only Seeking/Engaging/Attacking units receive separation
        if matches!(state, TargetingState::Moving) {
            if let Ok(mut f) = forces.get_mut(entity) {
                f.0 = Vec2::ZERO;
            }
            continue;
        }

        let pos = transform.translation().xy();
        let mut push = Vec2::ZERO;

        hash.for_each_neighbor(pos, SEPARATION_RADIUS, |neighbor| {
            if neighbor == entity {
                return;
            }
            let Ok((_, neighbor_tf, neighbor_team, _)) = units.get(neighbor) else {
                return;
            };
            let diff = pos - neighbor_tf.translation().xy();
            let dist = diff.length();
            if dist < f32::EPSILON || dist >= SEPARATION_RADIUS {
                return;
            }

            let dir = diff / dist; // normalize
            if *neighbor_team != *team {
                // Cross-team: strong push + lateral slide
                push += dir * (CROSS_TEAM_STRENGTH / dist) + dir.perp() * CROSS_TEAM_SLIDE;
            } else {
                // Same-team: gentle push
                push += dir * (SAME_TEAM_STRENGTH / dist);
            }
        });

        if let Ok(mut force) = forces.get_mut(entity) {
            force.0 = if push.length_squared() > f32::EPSILON {
                push.normalize() * push.length().min(SEPARATION_MAX)
            } else {
                Vec2::ZERO
            };
        }
    }
}

/// Apply separation forces to unit transforms.
pub fn apply_separation(
    mut units: Query<(&mut Transform, &SeparationForce), With<Unit>>,
    time: Res<Time>,
) {
    let dt = time.delta_secs();
    for (mut transform, force) in &mut units {
        if force.0.length_squared() > f32::EPSILON {
            transform.translation += (force.0 * dt).extend(0.0);
        }
    }
}
```

#### 2. Rewrite: `src/gameplay/units/movement.rs`

**File**: `src/gameplay/units/movement.rs`
**Changes**: `unit_movement` writes `Transform` directly instead of `PreferredVelocity`. Adds `Time` parameter. Removes `PreferredVelocity` from query.

```rust
//! Unit movement using flow field directions.

use bevy::prelude::*;

use super::{CombatStats, Movement, TargetingState, Unit};
use crate::gameplay::flow_field::{AssignedGoal, GoalRegistry};
use crate::gameplay::{EntityExtent, extent_distance};

/// Moves units based on targeting state and flow field.
///
/// - `Moving`/`Seeking`: follow flow field toward assigned goal.
/// - `Engaging`: steer directly toward target; stop if in attack range.
/// - `Attacking`: no movement.
///
/// Writes `Transform.translation` directly.
/// Runs in `GameSet::Movement`.
pub(super) fn unit_movement(
    mut units: Query<
        (
            &TargetingState,
            &Movement,
            &CombatStats,
            &mut Transform,
            &GlobalTransform,
            &EntityExtent,
            &AssignedGoal,
        ),
        With<Unit>,
    >,
    targets: Query<(&GlobalTransform, &EntityExtent)>,
    registry: Option<Res<GoalRegistry>>,
    time: Res<Time>,
) {
    let Some(registry) = registry else { return };
    let dt = time.delta_secs();

    for (targeting_state, movement, stats, mut transform, global_transform, unit_extent, goal) in
        &mut units
    {
        let current_xy = global_transform.translation().xy();

        let velocity = match *targeting_state {
            TargetingState::Moving | TargetingState::Seeking => {
                let flow_field = match goal {
                    AssignedGoal::EnemyFortress => &registry.enemy_fortress.flow_field,
                    AssignedGoal::PlayerFortress => &registry.player_fortress.flow_field,
                };
                let direction = flow_field.direction_at(current_xy);
                direction * movement.speed
            }
            TargetingState::Engaging(target_entity) => {
                let Ok((target_pos, target_extent)) = targets.get(target_entity) else {
                    Vec2::ZERO // target despawned
                    continue; // (handled below)
                };
                let target_xy = target_pos.translation().xy();
                let distance =
                    extent_distance(unit_extent, current_xy, target_extent, target_xy);

                if distance <= stats.range {
                    Vec2::ZERO
                } else {
                    let diff = target_xy - current_xy;
                    let dist = diff.length();
                    if dist < f32::EPSILON {
                        Vec2::ZERO
                    } else {
                        (diff / dist) * movement.speed
                    }
                }
            }
            TargetingState::Attacking(_) => Vec2::ZERO,
        };

        transform.translation += (velocity * dt).extend(0.0);
    }
}
```

**Note on Engaging despawned target**: The current code does `preferred.0 = Vec2::ZERO; continue;` — the new code needs the same pattern. The snippet above uses `continue` to skip the translation update for despawned targets (velocity is already zero).

#### 3. Update: `src/gameplay/units/mod.rs`

**Changes**:
- Replace `pub mod avoidance;` with `pub mod separation;`
- Remove imports: `AvoidanceAgent`, `AvoidanceConfig`, `AvoidanceSpatialHash`, `PreferredVelocity`, `SpatialHash`
- Add imports: `SeparationForce`, `UnitSpatialHash`
- Remove from `spawn_unit`: `LinearVelocity::ZERO`, `PreferredVelocity::default()`, `AvoidanceAgent::default()`
- Add to `spawn_unit`: `SeparationForce::default()`
- Update plugin: remove avoidance type registrations and resource init. Add separation type registration and `UnitSpatialHash` resource init.
- Update system chain: `unit_movement` → `rebuild_unit_spatial_hash` → `compute_separation` → `apply_separation`
- Remove `LockedAxes::ROTATION_LOCKED` from spawn (physics cleanup — but since avian2d is still present, keep `RigidBody::Dynamic` and `Collider` for now per GAM-62)

Actually, keep `LockedAxes` since avian2d is still on units — removing it could cause visual rotation. GAM-62 removes all physics components.

Specific spawn changes — remove from second `.insert()`:
- `LinearVelocity::ZERO`
- `PreferredVelocity::default()`
- `AvoidanceAgent::default()`

Add to second `.insert()`:
- `SeparationForce::default()`

Plugin changes:
```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<Unit>()
        .register_type::<UnitType>()
        .register_type::<SeparationForce>()
        .init_resource::<UnitSpatialHash>();

    app.add_systems(OnEnter(GameState::InGame), setup_unit_assets);

    spawn::plugin(app);

    app.add_systems(
        Update,
        (
            movement::unit_movement,
            separation::rebuild_unit_spatial_hash,
            separation::compute_separation,
            separation::apply_separation,
        )
            .chain_ignore_deferred()
            .in_set(GameSet::Movement)
            .run_if(gameplay_running),
    );
}
```

#### 4. Update: `src/testing.rs`

**Changes**:
- Remove import: `AvoidanceAgent`, `PreferredVelocity`
- Add import: `SeparationForce` from `separation` module
- In `spawn_test_unit`: remove `LinearVelocity::ZERO`, `PreferredVelocity::default()`, `AvoidanceAgent::default()` from second `.insert()`. Add `SeparationForce::default()`.
- Update doc comment to remove `LinearVelocity` mention.

#### 5. Update: `src/dev_tools/mod.rs`

**Changes**:
- Remove import: `LinearVelocity` (from avian2d), `PreferredVelocity`
- Add import: `SeparationForce` from `separation` module
- Rename `debug_draw_avoidance` → `debug_draw_separation`
- Change query from `(&GlobalTransform, &LinearVelocity, &PreferredVelocity)` to `(&GlobalTransform, &SeparationForce)`
- Draw one arrow (green) for `SeparationForce` instead of two

```rust
/// Draw separation force debug visualization.
fn debug_draw_separation(
    units: Query<(&GlobalTransform, &SeparationForce), With<Unit>>,
    mut gizmos: Gizmos,
) {
    let scale = 0.5;
    for (transform, force) in &units {
        if force.0.length_squared() > f32::EPSILON {
            let pos = transform.translation().xy();
            gizmos.arrow_2d(pos, pos + force.0 * scale, Color::srgb(1.0, 0.5, 0.0));
        }
    }
}
```

#### 6. Delete files

- Delete `src/gameplay/units/avoidance/mod.rs`
- Delete `src/gameplay/units/avoidance/orca.rs`
- Delete `src/gameplay/units/avoidance/` directory
- Delete `src/gameplay/spatial_hash.rs`

#### 7. Update: `src/gameplay/mod.rs`

**Changes**:
- Remove `pub mod spatial_hash;` module declaration
- Update entity archetype doc comments to remove `LinearVelocity` from Units, add `SeparationForce`

#### 8. Update: `src/gameplay/units/movement.rs` tests

**Changes**: All 7 tests currently assert on `PreferredVelocity`. Since movement now writes `Transform` directly, tests should assert on `Transform.translation` changes after an update tick.

Key changes:
- Remove `PreferredVelocity` from test assertions
- Add `GoalRegistry` resource to test app (already present)
- Assert `Transform.translation` moved in the expected direction after `app.update()`
- For "zero velocity" cases (attacking, in-range, despawned target), assert position unchanged

Test approach:
```rust
// Before
let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
assert!(velocity.0.x > 0.0);

// After
let pos_before = app.world().get::<Transform>(unit).unwrap().translation;
app.update();
let pos_after = app.world().get::<Transform>(unit).unwrap().translation;
assert!(pos_after.x > pos_before.x, "Unit should move rightward");
```

#### 9. New tests: `src/gameplay/units/separation.rs`

Add tests inside the new `separation.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_spatial_hash_insert_and_query() {
        let mut hash = UnitSpatialHash::default();
        let entity = Entity::from_bits(1);
        hash.insert(entity, Vec2::new(100.0, 100.0));
        let mut found = Vec::new();
        hash.for_each_neighbor(Vec2::new(100.0, 100.0), 10.0, |e| found.push(e));
        assert!(found.contains(&entity));
    }

    #[test]
    fn unit_spatial_hash_excludes_distant() {
        let mut hash = UnitSpatialHash::default();
        let near = Entity::from_bits(1);
        let far = Entity::from_bits(2);
        hash.insert(near, Vec2::new(100.0, 100.0));
        hash.insert(far, Vec2::new(500.0, 500.0));
        let mut found = Vec::new();
        hash.for_each_neighbor(Vec2::new(100.0, 100.0), 30.0, |e| found.push(e));
        assert!(found.contains(&near));
        assert!(!found.contains(&far));
    }

    #[test]
    fn unit_spatial_hash_clear_removes_all() {
        let mut hash = UnitSpatialHash::default();
        hash.insert(Entity::from_bits(1), Vec2::new(100.0, 100.0));
        hash.clear();
        let mut found = Vec::new();
        hash.for_each_neighbor(Vec2::new(100.0, 100.0), 1000.0, |e| found.push(e));
        assert!(found.is_empty());
    }

    #[test]
    fn out_of_bounds_insert_ignored() {
        let mut hash = UnitSpatialHash::default();
        let entity = Entity::from_bits(1);
        hash.insert(entity, Vec2::new(-100.0, -100.0));
        let mut found = Vec::new();
        hash.for_each_neighbor(Vec2::new(0.0, 0.0), 200.0, |e| found.push(e));
        assert!(!found.contains(&entity));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::gameplay::units::Unit;
    use crate::gameplay::{Movement, Team};

    fn create_separation_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<UnitSpatialHash>();
        app.add_systems(
            Update,
            (rebuild_unit_spatial_hash, compute_separation)
                .chain_ignore_deferred(),
        );
        app.update(); // Initialize time
        app
    }

    fn spawn_separation_unit(
        world: &mut World,
        team: Team,
        x: f32,
        y: f32,
        state: TargetingState,
    ) -> Entity {
        world
            .spawn((
                Unit,
                team,
                state,
                Movement { speed: 50.0 },
                Transform::from_xyz(x, y, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
                SeparationForce::default(),
            ))
            .id()
    }

    #[test]
    fn lone_unit_gets_zero_separation() {
        let mut app = create_separation_test_app();
        let unit = spawn_separation_unit(
            app.world_mut(), Team::Player, 100.0, 100.0, TargetingState::Seeking,
        );
        app.update();
        let force = app.world().get::<SeparationForce>(unit).unwrap();
        assert!(force.0.length() < f32::EPSILON);
    }

    #[test]
    fn close_same_team_units_pushed_apart() {
        let mut app = create_separation_test_app();
        let a = spawn_separation_unit(
            app.world_mut(), Team::Player, 100.0, 100.0, TargetingState::Seeking,
        );
        let b = spawn_separation_unit(
            app.world_mut(), Team::Player, 110.0, 100.0, TargetingState::Seeking,
        );
        app.update();
        let force_a = app.world().get::<SeparationForce>(a).unwrap();
        let force_b = app.world().get::<SeparationForce>(b).unwrap();
        // A should be pushed left (away from B)
        assert!(force_a.0.x < 0.0, "A should be pushed left, got {:?}", force_a.0);
        // B should be pushed right (away from A)
        assert!(force_b.0.x > 0.0, "B should be pushed right, got {:?}", force_b.0);
    }

    #[test]
    fn cross_team_gets_lateral_nudge() {
        let mut app = create_separation_test_app();
        let a = spawn_separation_unit(
            app.world_mut(), Team::Player, 100.0, 100.0, TargetingState::Seeking,
        );
        let _b = spawn_separation_unit(
            app.world_mut(), Team::Enemy, 110.0, 100.0, TargetingState::Seeking,
        );
        app.update();
        let force = app.world().get::<SeparationForce>(a).unwrap();
        // Cross-team should have lateral (y) component from perp() nudge
        assert!(force.0.y.abs() > 0.1, "Cross-team should have lateral nudge, got {:?}", force.0);
    }

    #[test]
    fn moving_units_get_zero_separation() {
        let mut app = create_separation_test_app();
        let a = spawn_separation_unit(
            app.world_mut(), Team::Player, 100.0, 100.0, TargetingState::Moving,
        );
        let _b = spawn_separation_unit(
            app.world_mut(), Team::Player, 110.0, 100.0, TargetingState::Seeking,
        );
        app.update();
        let force = app.world().get::<SeparationForce>(a).unwrap();
        assert!(force.0.length() < f32::EPSILON, "Moving units should not receive separation");
    }

    #[test]
    fn distant_units_no_separation() {
        let mut app = create_separation_test_app();
        let a = spawn_separation_unit(
            app.world_mut(), Team::Player, 100.0, 100.0, TargetingState::Seeking,
        );
        let _b = spawn_separation_unit(
            app.world_mut(), Team::Player, 500.0, 500.0, TargetingState::Seeking,
        );
        app.update();
        let force = app.world().get::<SeparationForce>(a).unwrap();
        assert!(force.0.length() < f32::EPSILON, "Distant units should not affect each other");
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (clippy + compile)
- [ ] `make test` passes — all new + updated tests green
- [ ] No references to `PreferredVelocity`, `AvoidanceAgent`, `AvoidanceConfig`, `AvoidanceSpatialHash` remain in `src/`
- [ ] `avoidance/` directory deleted
- [ ] `spatial_hash.rs` deleted
- [ ] `separation.rs` exists with `SeparationForce`, `UnitSpatialHash`, 3 systems, tests

#### Manual Verification:
- [ ] Units stream across battlefield following flow field (same as before)
- [ ] Same-team units push apart gently — no stacking on same pixel
- [ ] Opposing armies slide past each other at contact line (lateral nudge visible)
- [ ] F3 debug overlay shows separation force arrows (orange) on units in contact zone
- [ ] No visible regression in unit movement behavior
- [ ] Performance acceptable at current unit counts

**Implementation Note**: This is a single phase. After automated verification passes, pause for manual confirmation before marking complete.

## Testing Strategy

### Unit Tests (in `separation.rs`):
- `UnitSpatialHash`: insert/query, clear, out-of-bounds, distance exclusion
- `compute_separation`: lone unit (zero), same-team push, cross-team lateral nudge, moving units skipped, distant units unaffected

### Updated Tests (in `movement.rs`):
- All 7 existing tests rewritten to assert `Transform.translation` changes instead of `PreferredVelocity`
- Same scenarios: seeking → rightward, moving → rightward, engaging → toward target, in-range → stop, attacking → stop, despawned → stop, diagonal direction normalized

### Integration Test (in `units/mod.rs`):
- Existing `unit_assets_created_on_enter_ingame` test should still pass

## Verified API Patterns (Bevy 0.18)

- `Transform` is a component — can be queried mutably alongside `GlobalTransform` (read-only)
- `Time::delta_secs()` returns `f32` — no `.as_secs_f32()` needed
- `Vec2::perp()` returns the perpendicular vector (rotated 90°) — in prelude
- `.chain_ignore_deferred()` suppresses auto-`ApplyDeferred` between chained systems
- `#[derive(Component, Debug, Clone, Copy, Default, Reflect)]` + `#[reflect(Component)]` for marker/data components
- `init_resource::<T>()` works when `T: Default` — used for `UnitSpatialHash`

## References

- Linear ticket: [GAM-61](https://linear.app/tayhu-games/issue/GAM-61/separation-force-remove-orca)
- Research: `thoughts/shared/research/2026-03-05-targeting-movement-combat-scalability.md` (Section 4.5, 4.6, Ticket 4)
- Blocks: [GAM-62](https://linear.app/tayhu-games/issue/GAM-62/remove-avian2d-physics-engine) (remove avian2d)
- Depends on: [GAM-60](https://linear.app/tayhu-games/issue/GAM-60/flow-field-infrastructure-remove-navmesh) (flow field — Done)

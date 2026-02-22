# GAM-12: NavMesh Pathfinding Implementation Plan

## Overview

Add navmesh-based pathfinding so units route around buildings and fortresses instead of moving in straight lines. Uses `vleue_navigator` (v0.15) with native avian2d integration — colliders automatically become navmesh obstacles.

Unit-to-unit avoidance (ORCA) is deferred to GAM-32.

## Current State Analysis

- **Movement** (`gameplay/units/movement.rs:13`): `unit_movement` system sets `LinearVelocity` directly toward `CurrentTarget` — pure straight line. No obstacle awareness.
- **AI** (`gameplay/ai.rs:24`): `find_target` picks nearest enemy target. Runs in `GameSet::Ai`.
- **Physics**: Units have `RigidBody::Dynamic` + `Collider::circle(6.0)`. Buildings/fortresses have `RigidBody::Static` + rectangle colliders. Physics pushes units apart but doesn't route them around obstacles.
- **Grid**: 82 cols × 10 rows, `CELL_SIZE = 64px`. Buildings are `40px` sprites in `64px` cells (24px gap). Build zone = cols 2-7.

### Key Discoveries:
- Buildings spawned in `gameplay/building/placement.rs:119` — already have `Collider::rectangle(BUILDING_SPRITE_SIZE, BUILDING_SPRITE_SIZE)`
- Fortresses spawned in `gameplay/battlefield/renderer.rs:71,153` — have `Collider::rectangle(fortress_size.x, fortress_size.y)`
- `UNIT_RADIUS = 6.0` (`gameplay/units/mod.rs:21`) — diameter 12px, fits through 24px building gaps
- `GameSet` ordering: `Input → Production → Ai → Movement → Combat → Death → Ui`
- `NavmeshUpdaterPlugin::<Collider, Marker>` watches entities with both `Collider` AND a marker component, rebuilds navmesh on change

## Desired End State

Units navigate around buildings and fortresses using navmesh pathfinding:
1. A navmesh covers the full battlefield, auto-built from building/fortress colliders
2. When a unit's target changes, a path is computed through the navmesh
3. The movement system follows waypoints from the path, then switches to direct targeting for the final approach
4. When buildings are placed/destroyed, the navmesh rebuilds automatically
5. Units in open space (no obstacles between them and target) get trivial straight-line paths — no performance concern

### How to verify:
- Place buildings in a wall pattern leaving one gap → units route through the gap instead of getting stuck
- Place a building between a spawning barracks and the combat zone → units path around it
- Destroy a building → units update their paths through the new opening
- Units in the open combat zone still move directly toward targets (no visible difference)

## What We're NOT Doing

- **Unit-to-unit avoidance** — deferred to GAM-32 (ORCA steering). Physics handles unit collisions for now.
- **Dynamic obstacles** — units are NOT navmesh obstacles. Only static buildings/fortresses.
- **Async pathfinding** — the battlefield is small (82×10 cells), sync pathfinding is fast enough.
- **Multi-layer navmeshes** — single flat 2D navmesh.

## Implementation Approach

**vleue_navigator** provides:
- `VleueNavigatorPlugin` — registers NavMesh asset type
- `NavmeshUpdaterPlugin::<Collider, NavObstacle>` — watches for obstacle entities, auto-rebuilds navmesh
- `NavMeshSettings` component — spawns a navmesh entity with configurable outer boundary and agent radius
- `navmesh.path(from, to)` — synchronous A*-on-navmesh pathfinding

We add:
- `NavObstacle` marker on buildings/fortresses (tells the updater "this collider is an obstacle")
- `NavPath` component on units (stores waypoints from pathfinding)
- `compute_paths` system in `GameSet::Ai` (runs after `find_target`)
- Modified `unit_movement` in `GameSet::Movement` (follows waypoints instead of straight-line)

## Phase 1: Dependencies & Third-Party Setup

### Overview
Add vleue_navigator dependency, create third-party isolation module, define the NavObstacle marker.

### Changes Required:

#### 1. Cargo.toml
**File**: `Cargo.toml`
**Changes**: Add vleue_navigator with avian2d feature

```toml
[dependencies]
# ... existing deps ...
vleue_navigator = { version = "0.15", default-features = false, features = ["avian2d"] }
```

Note: disable default features (which include `debug-with-gizmos`) and enable only `avian2d`. Debug gizmos will be added in Phase 5 under the `dev` feature.

#### 2. Third-party isolation module
**File**: `src/third_party/vleue_navigator.rs` (new)
**Changes**: Plugin setup and NavObstacle marker

```rust
//! vleue_navigator navmesh configuration for pathfinding.

use avian2d::prelude::*;
use bevy::prelude::*;
use vleue_navigator::prelude::*;

/// Marker: this entity's `Collider` is a navmesh obstacle.
/// Add to buildings, fortresses — anything units must path around.
/// Do NOT add to units (dynamic), projectiles (kinematic), or non-blocking entities.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct NavObstacle;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<NavObstacle>();
    app.add_plugins((
        VleueNavigatorPlugin,
        NavmeshUpdaterPlugin::<Collider, NavObstacle>::default(),
    ));
}
```

#### 3. Third-party mod.rs
**File**: `src/third_party/mod.rs`
**Changes**: Add vleue_navigator module and re-export NavObstacle

```rust
//! Third-party plugin isolation.

mod avian;
mod vleue_navigator;

pub use avian::{CollisionLayer, surface_distance};
pub use self::vleue_navigator::NavObstacle;

pub fn plugin(app: &mut bevy::prelude::App) {
    app.add_plugins((avian::plugin, vleue_navigator::plugin));
}
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds with new dependency
- [x] `make check` passes (clippy + formatting)
- [x] `make test` — all existing tests pass

#### Manual Verification:
- [ ] Game starts without errors (vleue_navigator plugin initializes)

---

## Phase 2: NavMesh Entity & Obstacle Marking

### Overview
Spawn the NavMesh entity covering the battlefield. Add `NavObstacle` to buildings and fortresses so the updater auto-builds the navmesh from their colliders.

### Changes Required:

#### 1. Spawn NavMesh entity in battlefield setup
**File**: `src/gameplay/battlefield/renderer.rs`
**Changes**: Add navmesh spawn at the end of `spawn_battlefield`

Add import at top:
```rust
use crate::third_party::NavObstacle;
use crate::gameplay::units::UNIT_RADIUS;
use vleue_navigator::prelude::*;
```

Add NavObstacle to the player fortress spawn (line ~71):
```rust
    // Player fortress (blue)
    commands.spawn((
        // ... existing components ...
        NavObstacle,
    ));
```

Add NavObstacle to the enemy fortress spawn (line ~153):
```rust
    // Enemy fortress (red)
    commands.spawn((
        // ... existing components ...
        NavObstacle,
    ));
```

Add navmesh spawn at the end of `spawn_battlefield`:
```rust
    // NavMesh for unit pathfinding — covers the full battlefield.
    // Obstacles (buildings, fortresses with NavObstacle marker) are auto-carved by
    // NavmeshUpdaterPlugin. Agent radius ensures paths keep unit centers clear.
    commands.spawn((
        Name::new("Battlefield NavMesh"),
        NavMeshSettings {
            fixed: Triangulation::from_outer_edges(&[
                Vec2::new(0.0, 0.0),
                Vec2::new(BATTLEFIELD_WIDTH, 0.0),
                Vec2::new(BATTLEFIELD_WIDTH, BATTLEFIELD_HEIGHT),
                Vec2::new(0.0, BATTLEFIELD_HEIGHT),
            ]),
            agent_radius: UNIT_RADIUS,
            ..default()
        },
        NavMeshUpdateMode::Direct,
        DespawnOnExit(GameState::InGame),
    ));
```

#### 2. Add NavObstacle to buildings
**File**: `src/gameplay/building/placement.rs`
**Changes**: Add `NavObstacle` to the building spawn

Add import:
```rust
use crate::third_party::NavObstacle;
```

Add `NavObstacle` to the `commands.spawn((...))` tuple in `handle_building_placement` (around line 119):
```rust
    let mut entity_commands = commands.spawn((
        // ... existing components ...
        NavObstacle,
    ));
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `make check` passes
- [x] `make test` — all existing tests pass (NavObstacle is just a marker, no behavior change)

#### Manual Verification:
- [ ] Run game, place some buildings. No errors in console.
- [ ] If debug gizmos are enabled (Phase 6), the navmesh visualization should show holes where buildings/fortresses are.

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding.

---

## Phase 3: Convert Frame-Based Intervals to Time-Based (with Stagger)

### Overview
Convert `find_target`'s `RETARGET_INTERVAL_FRAMES` (frame-count-based) to a time-based slotted `RetargetTimer` resource. This fixes frame-rate-dependent behavior (0.17s at 60fps vs 0.33s at 30fps) while **preserving per-entity stagger** — entities are distributed across time slots so they don't all re-evaluate simultaneously. Also establishes the timer-resource pattern for the new `PathRefreshTimer`.

**Design**: A timer fires every `RETARGET_INTERVAL_SECS / RETARGET_SLOTS` seconds (0.015s). Each fire advances a slot counter (0→1→...→9→0). An entity evaluates when `entity_index % RETARGET_SLOTS == current_slot`. Result: each entity evaluates once per full 0.15s cycle, staggered across 10 time slots (same behavior as frame-based, but FPS-independent).

### Changes Required:

#### 1. Convert find_target to time-based with slotted stagger
**File**: `src/gameplay/ai.rs`
**Changes**: Replace `RETARGET_INTERVAL_FRAMES` + `Local<u32>` with a `RetargetTimer` resource containing a timer and slot counter

Replace the constants and system:
```rust
/// Seconds for a full retarget cycle across all slots.
/// Each entity re-evaluates once per cycle. Entities without a target
/// (or with a despawned target) always evaluate immediately.
const RETARGET_INTERVAL_SECS: f32 = 0.15;

/// Number of stagger slots. Entities are distributed across slots by their index.
/// Each timer tick evaluates one slot's worth of entities, spreading the load.
const RETARGET_SLOTS: u32 = 10;

/// Timer and slot state for staggered retargeting.
/// Entities re-evaluate targets in round-robin fashion: slot 0 first, then slot 1, etc.
/// The timer fires every `RETARGET_INTERVAL_SECS / RETARGET_SLOTS` seconds.
/// Exposed as a resource so tests can manipulate slot and timer state.
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct RetargetTimer {
    pub timer: Timer,
    pub current_slot: u32,
}

impl Default for RetargetTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(
                RETARGET_INTERVAL_SECS / RETARGET_SLOTS as f32,
                TimerMode::Repeating,
            ),
            current_slot: 0,
        }
    }
}

/// Finds the nearest valid target for each entity with `CurrentTarget`. Runs in `GameSet::Ai`.
///
/// Works for both units (with `Movement`) and static entities like fortresses (no `Movement`).
/// - Entities without a target evaluate every frame (so newly spawned units react instantly).
/// - Entities with a valid target re-evaluate on their stagger slot (once per
///   [`RETARGET_INTERVAL_SECS`] cycle, spread across [`RETARGET_SLOTS`] time intervals).
/// - Backtrack limit only applies to mobile entities (those with `Movement`).
pub fn find_target(
    time: Res<Time>,
    mut retarget_timer: ResMut<RetargetTimer>,
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

        // ... rest of targeting logic unchanged ...
    }
}
```

Register resource in plugin:
```rust
pub(super) fn plugin(app: &mut App) {
    app.init_resource::<RetargetTimer>();
    app.register_type::<RetargetTimer>();
    app.add_systems(
        Update,
        find_target.in_set(GameSet::Ai).run_if(gameplay_running),
    );
}
```

#### 2. Update find_target tests
**File**: `src/gameplay/ai.rs` (test section)
**Changes**: Update `create_ai_test_app` to init the resource. Add `set_retarget_for_entity` helper. Update `unit_switches_to_closer_target_on_retarget_frame` to use slot manipulation.

Update test app helper:
```rust
    fn create_ai_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<RetargetTimer>();
        app.add_systems(Update, find_target);
        app
    }
```

Add test helper to set the timer so the next update evaluates a specific entity's slot:
```rust
    /// Set the retarget timer so the NEXT `app.update()` will fire the slot
    /// that `entity` belongs to. Sets `current_slot` to entity's slot − 1
    /// and nearly expires the timer so the next tick advances into the entity's slot.
    fn set_retarget_for_entity(app: &mut App, entity: Entity) {
        let entity_slot = entity.index().index() % RETARGET_SLOTS;
        let prev_slot = if entity_slot == 0 {
            RETARGET_SLOTS - 1
        } else {
            entity_slot - 1
        };
        let mut timer = app.world_mut().resource_mut::<RetargetTimer>();
        timer.current_slot = prev_slot;
        let duration = timer.timer.duration();
        timer.timer.set_elapsed(duration - std::time::Duration::from_nanos(1));
    }
```

Update the retarget test:
```rust
    #[test]
    fn unit_switches_to_closer_target_on_retarget() {
        let mut app = create_ai_test_app();

        let player = spawn_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let enemy_far = spawn_unit(app.world_mut(), Team::Enemy, 300.0, 100.0);

        // First update gives a target (no target yet → evaluates immediately)
        app.update();
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, Some(enemy_far));

        // Spawn a closer enemy
        let enemy_near = spawn_unit(app.world_mut(), Team::Enemy, 150.0, 100.0);

        // Set timer to fire on the player's slot next update
        set_retarget_for_entity(&mut app, player);

        app.update();

        // Should have switched to the closer enemy
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, Some(enemy_near));
    }
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `make check` passes
- [x] `make test` — all AI tests pass with new timer-based slotted logic

---

## Phase 4: Pathfinding System

### Overview
Add `NavPath` component to units. Create `compute_paths` system that runs in `GameSet::Ai` after `find_target`, computing navmesh paths when targets change or periodically (time-based) to pick up navmesh rebuilds.

### Changes Required:

#### 1. New pathfinding module
**File**: `src/gameplay/units/pathfinding.rs` (new)
**Changes**: NavPath component, PathRefreshTimer resource, and compute_paths system

```rust
//! NavMesh pathfinding for units — computes waypoint paths around obstacles.

use bevy::prelude::*;
use vleue_navigator::prelude::*;

use super::Unit;
use crate::gameplay::CurrentTarget;

/// Seconds between periodic path recomputations for units that already have a path.
/// Picks up navmesh changes from building placement/destruction.
const PATH_REFRESH_INTERVAL_SECS: f32 = 0.5;

/// Timer controlling periodic path refresh for all units.
/// Exposed as a resource so tests can manipulate it.
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct PathRefreshTimer(pub Timer);

impl Default for PathRefreshTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            PATH_REFRESH_INTERVAL_SECS,
            TimerMode::Repeating,
        ))
    }
}

/// Waypoint path for a unit, computed from the NavMesh.
/// When present and non-empty, the movement system follows waypoints
/// instead of heading straight for the target.
#[derive(Component, Debug, Clone, Reflect, Default)]
#[reflect(Component)]
pub struct NavPath {
    /// World-space waypoints from navmesh pathfinding.
    pub waypoints: Vec<Vec2>,
    /// Index of the next waypoint to steer toward.
    pub current_index: usize,
    /// The target entity this path was computed for.
    /// Used to detect when the target changes.
    target: Option<Entity>,
}

impl NavPath {
    /// Replace the path with new waypoints for a new target.
    pub fn set(&mut self, waypoints: Vec<Vec2>, target: Option<Entity>) {
        self.waypoints = waypoints;
        self.current_index = 0;
        self.target = target;
    }

    /// Clear the path (no waypoints).
    pub fn clear(&mut self) {
        self.waypoints.clear();
        self.current_index = 0;
        self.target = None;
    }

    /// Get the current waypoint, if any remain.
    #[must_use]
    pub fn current_waypoint(&self) -> Option<Vec2> {
        self.waypoints.get(self.current_index).copied()
    }

    /// Advance to the next waypoint. Returns true if there are more waypoints.
    pub fn advance(&mut self) -> bool {
        self.current_index += 1;
        self.current_index < self.waypoints.len()
    }

    /// Whether this path needs recomputation for the given target.
    #[must_use]
    pub fn needs_recompute(&self, target: Option<Entity>) -> bool {
        self.target != target
    }
}

/// Computes navmesh paths for units whose target changed or whose path needs refreshing.
/// Runs in `GameSet::Ai` after `find_target`.
pub(super) fn compute_paths(
    time: Res<Time>,
    mut refresh_timer: ResMut<PathRefreshTimer>,
    mut units: Query<(&CurrentTarget, &GlobalTransform, &mut NavPath), With<Unit>>,
    targets: Query<&GlobalTransform>,
    navmeshes: Res<Assets<NavMesh>>,
    navmesh_query: Option<Single<(&ManagedNavMesh, &NavMeshStatus)>>,
) {
    let Some(inner) = navmesh_query else {
        return;
    };
    let (managed, status) = *inner;
    if *status != NavMeshStatus::Built {
        return;
    }
    let Some(navmesh) = navmeshes.get(managed) else {
        return;
    };

    refresh_timer.0.tick(time.delta());
    let refresh_due = refresh_timer.0.just_finished();

    for (current_target, transform, mut nav_path) in &mut units {
        let target_changed = nav_path.needs_recompute(current_target.0);

        // Skip recomputation if target hasn't changed and no periodic refresh
        if !target_changed && !refresh_due {
            continue;
        }

        let Some(target_entity) = current_target.0 else {
            nav_path.clear();
            continue;
        };

        let Ok(target_transform) = targets.get(target_entity) else {
            nav_path.clear();
            continue;
        };

        let from = transform.translation().xy();
        let to = target_transform.translation().xy();

        if let Some(path) = navmesh.path(from, to) {
            nav_path.set(path.path, current_target.0);
        } else {
            // No valid path — clear waypoints, movement falls back to direct
            nav_path.set(Vec::new(), current_target.0);
        }
    }
}
```

#### 2. Register in units plugin
**File**: `src/gameplay/units/mod.rs`
**Changes**: Add module and register system + resources

Add module declaration:
```rust
pub mod pathfinding;
```

In `plugin()` function, register types and add system:
```rust
    app.register_type::<pathfinding::NavPath>()
        .register_type::<pathfinding::PathRefreshTimer>()
        .init_resource::<pathfinding::PathRefreshTimer>();

    app.add_systems(
        Update,
        pathfinding::compute_paths
            .in_set(GameSet::Ai)
            .after(crate::gameplay::ai::find_target)
            .run_if(gameplay_running),
    );
```

#### 3. Add NavPath to unit spawn
**File**: `src/gameplay/units/mod.rs`
**Changes**: Add `NavPath::default()` to the `spawn_unit` function

In `spawn_unit` (line ~98), add to the component tuple:
```rust
    commands
        .spawn((
            // ... existing components ...
            pathfinding::NavPath::default(),
        ))
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `make check` passes
- [x] `make test` — all existing tests pass

#### Manual Verification:
- [ ] Run game, observe no errors. Units still move (paths are computed but movement system doesn't use them yet — that's Phase 5).

**Implementation Note**: After completing this phase, pause for verification before proceeding.

---

## Phase 5: Movement System Update

### Overview
Modify `unit_movement` to follow `NavPath` waypoints instead of heading straight for the target. When waypoints are exhausted or the unit is within attack range, fall back to existing direct targeting.

### Changes Required:

#### 1. Update movement system
**File**: `src/gameplay/units/movement.rs`
**Changes**: Integrate NavPath waypoint following

```rust
//! Unit movement toward current target, following NavPath waypoints around obstacles.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::pathfinding::NavPath;
use super::{CombatStats, CurrentTarget, Movement, Unit};
use crate::third_party::surface_distance;

/// Distance threshold for reaching a waypoint — when the unit's center
/// is within this distance of a waypoint, advance to the next one.
const WAYPOINT_REACHED_DISTANCE: f32 = 8.0;

/// Sets unit `LinearVelocity` toward their current waypoint or target.
///
/// If the unit has a `NavPath` with remaining waypoints, steers toward
/// the next waypoint. When close enough, advances to the next waypoint.
/// When all waypoints are consumed (or no path exists), steers directly
/// toward the `CurrentTarget` (existing straight-line behavior).
///
/// Always checks attack range against the actual target — if in range,
/// stops regardless of remaining waypoints.
///
/// Runs in `GameSet::Movement`.
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
) {
    for (current_target, movement, stats, global_transform, unit_collider, mut velocity, mut nav_path) in
        &mut units
    {
        let Some(target_entity) = current_target.0 else {
            velocity.0 = Vec2::ZERO;
            continue;
        };
        let Ok((target_pos, target_collider)) = targets.get(target_entity) else {
            velocity.0 = Vec2::ZERO;
            continue;
        };

        let current_xy = global_transform.translation().xy();
        let target_xy = target_pos.translation().xy();
        let distance_to_target = surface_distance(unit_collider, current_xy, target_collider, target_xy);

        // Already within attack range — stop
        if distance_to_target <= stats.range {
            velocity.0 = Vec2::ZERO;
            continue;
        }

        // Determine steering target: next waypoint or direct to target
        let steer_toward = if let Some(waypoint) = nav_path.current_waypoint() {
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
        } else {
            // No path — steer directly to target (fallback)
            target_xy
        };

        // Compute velocity toward steering target
        let diff = steer_toward - current_xy;
        let dist = diff.length();
        if dist < f32::EPSILON {
            velocity.0 = Vec2::ZERO;
            continue;
        }

        let direction = diff / dist;
        velocity.0 = direction * movement.speed;
    }
}
```

#### 2. Update movement tests
**File**: `src/gameplay/units/movement.rs` (test section)
**Changes**: Add `NavPath::default()` to test unit spawns and add pathfinding-specific tests

Update `spawn_unit_at` helper to include NavPath:
```rust
    fn spawn_unit_at(world: &mut World, x: f32, speed: f32, target: Option<Entity>) -> Entity {
        let stats = unit_stats(UnitType::Soldier);
        world
            .spawn((
                Unit,
                CurrentTarget(target),
                Movement { speed },
                CombatStats {
                    damage: stats.damage,
                    attack_speed: stats.attack_speed,
                    range: stats.attack_range,
                },
                Transform::from_xyz(x, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, 100.0, 0.0)),
                Collider::circle(UNIT_RADIUS),
                LinearVelocity::ZERO,
                NavPath::default(),
            ))
            .id()
    }
```

Add new tests for waypoint following:
```rust
    #[test]
    fn unit_follows_waypoint_instead_of_direct() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        // Set a path that goes up then right (around an obstacle)
        let mut nav_path = app.world_mut().get_mut::<NavPath>(unit).unwrap();
        nav_path.set(
            vec![Vec2::new(100.0, 300.0), Vec2::new(500.0, 300.0), Vec2::new(500.0, 100.0)],
            Some(target),
        );

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        // Should head toward first waypoint (100, 300) = upward from (100, 100)
        assert!(
            velocity.y > 0.0,
            "Unit should move upward toward first waypoint, got vy={}",
            velocity.y
        );
        assert!(
            velocity.x.abs() < 0.1,
            "Unit should not move horizontally toward first waypoint, got vx={}",
            velocity.x
        );
    }

    #[test]
    fn unit_advances_to_next_waypoint() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        // Place unit very close to the first waypoint
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        // Set path with first waypoint very close to current position
        let mut nav_path = app.world_mut().get_mut::<NavPath>(unit).unwrap();
        nav_path.set(
            vec![Vec2::new(102.0, 100.0), Vec2::new(500.0, 100.0)],
            Some(target),
        );

        app.update();

        // Should have advanced past the first waypoint (within WAYPOINT_REACHED_DISTANCE)
        let nav_path = app.world().get::<NavPath>(unit).unwrap();
        assert!(
            nav_path.current_index >= 1,
            "Should have advanced past first waypoint, index={}",
            nav_path.current_index
        );
    }

    #[test]
    fn unit_falls_back_to_direct_when_no_path() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));
        // NavPath is default (empty) — should go direct to target

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        assert!(
            velocity.x > 0.0,
            "Unit with no path should move directly toward target, got vx={}",
            velocity.x
        );
    }

    #[test]
    fn unit_stops_at_range_even_with_remaining_waypoints() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        // Place unit within attack range of target
        let unit = spawn_unit_at(
            app.world_mut(),
            500.0 - stats.attack_range + 1.0,
            stats.move_speed,
            Some(target),
        );

        // Give it a path with remaining waypoints
        let mut nav_path = app.world_mut().get_mut::<NavPath>(unit).unwrap();
        nav_path.set(
            vec![Vec2::new(600.0, 100.0), Vec2::new(700.0, 100.0)],
            Some(target),
        );

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Unit in attack range should stop even with waypoints, got {:?}",
            velocity.0
        );
    }
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds
- [x] `make check` passes
- [x] `make test` — all tests pass (including new waypoint tests)

#### Manual Verification:
- [x] Run game, place buildings in a wall pattern with one gap
- [x] Observe enemy units routing through the gap instead of getting stuck on buildings
- [x] Verify units still reach and attack the player fortress
- [x] Verify units stop at attack range of their targets
- [x] Destroy a building in the wall — units update their route through the new opening

**Implementation Note**: This is the core behavior change. Thorough manual testing is important here.

---

## Phase 6: Debug Visualization (dev feature only)

### Overview
Add navmesh and path debug gizmos under the `dev` feature flag for development-time visualization.

### Changes Required:

#### 1. Add debug gizmo feature to Cargo.toml
**File**: `Cargo.toml`
**Changes**: Add `debug-with-gizmos` feature to vleue_navigator when dev feature is active

```toml
[dependencies]
vleue_navigator = { version = "0.15", default-features = false, features = ["avian2d"] }

[features]
default = ["dev"]
dev = ["bevy/dynamic_linking", "vleue_navigator/debug-with-gizmos"]
```

#### 2. Add debug systems to dev_tools
**File**: `src/dev_tools/mod.rs`
**Changes**: Add navmesh debug visualization and unit path gizmos

```rust
use vleue_navigator::prelude::NavMeshesDebug;

// In the plugin function, add:
app.insert_resource(NavMeshesDebug(Color::srgba(1.0, 0.0, 0.0, 0.15)));

// Add path debug system:
app.add_systems(Update, debug_draw_unit_paths.run_if(crate::gameplay_running));
```

Path debug system:
```rust
fn debug_draw_unit_paths(
    units: Query<(&GlobalTransform, &crate::gameplay::units::pathfinding::NavPath), With<crate::gameplay::units::Unit>>,
    mut gizmos: Gizmos,
) {
    for (transform, nav_path) in &units {
        if nav_path.waypoints.is_empty() {
            continue;
        }
        let mut points = vec![transform.translation().xy()];
        for &wp in &nav_path.waypoints[nav_path.current_index..] {
            points.push(wp);
        }
        if points.len() >= 2 {
            gizmos.linestrip_2d(points, Color::srgb(1.0, 1.0, 0.0));
        }
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` succeeds (with and without `dev` feature)
- [x] `make check` passes
- [x] `make test` passes

#### Manual Verification:
- [x] Run game with dev feature — see red-tinted navmesh overlay on the battlefield
- [x] Navmesh shows holes where buildings and fortresses are
- [x] See yellow lines showing unit paths
- [x] Place a building — navmesh updates visually (hole appears)

---

## Phase 7: Tests

### Overview
Add targeted tests for the pathfinding system and integration tests for the full pathfinding + movement pipeline.

### Changes Required:

#### 1. NavPath unit tests
**File**: `src/gameplay/units/pathfinding.rs`
**Changes**: Add tests for NavPath component logic

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_path_default_is_empty() {
        let path = NavPath::default();
        assert!(path.waypoints.is_empty());
        assert_eq!(path.current_index, 0);
        assert!(path.target.is_none());
        assert!(path.current_waypoint().is_none());
    }

    #[test]
    fn nav_path_set_replaces_waypoints() {
        let mut path = NavPath::default();
        let entity = Entity::from_bits(42);
        path.set(vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 4.0)], Some(entity));

        assert_eq!(path.waypoints.len(), 2);
        assert_eq!(path.current_index, 0);
        assert_eq!(path.target, Some(entity));
        assert_eq!(path.current_waypoint(), Some(Vec2::new(1.0, 2.0)));
    }

    #[test]
    fn nav_path_advance_increments_index() {
        let mut path = NavPath::default();
        path.set(vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 4.0)], None);

        assert!(path.advance()); // Advance to index 1
        assert_eq!(path.current_waypoint(), Some(Vec2::new(3.0, 4.0)));

        assert!(!path.advance()); // No more waypoints
        assert!(path.current_waypoint().is_none());
    }

    #[test]
    fn nav_path_clear_resets_everything() {
        let mut path = NavPath::default();
        let entity = Entity::from_bits(42);
        path.set(vec![Vec2::new(1.0, 2.0)], Some(entity));
        path.clear();

        assert!(path.waypoints.is_empty());
        assert_eq!(path.current_index, 0);
        assert!(path.target.is_none());
    }

    #[test]
    fn nav_path_needs_recompute_detects_target_change() {
        let mut path = NavPath::default();
        let entity_a = Entity::from_bits(42);
        let entity_b = Entity::from_bits(99);

        path.set(vec![Vec2::ZERO], Some(entity_a));

        assert!(!path.needs_recompute(Some(entity_a))); // Same target
        assert!(path.needs_recompute(Some(entity_b)));  // Different target
        assert!(path.needs_recompute(None));             // No target
    }
}
```

#### 2. Integration tests for compute_paths
**File**: `src/gameplay/units/pathfinding.rs`
**Changes**: Add integration tests (these require NavMesh setup which may be complex in test apps — can be deferred to manual verification if NavMesh asset creation in tests is impractical)

Note: Integration testing of `compute_paths` requires a fully-built NavMesh asset. If `vleue_navigator` provides a way to construct NavMesh programmatically in tests (via `NavMesh::from_edge_and_obstacles()`), add integration tests. Otherwise, the pathfinding behavior is verified through manual testing in Phase 4.

### Success Criteria:

#### Automated Verification:
- [x] `make test` — all new and existing tests pass (192 unit + 2 integration)
- [x] `make check` passes
- [x] Test coverage maintained or increased

---

## Testing Strategy

### Unit Tests:
- `NavPath` component: set, clear, advance, current_waypoint, needs_recompute
- Movement with waypoints: unit follows waypoints, advances, falls back to direct
- Movement stops at range even with remaining waypoints
- Existing movement tests still pass (with NavPath::default())

### Integration Tests:
- compute_paths system: may require NavMesh construction in test (evaluate feasibility)
- Full pipeline: target → path → movement (complex, may be manual-only)

### Manual Testing Steps:
1. Start game, enter InGame state — verify no errors
2. Place 3 buildings in a horizontal wall (e.g., rows 3-5, col 3) with a gap in row 4
3. Verify enemy units route through the gap instead of getting stuck
4. Destroy the middle building — verify units take the now-shorter direct route
5. Place buildings to completely block a row — verify units go around via adjacent rows
6. Verify units in open combat zone (no buildings in the way) still move directly
7. Verify units stop at attack range and engage targets correctly
8. Verify performance is acceptable with 20+ units pathfinding simultaneously

## Performance Considerations

- **NavMesh size**: The battlefield is 5248×640 pixels. With only buildings and fortresses as obstacles (max ~60 buildings + 2 fortresses), the navmesh is small.
- **Pathfinding frequency**: Paths recomputed on target change + every 0.5s (time-based timer). Not every frame for every unit.
- **NavMesh rebuild**: `Direct` mode rebuilds on every obstacle change. Building placement is infrequent, so this is fine. If performance becomes an issue, switch to `Debounced(0.5)`.
- **Path computation**: vleue_navigator uses Polyanya (optimal navmesh pathfinding). Single path queries on our small mesh should be sub-millisecond.

## References

- Linear ticket: [GAM-12](https://linear.app/tayhu-games/issue/GAM-12/unit-pathfinding-based-on-physics-objects)
- Follow-up: [GAM-32](https://linear.app/tayhu-games/issue/GAM-32/unit-to-unit-local-avoidance-orca-steering) (ORCA local avoidance)
- vleue_navigator: [GitHub](https://github.com/vleue/vleue_navigator) | [docs.rs](https://docs.rs/vleue_navigator/0.15.0)
- Current movement system: `src/gameplay/units/movement.rs:13`
- Current AI system: `src/gameplay/ai.rs:24`
- Building placement: `src/gameplay/building/placement.rs:119`
- Fortress spawning: `src/gameplay/battlefield/renderer.rs:71,153`

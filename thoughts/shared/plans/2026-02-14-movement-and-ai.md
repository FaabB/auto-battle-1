# Ticket 0004: Movement & AI — Implementation Plan

## Overview

Add the core unit AI loop: find target → move toward target → stop at attack range. Also adds a debug enemy spawner (E key) for testing, and makes buildings/fortresses targetable.

## Current State Analysis

- Units spawn from barracks (`building/production.rs:29-45`) with `Unit`, `Team::Player`, `Health`, `CombatStats`, `Movement` — no targeting or movement yet
- `UnitAssets` has only `player_mesh` and `player_material` — needs enemy material
- Fortresses exist at: player x=64.0, enemy x=5184.0 (`battlefield/renderer.rs:46-97`) — no `Team` component
- Buildings spawned without `Team` component (`building/placement.rs:91-103`)
- `GameSet` ordering already defined: `Ai → Movement` (`lib.rs:47-49`)
- `dev_tools/mod.rs` is an empty stub ready for debug spawner
- `GlobalTransform::translation()` returns `Vec3` (verified against Bevy 0.18 source)

## Desired End State

- Player units spawn from barracks and walk rightward toward nearest enemy (or enemy fortress)
- Press E → 3 red enemy units appear in the right combat zone
- Enemy units walk leftward toward nearest player target (units, buildings, fortress)
- Units stop within attack range of their target
- Units re-target when their current target is despawned
- Units don't backtrack more than 2 cells behind their current position

### Key Discoveries:
- `GlobalTransform` and `Transform` are different components — no query conflicts (`battlefield/camera.rs` already uses this pattern)
- Fortresses have `Transform` + auto-added `GlobalTransform` from `Sprite` requirement
- `GameSet::Ai` and `GameSet::Movement` are already chained in order

## What We're NOT Doing

- Combat/damage (Ticket 5)
- Health bars (Ticket 5)
- Wave system (Ticket 7) — debug spawner is a temporary placeholder
- Fortress Health component (Ticket 8)
- Unit pathfinding or collision avoidance — direct movement toward target

## Implementation Approach

Two new components (`Target` marker, `CurrentTarget` tracker), two new systems (`unit_find_target`, `unit_movement`), one debug spawner. The `Target` marker goes on all targetable entities (units, buildings, fortresses). `CurrentTarget` goes on units and persists across frames — only recalculated when the target entity is dead/invalid.

---

## Phase 1: Component & Resource Foundation

### Overview
Add new components, extend UnitAssets with enemy material, and update existing spawn code to include `Target`/`Team` on all targetable entities.

### Changes Required:

#### 1. New components and constants in units module
**File**: `src/gameplay/units/mod.rs`

Add enemy color constant:
```rust
/// Enemy unit color (red).
const ENEMY_UNIT_COLOR: Color = Color::srgb(0.8, 0.2, 0.2);
```

Add backtrack constant:
```rust
/// Maximum distance (pixels) a unit will backtrack to chase a target behind it.
/// 2 cells = 128 pixels.
pub const BACKTRACK_DISTANCE: f32 = 2.0 * crate::gameplay::battlefield::CELL_SIZE;
```

Add `Target` marker component:
```rust
/// Marker: this entity can be targeted by units.
/// Placed on units, buildings, and fortresses.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Target;
```

Add `CurrentTarget` component:
```rust
/// Tracks the entity this unit is currently moving toward / attacking.
/// Updated by the AI system; read by movement and (future) combat systems.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct CurrentTarget(pub Option<Entity>);
```

Register both in plugin:
```rust
app.register_type::<Target>()
    .register_type::<CurrentTarget>();
```

#### 2. Extend UnitAssets with enemy material
**File**: `src/gameplay/units/mod.rs`

Update `UnitAssets`:
```rust
pub struct UnitAssets {
    pub mesh: Handle<Mesh>,                    // renamed from player_mesh (shared circle)
    pub player_material: Handle<ColorMaterial>,
    pub enemy_material: Handle<ColorMaterial>,  // NEW
}
```

Update `setup_unit_assets`:
```rust
fn setup_unit_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.insert_resource(UnitAssets {
        mesh: meshes.add(Circle::new(UNIT_RADIUS)),
        player_material: materials.add(PLAYER_UNIT_COLOR),
        enemy_material: materials.add(ENEMY_UNIT_COLOR),
    });
}
```

#### 3. Update production.rs for renamed field + new components
**File**: `src/gameplay/building/production.rs`

- Change `unit_assets.player_mesh.clone()` → `unit_assets.mesh.clone()` (line 41)
- Add `Target` and `CurrentTarget(None)` to the spawn bundle (after `Team::Player`)

#### 4. Add Team and Target to buildings
**File**: `src/gameplay/building/placement.rs`

In `handle_building_placement` (line 91-103), add to the spawn bundle:
```rust
commands.spawn((
    Building { building_type, grid_col: col, grid_row: row },
    Team::Player,    // NEW
    Target,          // NEW
    Sprite::from_color(building_color(building_type), Vec2::splat(BUILDING_SPRITE_SIZE)),
    Transform::from_xyz(world_x, world_y, Z_BUILDING),
    DespawnOnExit(GameState::InGame),
));
```

Import `Team` and `Target` from `crate::gameplay::units`.

#### 5. Add Team and Target to fortresses
**File**: `src/gameplay/battlefield/renderer.rs`

Player fortress spawn (line 46-55) — add `Team::Player` and `Target`:
```rust
commands.spawn((
    PlayerFortress,
    Team::Player,    // NEW
    Target,          // NEW
    Sprite::from_color(PLAYER_FORT_COLOR, fortress_size),
    Transform::from_xyz(...),
    DespawnOnExit(GameState::InGame),
));
```

Enemy fortress spawn (line 88-97) — add `Team::Enemy` and `Target`:
```rust
commands.spawn((
    EnemyFortress,
    Team::Enemy,     // NEW
    Target,          // NEW
    Sprite::from_color(ENEMY_FORT_COLOR, fortress_size),
    Transform::from_xyz(...),
    DespawnOnExit(GameState::InGame),
));
```

Import `Team` and `Target` from `crate::gameplay::units`.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (clippy + type checking)
- [ ] `make test` passes (existing tests still work with renamed field)
- [ ] `make build` succeeds

#### Manual Verification:
- [ ] Game runs normally, no visual changes yet

**Pause here for verification before proceeding to Phase 2.**

---

## Phase 2: AI System (Target Finding)

### Overview
New `units/ai.rs` with `unit_find_target` system. Finds nearest enemy `Target` entity, stores in `CurrentTarget`. Respects backtrack limit. Only re-evaluates when current target is invalid.

### Changes Required:

#### 1. New AI module
**File**: `src/gameplay/units/ai.rs` (NEW)

```rust
//! Unit AI: target selection.

use bevy::prelude::*;

use super::{CurrentTarget, Target, Team, Unit, BACKTRACK_DISTANCE};
use crate::gameplay::battlefield::CELL_SIZE;

/// Finds the nearest valid target for each unit. Runs in `GameSet::Ai`.
///
/// Priority:
/// 1. Keep current target if still alive
/// 2. Find nearest enemy entity with `Target` marker (within backtrack limit)
/// 3. If nothing found, `CurrentTarget` is set to `None`
pub(super) fn unit_find_target(
    mut units: Query<(Entity, &Team, &GlobalTransform, &mut CurrentTarget), With<Unit>>,
    all_targets: Query<(Entity, &Team, &GlobalTransform), With<Target>>,
) {
    for (entity, team, transform, mut current_target) in &mut units {
        // Keep current target if still alive
        if let Some(target_entity) = current_target.0 {
            if all_targets.get(target_entity).is_ok() {
                continue;
            }
            // Target gone — clear and re-evaluate
            current_target.0 = None;
        }

        let my_pos = transform.translation().xy();
        let opposing_team = match team {
            Team::Player => Team::Enemy,
            Team::Enemy => Team::Player,
        };

        // Find nearest enemy target within backtrack limit
        let mut nearest: Option<(Entity, f32)> = None;
        for (candidate, candidate_team, candidate_pos) in &all_targets {
            if candidate == entity || *candidate_team != opposing_team {
                continue;
            }
            let candidate_xy = candidate_pos.translation().xy();

            // Backtrack filter: ignore targets too far behind
            let behind = match team {
                Team::Player => my_pos.x - candidate_xy.x,
                Team::Enemy => candidate_xy.x - my_pos.x,
            };
            if behind > BACKTRACK_DISTANCE {
                continue;
            }

            let dist = my_pos.distance(candidate_xy);
            if nearest.map_or(true, |(_, d)| dist < d) {
                nearest = Some((candidate, dist));
            }
        }

        current_target.0 = nearest.map(|(e, _)| e);
    }
}
```

**Query conflict analysis**: `units` writes `CurrentTarget`, `all_targets` reads `Team` + `GlobalTransform`. No overlapping mutable component access — safe.

**Backtrack behavior**: Player units (moving right) ignore targets more than 128px to their left. Enemy units (moving left) ignore targets more than 128px to their right. Fortresses are always "ahead" so they pass the filter naturally.

#### 2. Register AI module in units plugin
**File**: `src/gameplay/units/mod.rs`

Add module declaration:
```rust
mod ai;
```

Register system in plugin:
```rust
app.add_systems(
    Update,
    ai::unit_find_target
        .in_set(crate::GameSet::Ai)
        .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
);
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes
- [ ] `make build` succeeds

#### Manual Verification:
- [ ] No visible change yet (movement not implemented), but no panics or errors

**Pause here for verification before proceeding to Phase 3.**

---

## Phase 3: Movement System

### Overview
New `units/movement.rs` with `unit_movement` system. Reads `CurrentTarget`, moves unit toward target position, stops at attack range.

### Changes Required:

#### 1. New movement module
**File**: `src/gameplay/units/movement.rs` (NEW)

```rust
//! Unit movement toward current target.

use bevy::prelude::*;

use super::{CombatStats, CurrentTarget, Movement, Unit};

/// Moves units toward their `CurrentTarget`, stopping at attack range.
/// Runs in `GameSet::Movement`.
pub(super) fn unit_movement(
    time: Res<Time>,
    mut units: Query<(&CurrentTarget, &Movement, &CombatStats, &mut Transform), With<Unit>>,
    positions: Query<&GlobalTransform>,
) {
    for (current_target, movement, stats, mut transform) in &mut units {
        let Some(target_entity) = current_target.0 else {
            continue;
        };
        let Ok(target_pos) = positions.get(target_entity) else {
            continue;
        };

        let target_xy = target_pos.translation().xy();
        let current_xy = transform.translation.xy();
        let diff = target_xy - current_xy;
        let distance = diff.length();

        // Already within attack range — stop
        if distance <= stats.range {
            continue;
        }

        let direction = diff / distance; // normalized
        let move_amount = movement.speed * time.delta_secs();
        let max_move = distance - stats.range;

        if move_amount >= max_move {
            // Would overshoot — snap to attack range distance
            transform.translation.x = target_xy.x - direction.x * stats.range;
            transform.translation.y = target_xy.y - direction.y * stats.range;
        } else {
            transform.translation.x += direction.x * move_amount;
            transform.translation.y += direction.y * move_amount;
        }
    }
}
```

**Query conflict analysis**: `units` writes `Transform` on `With<Unit>`, `positions` reads `GlobalTransform` on any entity. Different components — no conflict.

**Edge cases handled**:
- `CurrentTarget(None)` → skip (no target)
- Target entity despawned → `positions.get()` returns `Err` → skip
- Already at range → don't move
- Would overshoot → snap to range distance

#### 2. Register movement module in units plugin
**File**: `src/gameplay/units/mod.rs`

Add module declaration:
```rust
mod movement;
```

Register system in plugin:
```rust
app.add_systems(
    Update,
    movement::unit_movement
        .in_set(crate::GameSet::Movement)
        .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
);
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes
- [ ] `make build` succeeds

#### Manual Verification:
- [ ] Place a barracks → units spawn and walk rightward toward enemy fortress
- [ ] Units stop at the enemy fortress (within attack range distance)
- [ ] Units don't overshoot or jitter

**Pause here for verification before proceeding to Phase 4.**

---

## Phase 4: Debug Enemy Spawner

### Overview
Add debug enemy spawner to `dev_tools/`. Press E → spawns 3 red enemy units in the right combat zone. Feature-gated on `dev`.

### Changes Required:

#### 1. Update dev_tools plugin
**File**: `src/dev_tools/mod.rs`

```rust
//! Development tools — only included with `cargo run --features dev`.
//!
//! Debug overlays, test spawners, and inspector setup go here.
//! This module is stripped from release builds.

use bevy::prelude::*;

use crate::gameplay::battlefield::{
    COMBAT_ZONE_COLS, COMBAT_ZONE_START_COL, col_to_world_x, row_to_world_y,
};
use crate::gameplay::units::{
    CombatStats, CurrentTarget, Health, Movement, Target, Team, Unit, UnitAssets,
    SOLDIER_ATTACK_RANGE, SOLDIER_ATTACK_SPEED, SOLDIER_DAMAGE, SOLDIER_HEALTH,
    SOLDIER_MOVE_SPEED,
};
use crate::menus::Menu;
use crate::screens::GameState;
use crate::Z_UNIT;

/// Number of enemies spawned per E key press.
const ENEMIES_PER_SPAWN: u32 = 3;

/// Column where debug enemies spawn (near enemy fortress side of combat zone).
const DEBUG_SPAWN_COL: u16 = COMBAT_ZONE_START_COL + COMBAT_ZONE_COLS - 5; // col 75

fn debug_spawn_enemies(
    keyboard: Res<ButtonInput<KeyCode>>,
    unit_assets: Res<UnitAssets>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }

    for i in 0..ENEMIES_PER_SPAWN {
        // Spread across rows: 2, 5, 8
        let row = i as u16 * 3 + 2;
        let spawn_x = col_to_world_x(DEBUG_SPAWN_COL);
        let spawn_y = row_to_world_y(row);

        commands.spawn((
            Unit,
            Team::Enemy,
            Target,
            CurrentTarget(None),
            Health::new(SOLDIER_HEALTH),
            CombatStats {
                damage: SOLDIER_DAMAGE,
                attack_speed: SOLDIER_ATTACK_SPEED,
                range: SOLDIER_ATTACK_RANGE,
            },
            Movement {
                speed: SOLDIER_MOVE_SPEED,
            },
            Mesh2d(unit_assets.mesh.clone()),
            MeshMaterial2d(unit_assets.enemy_material.clone()),
            Transform::from_xyz(spawn_x, spawn_y, Z_UNIT),
            DespawnOnExit(GameState::InGame),
        ));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        debug_spawn_enemies
            .in_set(crate::GameSet::Input)
            .run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
    );
}
```

**Spawn positions**: Column 75 (near enemy fortress), rows 2/5/8 (spread vertically). Using `col_to_world_x(75)` = 4832px, well inside the combat zone.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes
- [ ] `make build` succeeds

#### Manual Verification:
- [ ] Press E → 3 red circles appear on the right side of the combat zone
- [ ] Red enemies walk leftward toward player units/buildings/fortress
- [ ] Player units and enemy units walk toward each other and stop when close
- [ ] If all enemies are killed (future — can't die yet), player units resume walking right
- [ ] Press E multiple times → more enemies spawn

**Pause here for verification before proceeding to Phase 5.**

---

## Phase 5: Tests

### Overview
Integration tests for AI targeting, movement, and debug spawner. Targets 90% coverage.

### Changes Required:

#### 1. AI system tests
**File**: `src/gameplay/units/ai.rs` — add `#[cfg(test)] mod tests`

Tests:
- `unit_targets_nearest_enemy` — spawn player unit + 2 enemy units at different distances → verify CurrentTarget points to nearer enemy
- `unit_targets_fortress_when_no_enemies` — spawn player unit + enemy fortress with Target+Team::Enemy → verify CurrentTarget is fortress entity
- `unit_retargets_when_target_despawned` — set CurrentTarget to an entity, despawn it, run AI → verify new target found
- `unit_keeps_valid_target` — set CurrentTarget to valid entity, run AI → verify target unchanged
- `unit_respects_backtrack_limit` — spawn enemy behind player unit beyond backtrack distance → verify not targeted
- `unit_targets_building` — spawn enemy unit + player building with Target+Team::Player → verify enemy targets building

#### 2. Movement system tests
**File**: `src/gameplay/units/movement.rs` — add `#[cfg(test)] mod tests`

Tests:
- `unit_moves_toward_target` — place unit at x=100, target at x=500 → run update → verify unit.x increased
- `unit_stops_at_attack_range` — place unit within attack range of target → verify no movement
- `unit_no_movement_without_target` — set CurrentTarget(None) → verify position unchanged
- `unit_snaps_to_range_on_overshoot` — place unit very close to target (just outside range) with high speed → verify snaps to range distance

#### 3. Debug spawner tests
**File**: `src/dev_tools/mod.rs` — add `#[cfg(test)] mod tests`

Tests:
- `pressing_e_spawns_enemy_units` — press E → verify 3 Unit+Team::Enemy entities spawned
- `enemies_have_correct_components` — verify spawned enemies have Target, CurrentTarget, Health, CombatStats, Movement

#### 4. Test helpers
Movement tests need a helper app with:
- `create_base_test_app_no_input()` (no InputPlugin — for deterministic time deltas)
- `ButtonInput<KeyCode>` init
- GameSet configured (at least Ai, Movement chained)
- Battlefield + units plugins added
- Mesh/ColorMaterial asset stores

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — all new tests green
- [ ] No decrease in existing test coverage

---

## Verified API Patterns (Bevy 0.18)

These were verified against the actual crate source in `~/.cargo/registry/src/`:

- `GlobalTransform::translation()` returns `Vec3` (not `Vec3A`) — `global_transform.rs:214`
- `GlobalTransform` is auto-added when `Transform` is spawned — `#[require(GlobalTransform)]` on `Transform`
- `Query<&GlobalTransform>` + `Query<&mut Transform, With<Unit>>` — no conflict (different components)
- `Vec3::xy()` returns `Vec2` — standard glam method
- `Vec2::distance(other)` returns `f32` — standard glam method
- 1-frame lag between `Transform` mutation and `GlobalTransform` update (propagation runs in `PostUpdate`) — acceptable for gameplay

## File Change Summary

| File | Change |
|------|--------|
| `src/gameplay/units/mod.rs` | Add Target, CurrentTarget, ENEMY_UNIT_COLOR, BACKTRACK_DISTANCE; update UnitAssets; register types; declare ai + movement submodules |
| `src/gameplay/units/ai.rs` | **NEW** — `unit_find_target` system |
| `src/gameplay/units/movement.rs` | **NEW** — `unit_movement` system |
| `src/gameplay/building/production.rs` | Add Target + CurrentTarget to spawn; rename player_mesh → mesh |
| `src/gameplay/building/placement.rs` | Add Team::Player + Target to building spawn |
| `src/gameplay/battlefield/renderer.rs` | Add Team + Target to both fortress spawns |
| `src/dev_tools/mod.rs` | Add debug_spawn_enemies system |

## References

- Original ticket: `thoughts/shared/tickets/2026-02-08-0004-movement-and-ai.md`
- Research: `thoughts/shared/research/2026-02-04-tano-style-game-research.md`
- Dependent tickets: 0005 (combat uses CurrentTarget), 0007 (replaces debug spawner), 0008 (fortress Health)

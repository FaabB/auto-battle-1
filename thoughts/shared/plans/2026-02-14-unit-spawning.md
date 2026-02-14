# Ticket 0003: Unit Spawning — Implementation Plan

## Overview

Barracks buildings produce soldier units over time, rendered as green circles. This is the first ticket where the game "comes alive" — buildings actually do something. Only Barracks produce units; Farms don't.

## Current State Analysis

- **Building system** exists in `src/gameplay/building/` — places Barracks on grid (all buildings hardcoded to Barracks)
- **`GameSet::Production`** defined in `lib.rs:46` but unused
- **`Z_UNIT`** constant exists at `lib.rs:29` (currently `#[allow(dead_code)]`)
- **No `Mesh2d`/`ColorMaterial` usage** yet in the codebase — needed for circle rendering
- **No `gameplay/units/` module** yet — future domain plugins section in ARCHITECTURE.md references it

### Key Discoveries:
- `handle_building_placement` at `building/placement.rs:57-103` spawns buildings — needs modification to attach `ProductionTimer`
- `col_to_world_x` / `row_to_world_y` helpers at `battlefield/mod.rs:129-137` for grid→world conversion
- `CELL_SIZE = 64.0` at `battlefield/mod.rs:16`
- `BUILD_ZONE_START_COL = 2` at `battlefield/mod.rs:48`
- Test helpers in `testing.rs` — `create_base_test_app`, `transition_to_ingame`, `assert_entity_count`

## Desired End State

- Place a Barracks → it gets a repeating 3-second production timer
- Every 3 seconds, a green circle (unit) spawns one cell to the right of the building
- Units have `Team::Player`, `Unit`, `Health(100)`, `CombatStats`, `Movement` components
- Units idle at their spawn position (no movement yet — Ticket 4)
- `GameSet::Production` is active with production systems
- 90% test coverage maintained

### Verification:
- `make check` passes (clippy + type checking)
- `make test` passes (unit + integration tests)
- Manual: place Barracks, wait ~3s, see green circle appear; more circles accumulate over time

## What We're NOT Doing

- Unit movement (Ticket 4)
- Combat / health bars (Ticket 5)
- Economy / building costs (Ticket 6)
- Enemy units (Ticket 4 debug spawner)
- Building type selection UI (currently hardcoded to Barracks)

## Verified API Patterns (Bevy 0.18)

These were verified against the actual crate source:

- **`Circle::new(radius)`** — from `bevy::math::primitives`, in prelude
- **`Mesh2d(handle)`** — from `bevy::mesh`, in prelude. Has `#[require(Transform)]`
- **`MeshMaterial2d(handle)`** — from `bevy::sprite_render`, in prelude
- **`ColorMaterial`** — implements `From<Color>`, so `materials.add(Color::srgb(...))` works
- **`Timer::from_seconds(duration, TimerMode::Repeating)`** — from `bevy::time`, in prelude
- **`timer.tick(time.delta())`** — `Time::delta()` returns `Duration`
- **`timer.just_finished()`** — true only on the tick the timer completed

## Implementation Approach

Three phases: (1) define unit components, (2) add production system, (3) tests. The unit components module provides the data model; the production system in `building/` ticks timers and spawns units.

---

## Phase 1: Unit Components Module

### Overview
Create `src/gameplay/units/mod.rs` with all unit-related components, constants, and a `UnitAssets` resource for shared circle mesh/material handles.

### Changes Required:

#### 1. New file: `src/gameplay/units/mod.rs`

```rust
//! Unit components, constants, and shared rendering assets.

use bevy::prelude::*;

use crate::screens::GameState;

// === Constants ===

/// Prototype soldier stats.
pub const SOLDIER_HEALTH: f32 = 100.0;
pub const SOLDIER_DAMAGE: f32 = 10.0;
pub const SOLDIER_ATTACK_SPEED: f32 = 1.0;
pub const SOLDIER_MOVE_SPEED: f32 = 50.0;
pub const SOLDIER_ATTACK_RANGE: f32 = 30.0;

/// Visual radius of a unit circle.
pub const UNIT_RADIUS: f32 = 12.0;

/// Player unit color (green).
const PLAYER_UNIT_COLOR: Color = Color::srgb(0.2, 0.8, 0.2);

/// Barracks production interval in seconds.
pub const BARRACKS_PRODUCTION_INTERVAL: f32 = 3.0;

// === Components ===

/// Which side an entity belongs to. Standalone component used on units and fortresses.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub enum Team {
    Player,
    Enemy,
}

/// Marker for unit entities.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Unit;

/// Hit points for any damageable entity (units, fortresses).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    #[must_use]
    pub fn new(max: f32) -> Self {
        Self { current: max, max }
    }
}

/// Combat parameters.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct CombatStats {
    pub damage: f32,
    pub attack_speed: f32,
    pub range: f32,
}

/// Movement speed.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Movement {
    pub speed: f32,
}

// === Resources ===

/// Shared mesh and material handles for unit circle rendering.
#[derive(Resource, Debug)]
pub struct UnitAssets {
    pub player_mesh: Handle<Mesh>,
    pub player_material: Handle<ColorMaterial>,
}

// === Systems ===

fn setup_unit_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.insert_resource(UnitAssets {
        player_mesh: meshes.add(Circle::new(UNIT_RADIUS)),
        player_material: materials.add(PLAYER_UNIT_COLOR),
    });
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Team>()
        .register_type::<Unit>()
        .register_type::<Health>()
        .register_type::<CombatStats>()
        .register_type::<Movement>();

    app.add_systems(OnEnter(GameState::InGame), setup_unit_assets);
}
```

#### 2. Modified file: `src/gameplay/mod.rs`

Add units module and register plugin:

```rust
pub(crate) mod battlefield;
pub(crate) mod building;
pub(crate) mod units;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((battlefield::plugin, building::plugin, units::plugin));
}
```

#### 3. Modified file: `src/lib.rs`

Remove `#[allow(dead_code)]` from `Z_UNIT`:

```rust
/// Units (Ticket 3).
pub(crate) const Z_UNIT: f32 = 4.0;
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make build` passes (new module compiles)

---

## Phase 2: Production System

### Overview
Add `ProductionTimer` to Barracks buildings, and a system that ticks timers and spawns units when they fire.

### Changes Required:

#### 1. New file: `src/gameplay/building/production.rs`

```rust
//! Building production: timer ticking and unit spawning.

use bevy::prelude::*;

use super::ProductionTimer;
use crate::gameplay::battlefield::{BUILD_ZONE_START_COL, CELL_SIZE, col_to_world_x, row_to_world_y};
use crate::gameplay::units::{
    CombatStats, Health, Movement, SOLDIER_ATTACK_RANGE, SOLDIER_ATTACK_SPEED,
    SOLDIER_DAMAGE, SOLDIER_HEALTH, SOLDIER_MOVE_SPEED, Team, Unit, UnitAssets,
};
use crate::screens::GameState;
use crate::Z_UNIT;

/// Ticks production timers on all buildings and spawns units when timers fire.
pub(super) fn tick_production_and_spawn_units(
    time: Res<Time>,
    mut buildings: Query<(&super::Building, &mut ProductionTimer, &Transform)>,
    unit_assets: Res<UnitAssets>,
    mut commands: Commands,
) {
    for (building, mut timer, transform) in &mut buildings {
        timer.0.tick(time.delta());

        if timer.0.just_finished() {
            // Spawn unit one cell to the right of the building (toward combat zone)
            let spawn_x = transform.translation.x + CELL_SIZE;
            let spawn_y = transform.translation.y;

            commands.spawn((
                Unit,
                Team::Player,
                Health::new(SOLDIER_HEALTH),
                CombatStats {
                    damage: SOLDIER_DAMAGE,
                    attack_speed: SOLDIER_ATTACK_SPEED,
                    range: SOLDIER_ATTACK_RANGE,
                },
                Movement {
                    speed: SOLDIER_MOVE_SPEED,
                },
                Mesh2d(unit_assets.player_mesh.clone()),
                MeshMaterial2d(unit_assets.player_material.clone()),
                Transform::from_xyz(spawn_x, spawn_y, Z_UNIT),
                DespawnOnExit(GameState::InGame),
            ));
        }
    }
}
```

#### 2. Modified file: `src/gameplay/building/mod.rs`

Add `ProductionTimer` component, declare production module, register in plugin:

**New component** (add after `HoveredCell`):
```rust
/// Production timer for buildings that spawn units (e.g., Barracks).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct ProductionTimer(pub Timer);
```

**New module declaration** (add after `mod placement;`):
```rust
mod production;
```

**Plugin registration** — add type registration and production system:
```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<Building>()
        .register_type::<BuildingType>()
        .register_type::<Occupied>()
        .register_type::<GridCursor>()
        .register_type::<HoveredCell>()
        .register_type::<ProductionTimer>()
        .init_resource::<HoveredCell>();

    app.add_systems(
        OnEnter(GameState::InGame),
        placement::spawn_grid_cursor.after(BattlefieldSetup),
    )
    .add_systems(
        Update,
        (
            placement::update_grid_cursor,
            placement::handle_building_placement,
        )
            .chain_ignore_deferred()
            .in_set(crate::GameSet::Input)
            .run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
    )
    .add_systems(
        Update,
        production::tick_production_and_spawn_units
            .in_set(crate::GameSet::Production)
            .run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
    );
}
```

#### 3. Modified file: `src/gameplay/building/placement.rs`

Add `ProductionTimer` to Barracks when placed. Modify `handle_building_placement`:

**Add import:**
```rust
use super::ProductionTimer;
use crate::gameplay::units::BARRACKS_PRODUCTION_INTERVAL;
```

**Modify spawn in `handle_building_placement`** (replace lines 90-102):
```rust
    // Spawn the building entity
    let building_type = BuildingType::Barracks; // Hardcoded for now (Ticket 6 adds selector)
    let world_x = col_to_world_x(BUILD_ZONE_START_COL + col);
    let world_y = row_to_world_y(row);

    let mut entity_commands = commands.spawn((
        Building {
            building_type,
            grid_col: col,
            grid_row: row,
        },
        Sprite::from_color(
            building_color(building_type),
            Vec2::splat(BUILDING_SPRITE_SIZE),
        ),
        Transform::from_xyz(world_x, world_y, Z_BUILDING),
        DespawnOnExit(GameState::InGame),
    ));

    // Only Barracks get a production timer
    if building_type == BuildingType::Barracks {
        entity_commands.insert(ProductionTimer(
            Timer::from_seconds(BARRACKS_PRODUCTION_INTERVAL, TimerMode::Repeating),
        ));
    }
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes

#### Manual Verification:
- [ ] Place a Barracks building
- [ ] Wait ~3 seconds — green circle appears to the right of the building
- [ ] More green circles continue spawning at 3-second intervals
- [ ] Circles stay at their spawn position (no movement)
- [ ] Pausing (ESC) stops production; resuming continues it

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Phase 3: Tests

### Overview
Add unit tests for components/constants and integration tests for the production pipeline.

### Changes Required:

#### 1. Tests in `src/gameplay/units/mod.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn health_new_sets_current_to_max() {
        let health = Health::new(100.0);
        assert_eq!(health.current, 100.0);
        assert_eq!(health.max, 100.0);
    }

    #[test]
    fn team_variants_are_distinct() {
        assert_ne!(Team::Player, Team::Enemy);
    }

    #[test]
    fn soldier_stats_are_positive() {
        assert!(SOLDIER_HEALTH > 0.0);
        assert!(SOLDIER_DAMAGE > 0.0);
        assert!(SOLDIER_ATTACK_SPEED > 0.0);
        assert!(SOLDIER_MOVE_SPEED > 0.0);
        assert!(SOLDIER_ATTACK_RANGE > 0.0);
    }
}
```

#### 2. Integration tests in `src/gameplay/building/production.rs`

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::gameplay::battlefield::CELL_SIZE;
    use crate::gameplay::building::{Building, BuildingType, ProductionTimer};
    use crate::gameplay::units::{
        CombatStats, Health, Movement, Team, Unit, UnitAssets, UNIT_RADIUS,
    };
    use crate::menus::Menu;
    use crate::screens::GameState;
    use crate::testing::{assert_entity_count, transition_to_ingame};
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    /// Helper: app with battlefield + building + units plugins, no InputPlugin.
    fn create_production_test_app() -> App {
        let mut app = crate::testing::create_base_test_app_no_input();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>();

        // Configure GameSet ordering (normally done in lib.rs plugin)
        app.configure_sets(
            Update,
            (crate::GameSet::Input, crate::GameSet::Production).chain(),
        );

        app.add_plugins(crate::gameplay::battlefield::plugin);
        app.add_plugins(crate::gameplay::building::plugin);
        app.add_plugins(crate::gameplay::units::plugin);
        transition_to_ingame(&mut app);
        app
    }

    #[test]
    fn barracks_gets_production_timer() {
        let mut app = create_production_test_app();

        // Place a barracks via HoveredCell + mouse click
        app.world_mut()
            .resource_mut::<crate::gameplay::building::HoveredCell>()
            .0 = Some((2, 3));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        // Verify building has ProductionTimer
        assert_entity_count::<(With<Building>, With<ProductionTimer>)>(&mut app, 1);
    }

    #[test]
    fn production_timer_spawns_unit() {
        let mut app = create_production_test_app();

        // Manually spawn a barracks with a very short production timer
        let building_x = 320.0;
        let building_y = 160.0;
        let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
        timer.tick(Duration::from_secs_f32(0.002)); // Pre-tick past duration

        app.world_mut().spawn((
            Building {
                building_type: BuildingType::Barracks,
                grid_col: 2,
                grid_row: 3,
            },
            ProductionTimer(timer),
            Transform::from_xyz(building_x, building_y, crate::Z_BUILDING),
            DespawnOnExit(GameState::InGame),
        ));

        // Run one update — production system should fire
        app.update();

        // Verify unit was spawned
        assert_entity_count::<With<Unit>>(&mut app, 1);
    }

    #[test]
    fn spawned_unit_has_correct_components() {
        let mut app = create_production_test_app();

        let building_x = 320.0;
        let building_y = 160.0;
        let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
        timer.tick(Duration::from_secs_f32(0.002));

        app.world_mut().spawn((
            Building {
                building_type: BuildingType::Barracks,
                grid_col: 2,
                grid_row: 3,
            },
            ProductionTimer(timer),
            Transform::from_xyz(building_x, building_y, crate::Z_BUILDING),
            DespawnOnExit(GameState::InGame),
        ));
        app.update();

        // Check unit has all expected components
        assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<Health>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<CombatStats>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<Movement>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<DespawnOnExit<GameState>>)>(&mut app, 1);
    }

    #[test]
    fn spawned_unit_is_player_team() {
        let mut app = create_production_test_app();

        let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
        timer.tick(Duration::from_secs_f32(0.002));

        app.world_mut().spawn((
            Building {
                building_type: BuildingType::Barracks,
                grid_col: 0,
                grid_row: 0,
            },
            ProductionTimer(timer),
            Transform::from_xyz(200.0, 100.0, crate::Z_BUILDING),
            DespawnOnExit(GameState::InGame),
        ));
        app.update();

        let mut query = app.world_mut().query_filtered::<&Team, With<Unit>>();
        let team = query.single(app.world()).unwrap();
        assert_eq!(*team, Team::Player);
    }

    #[test]
    fn unit_spawns_to_right_of_building() {
        let mut app = create_production_test_app();

        let building_x = 320.0;
        let building_y = 160.0;
        let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
        timer.tick(Duration::from_secs_f32(0.002));

        app.world_mut().spawn((
            Building {
                building_type: BuildingType::Barracks,
                grid_col: 2,
                grid_row: 3,
            },
            ProductionTimer(timer),
            Transform::from_xyz(building_x, building_y, crate::Z_BUILDING),
            DespawnOnExit(GameState::InGame),
        ));
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<&Transform, With<Unit>>();
        let transform = query.single(app.world()).unwrap();
        assert_eq!(transform.translation.x, building_x + CELL_SIZE);
        assert_eq!(transform.translation.y, building_y);
    }

    #[test]
    fn no_units_without_buildings() {
        let mut app = create_production_test_app();
        app.update();
        assert_entity_count::<With<Unit>>(&mut app, 0);
    }
}
```

#### 3. Integration test in `src/gameplay/units/mod.rs`

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::testing::{assert_entity_count, transition_to_ingame};

    #[test]
    fn unit_assets_created_on_enter_ingame() {
        let mut app = crate::testing::create_base_test_app();
        app.add_plugins(plugin);
        transition_to_ingame(&mut app);

        assert!(app.world().get_resource::<UnitAssets>().is_some());
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes — all new tests green (59 total)
- [x] `make build` passes

#### Manual Verification:
- [ ] Same as Phase 2 manual checks — full end-to-end working

---

## Testing Strategy

### Unit Tests:
- `Health::new` constructor
- `Team` variants distinct
- Soldier stat constants are positive

### Integration Tests:
- `UnitAssets` resource created on `OnEnter(GameState::InGame)`
- Barracks gets `ProductionTimer` when placed
- Production timer spawns unit with correct components
- Spawned unit is `Team::Player`
- Spawned unit position is one cell right of building
- No units without buildings

### Test Approach for Timers:
Pre-tick a `Timer::from_seconds(0.001, TimerMode::Repeating)` past its duration before spawning the building entity. The production system's `tick(time.delta())` call with any positive delta will trigger `just_finished()` on such a short timer, spawning a unit.

## Files Changed Summary

| File | Action | Description |
|------|--------|-------------|
| `src/gameplay/units/mod.rs` | **New** | Unit components, constants, UnitAssets, plugin |
| `src/gameplay/building/production.rs` | **New** | Production tick + unit spawn system |
| `src/gameplay/building/mod.rs` | **Modified** | Add ProductionTimer, production module, register system |
| `src/gameplay/building/placement.rs` | **Modified** | Attach ProductionTimer to Barracks on placement |
| `src/gameplay/mod.rs` | **Modified** | Add units module + plugin |
| `src/lib.rs` | **Modified** | Remove dead_code allow from Z_UNIT |

## References

- Original ticket: `thoughts/shared/tickets/2026-02-08-0003-unit-spawning.md`
- Research doc: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Sections 2.2, 2.3)
- Dependent tickets: T4 (movement), T5 (combat), T8 (fortress health) — all import from `gameplay::units`

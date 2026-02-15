# Fix Building Health & Destruction — Implementation Plan

## Overview

Buildings are currently targetable but invulnerable — they have `Target` but no `Health` component, so projectile damage is silently skipped. This plan adds `Health` + `HealthBarConfig` to buildings at spawn time, and uses an observer to clean up the build slot when a building dies.

## Current State Analysis

- Buildings spawn in `gameplay/building/placement.rs:116-149` with `Building`, `Team::Player`, `Target`, a timer, and a sprite — but **no `Health`**
- In `combat/attack.rs:121`, `healths.get_mut(projectile.target)` returns `Err` for buildings → damage is skipped
- `check_death` in `combat/death.rs:14-19` already despawns any entity with `Health.current <= 0.0` — buildings will auto-die once they have `Health`
- Production/income timers live on the building entity, so despawning stops them automatically
- `Building` stores `grid_col`/`grid_row`; `GridIndex` maps `(col, row) → BuildSlot Entity`
- `Occupied` marker on `BuildSlot` prevents duplicate placement

### Key Discoveries:
- `spawn_health_bars` uses `Added<Health>` query filter with `.chain_ignore_deferred()` — canonical `On<Add, Health>` observer candidate
- Observer API: `On<Remove, Building>` fires before component removal, so `Building` data is still queryable
- All lifecycle types (`Add`, `Remove`, `On`) are in `bevy::prelude::*` — no explicit imports needed
- Entity archetype doc in `gameplay/mod.rs:8` already has a `(Health added in GAM-21)` placeholder

## Desired End State

- Buildings spawn with `Health` and `HealthBarConfig`
- Enemy projectiles reduce building HP
- Buildings despawn at 0 HP via existing `check_death`
- On building death, the corresponding `BuildSlot` loses its `Occupied` marker (grid cell reopens)
- Health bars appear above buildings showing current/max HP
- All behaviors covered by integration tests

### How to verify:
1. `make check` passes (clippy + build)
2. `make test` passes (all new + existing tests)
3. Manual: place a building, let enemies attack it, see health bar decrease, building despawns at 0, slot can be reused

## What We're NOT Doing

- Building sell/recycle mechanic (future feature)
- Building repair/heal mechanic
- Visual death effects (GAM-22)
- Data-driven building stats (GAM-16)
- Enemy buildings (only player buildings exist currently)

## Implementation Approach

Single phase — the changes are small and localized:
1. Add HP/health bar constants for each building type
2. Add `Health` + `HealthBarConfig` to the building spawn
3. Register a `On<Remove, Building>` observer for slot cleanup
4. Convert `spawn_health_bars` from `Added<Health>` system to `On<Add, Health>` observer
5. Update entity archetype doc
6. Write tests

## Phase 1: Building Health & Destruction

### Overview
Add Health to buildings, register observer for slot cleanup, write tests.

### Changes Required:

#### 1. Building Constants
**File**: `src/gameplay/building/mod.rs`
**Changes**: Add HP and health bar size constants per building type.

```rust
// === Building HP Constants ===

/// Barracks HP — tankier than farms, takes several hits to destroy.
pub const BARRACKS_HP: f32 = 300.0;

/// Farm HP — fragile, prioritize protecting them.
pub const FARM_HP: f32 = 150.0;

/// Building health bar width (wider than units since buildings are larger).
const BUILDING_HEALTH_BAR_WIDTH: f32 = 40.0;

/// Building health bar height.
const BUILDING_HEALTH_BAR_HEIGHT: f32 = 4.0;

/// Building health bar Y offset (above center of building sprite).
const BUILDING_HEALTH_BAR_Y_OFFSET: f32 = 36.0;
```

Add a helper to get HP by building type:

```rust
/// Get the max HP for a building type.
#[must_use]
pub const fn building_hp(building_type: BuildingType) -> f32 {
    match building_type {
        BuildingType::Barracks => BARRACKS_HP,
        BuildingType::Farm => FARM_HP,
    }
}
```

#### 2. Add Health + HealthBarConfig to Building Spawn
**File**: `src/gameplay/building/placement.rs`
**Changes**: Add `Health` and `HealthBarConfig` to the building spawn bundle.

In `handle_building_placement`, after the existing components in the spawn tuple, add:

```rust
use crate::gameplay::Health;
use crate::gameplay::combat::HealthBarConfig;
use super::{building_hp, BUILDING_HEALTH_BAR_WIDTH, BUILDING_HEALTH_BAR_HEIGHT, BUILDING_HEALTH_BAR_Y_OFFSET};

// Inside the spawn(...) call, add:
Health::new(building_hp(building_type)),
HealthBarConfig {
    width: BUILDING_HEALTH_BAR_WIDTH,
    height: BUILDING_HEALTH_BAR_HEIGHT,
    y_offset: BUILDING_HEALTH_BAR_Y_OFFSET,
},
```

#### 3. Slot Cleanup Observer
**File**: `src/gameplay/building/mod.rs`
**Changes**: Register an observer in the building plugin that removes `Occupied` from the corresponding `BuildSlot` when a building is removed.

```rust
// No explicit import needed — Remove and On are in bevy::prelude::*

// In the plugin function, add (between resources and systems):
app.add_observer(clear_build_slot_on_building_removed);
```

Define the observer handler:

```rust
/// When a building is removed (death, despawn), clear the `Occupied` marker
/// from the corresponding build slot so the grid cell can be reused.
fn clear_build_slot_on_building_removed(
    remove: On<Remove, Building>,
    buildings: Query<&Building>,
    grid_index: Res<crate::gameplay::battlefield::GridIndex>,
    mut commands: Commands,
) {
    // `Remove` fires before actual removal, so the component is still queryable.
    // On derefs to Remove, so remove.entity gives the target entity.
    let Ok(building) = buildings.get(remove.entity) else {
        return;
    };
    let Some(slot_entity) = grid_index.get(building.grid_col, building.grid_row) else {
        return;
    };
    commands.entity(slot_entity).remove::<Occupied>();
}
```

#### 4. Convert Health Bar Spawning to Observer
**File**: `src/gameplay/combat/health_bar.rs`
**Why**: `spawn_health_bars` currently uses `Added<Health>` query filter in an Update system, chained with `.chain_ignore_deferred()` to ensure bars exist before the update system runs. This is a textbook observer use case — a one-time reaction to component addition that's cross-cutting (units, buildings, and fortresses all get health bars).

**Current code** (lines 48-107):
```rust
fn spawn_health_bars(
    mut commands: Commands,
    new_entities: Query<(Entity, &HealthBarConfig), Added<Health>>,
) {
    for (entity, config) in &new_entities {
        commands.entity(entity).with_children(|parent| { /* spawn bars */ });
    }
}

// Plugin registration:
app.add_systems(
    Update,
    (spawn_health_bars, update_health_bars)
        .chain_ignore_deferred()
        .in_set(GameSet::Ui)
        .run_if(gameplay_running),
);
```

**Replace with**:
```rust
/// Spawns health bar child entities when `Health` is added to an entity with `HealthBarConfig`.
fn spawn_health_bars(
    add: On<Add, Health>,
    configs: Query<&HealthBarConfig>,
    mut commands: Commands,
) {
    let Ok(config) = configs.get(add.entity) else {
        return; // Entity has Health but no HealthBarConfig (shouldn't happen, but safe)
    };
    commands.entity(add.entity).with_children(|parent| {
        // Red background (full width, always visible)
        parent.spawn((
            Name::new("Health Bar BG"),
            Sprite::from_color(HEALTH_BAR_BG_COLOR, Vec2::new(config.width, config.height)),
            Transform::from_xyz(0.0, config.y_offset, 1.0),
            HealthBarBackground,
        ));
        // Green fill (scales with HP ratio, rendered in front of background)
        parent.spawn((
            Name::new("Health Bar Fill"),
            Sprite::from_color(HEALTH_BAR_FILL_COLOR, Vec2::new(config.width, config.height)),
            Transform::from_xyz(0.0, config.y_offset, 1.1),
            HealthBarFill,
        ));
    });
}
```

**Plugin registration changes**:
```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<HealthBarBackground>()
        .register_type::<HealthBarFill>()
        .register_type::<HealthBarConfig>();

    // Observer: spawn health bars immediately when Health is added
    app.add_observer(spawn_health_bars);

    // System: update health bar fill each frame (no longer needs chain)
    app.add_systems(
        Update,
        update_health_bars
            .in_set(GameSet::Ui)
            .run_if(gameplay_running),
    );
}
```

**Benefits**:
- Removes `.chain_ignore_deferred()` hack — observer fires immediately, bars exist before Update
- No per-frame `Added<Health>` polling — observer runs once per entity at insertion time
- Health bars appear same-frame as entity spawn (currently requires 2 updates)

**Test updates** (`health_bar.rs` integration_tests module):
- `create_health_bar_test_app()`: replace `add_systems(Update, (spawn_health_bars, update_health_bars).chain())` with `app.add_observer(spawn_health_bars)` + `app.add_systems(Update, update_health_bars)`
- Tests that previously needed 2 `app.update()` calls (spawn + apply deferred) may now need only 1 `app.update()` since the observer fires during entity spawn, but verify — `with_children` still queues deferred commands that need flushing
- `health_bar_spawned_on_entity_with_health`: likely still needs 2 updates (spawn entity → observer fires → deferred `with_children` applied on next update)
- `health_bar_fill_scales_with_damage`: unchanged (update system still runs in Update)

#### 5. Update Entity Archetype Doc
**File**: `src/gameplay/mod.rs`
**Changes**: Update line 8 to reflect the completed archetype:

```rust
/// **Buildings**: `Building`, `Team`, `Target`, `Health`, `HealthBarConfig`,
///           `ProductionTimer` or `IncomeTimer`
```

#### 5. Tests
**File**: `src/gameplay/building/placement.rs` (integration_tests module)

New tests to add:

```rust
#[test]
fn placed_building_has_health() {
    let mut app = create_placement_test_app();
    // Place a building (Barracks pre-selected)
    app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();

    let mut query = app.world_mut().query_filtered::<&Health, With<Building>>();
    let health = query.single(app.world()).unwrap();
    assert_eq!(health.current, BARRACKS_HP);
    assert_eq!(health.max, BARRACKS_HP);
}

#[test]
fn placed_building_has_health_bar_config() {
    let mut app = create_placement_test_app();
    app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();

    assert_entity_count::<(With<Building>, With<HealthBarConfig>)>(&mut app, 1);
}
```

**File**: `src/gameplay/building/mod.rs` (new test module for slot cleanup)

```rust
#[cfg(test)]
mod observer_tests {
    use super::*;
    use crate::gameplay::Health;
    use crate::gameplay::battlefield::{BuildSlot, GridIndex};
    use crate::testing::assert_entity_count;

    fn create_observer_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<GridIndex>();
        app.add_observer(clear_build_slot_on_building_removed);
        app
    }

    #[test]
    fn building_death_removes_occupied_from_slot() {
        let mut app = create_observer_test_app();

        // Spawn a build slot and register it in the grid index
        let slot = app.world_mut().spawn((BuildSlot { col: 2, row: 3 }, Occupied)).id();
        app.world_mut().resource_mut::<GridIndex>().insert(2, 3, slot);

        // Spawn a building at that grid position
        let building = app.world_mut().spawn((
            Building { building_type: BuildingType::Barracks, grid_col: 2, grid_row: 3 },
            Health::new(BARRACKS_HP),
        )).id();

        app.update();

        // Despawn the building (simulates check_death)
        app.world_mut().despawn(building);
        app.update(); // Process deferred commands from observer

        // Slot should no longer be occupied
        assert_entity_count::<(With<BuildSlot>, With<Occupied>)>(&mut app, 0);
        // Slot entity itself should still exist
        assert_entity_count::<With<BuildSlot>>(&mut app, 1);
    }

    #[test]
    fn building_death_slot_remains_when_not_in_grid_index() {
        let mut app = create_observer_test_app();

        // Spawn a building without a matching grid index entry
        let building = app.world_mut().spawn((
            Building { building_type: BuildingType::Farm, grid_col: 0, grid_row: 0 },
            Health::new(FARM_HP),
        )).id();

        app.update();
        app.world_mut().despawn(building);
        app.update();
        // Should not panic — gracefully handles missing slot
    }
}
```

**File**: `src/gameplay/building/mod.rs` (unit tests for new helpers)

```rust
// In existing tests module:

#[test]
fn barracks_hp_constant() {
    assert_eq!(building_hp(BuildingType::Barracks), BARRACKS_HP);
}

#[test]
fn farm_hp_constant() {
    assert_eq!(building_hp(BuildingType::Farm), FARM_HP);
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (clippy + build)
- [ ] `make test` passes (all existing + new tests)
- [ ] New tests: `placed_building_has_health`, `placed_building_has_health_bar_config`, `building_death_removes_occupied_from_slot`, `building_death_slot_remains_when_not_in_grid_index`, `barracks_hp_constant`, `farm_hp_constant`
- [ ] Existing health bar tests pass with observer-based spawning (no `.chain_ignore_deferred()`)

#### Manual Verification:
- [ ] Place a barracks — health bar appears above it
- [ ] Let enemies attack the barracks — health bar decreases
- [ ] Barracks despawns when HP reaches 0
- [ ] After barracks despawns, the grid cell shows as empty (no occupied highlight)
- [ ] Can place a new building on the same cell after the old one died
- [ ] Farms also take damage and die correctly
- [ ] Barracks death stops unit production (no phantom spawns)
- [ ] Farm death stops income generation
- [ ] Health bars still appear on enemy units (observer works for all entity types)

## Verified API Patterns (Bevy 0.18)

These were verified against actual crate source AND foxtrot usage:

- `On<Remove, T>` — observer event for component removal. `Remove` and `On` are in `bevy::prelude::*` — no explicit import needed
- `On` derefs to the event struct — use `remove.entity` (not `.observer()`) to get the target entity
- `Remove` fires **before** the component is removed — query still returns data
- Lifecycle event ordering during despawn: `Replace` → `Remove` → `Despawn`
- `app.add_observer(system)` registers a global observer in plugin setup (between resources and systems)
- Observer handlers support full system params: `Commands`, `Query`, `Res`, `ResMut`, `Single`
- Parameter naming convention: `add: On<Add, T>`, `remove: On<Remove, T>` (match the action)
- Handler naming convention: verb phrases describing the action (`clear_build_slot_on_building_removed`)
- `On<Add, T>` — observer event for component addition. `On` derefs to `Add` which has `.entity`
- `Added<T>` query filter — still valid for systems but observers are preferred for one-time setup reactions
- Observer `with_children` still uses deferred commands — tests may still need 2 `app.update()` calls

## Testing Strategy

### Unit Tests:
- `building_hp()` returns correct values per type
- Constants are positive and valid

### Integration Tests:
- Building spawns with correct `Health` and `HealthBarConfig`
- Building death removes `Occupied` from slot (`On<Remove, Building>` observer fires)
- Observer gracefully handles missing grid index entries
- Health bar observer (`On<Add, Health>`) spawns bars for new entities
- Existing health bar tests pass after observer migration (5 tests in `health_bar.rs`)
- Existing tests still pass (placement, gold deduction, occupied detection)

### Manual Testing Steps:
1. Start a game, place buildings in the build zone
2. Wait for enemy wave — enemies should target and attack buildings
3. Watch health bars decrease on hit
4. Building despawns at 0 HP
5. Verify grid cell is reusable after building death

## References

- Linear ticket: [GAM-21](https://linear.app/tayhu-games/issue/GAM-21/fix-building-health-and-destruction)
- Depends on: GAM-11 (shared Health/Target in gameplay/mod.rs) — already done
- Blocked by this: GAM-16 (data-driven building stats will include HP)

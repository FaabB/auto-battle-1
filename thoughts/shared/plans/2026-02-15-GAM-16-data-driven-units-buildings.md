# GAM-16: Data-Driven Units & Buildings Implementation Plan

## Overview

Extract hardcoded unit and building stats into data-driven lookup tables, and create a shared unit spawn factory to eliminate the duplicated 13-component spawn bundle. This makes adding new unit/building types a one-line change (enum variant + stats entry) instead of touching 5+ files.

## Current State Analysis

### Unit Spawning (DRY violation)
The same 13-component spawn bundle is duplicated in two places:
- `gameplay/units/spawn.rs:103-131` (enemy spawner)
- `gameplay/building/production.rs:34-62` (barracks production)

Only `Team`, material handle, and position differ.

### Unit Stats
One unit type (Soldier) with 5 hardcoded constants in `gameplay/units/mod.rs:15-19`:
```rust
pub const SOLDIER_HEALTH: f32 = 100.0;
pub const SOLDIER_DAMAGE: f32 = 10.0;
pub const SOLDIER_ATTACK_SPEED: f32 = 1.0;
pub const SOLDIER_MOVE_SPEED: f32 = 50.0;
pub const SOLDIER_ATTACK_RANGE: f32 = 30.0;
```

### Building Stats
Two building types with properties scattered across 4 files:
- **HP**: `building/mod.rs:31-34` (`BARRACKS_HP`, `FARM_HP`) + `building_hp()` match
- **Color**: `building/mod.rs:21-23` (`BARRACKS_COLOR`, `FARM_COLOR`) + `building_color()` match
- **Cost**: `economy/mod.rs:19-22` (`BARRACKS_COST`, `FARM_COST`) + `building_cost()` match
- **Timers**: `building/placement.rs:142-157` — per-type match arms for `ProductionTimer` vs `IncomeTimer`
- **Production interval**: `building/mod.rs:15` (`BARRACKS_PRODUCTION_INTERVAL`)
- **Income interval**: `economy/mod.rs:31` (`FARM_INCOME_INTERVAL`)

### Shop
Hardcoded `BUILDING_POOL: [BuildingType; 2]` in `economy/shop.rs:20`. Card text uses per-type match in `shop_ui.rs:219-224`.

## Desired End State

- **`UnitType` enum** on all unit entities, with `UnitStats` lookup
- **`BuildingStats` struct** consolidating all building properties in one place
- **Single `spawn_unit()` factory** — both production and enemy spawner call it
- **`BuildingType::ALL`** auto-populates the shop card pool
- Adding a new unit type = add enum variant + `unit_stats()` match arm
- Adding a new building type = add enum variant + `building_stats()` match arm

### Verification:
```
cargo clippy -- -D warnings  # no new warnings
cargo test                    # all existing + new tests pass
```
Manual: run the game, place buildings, verify units spawn correctly with same behavior.

## What We're NOT Doing

- **Not promoting `CombatStats` to shared** — no tower buildings in this ticket
- **Not adding new unit/building types** — that's GAM-17 (Archer) and GAM-18 (Knight)
- **Not extracting to external config files** — that's GAM-19 (BalanceConfig from RON/TOML)
- **Not changing any balance values** — all stats stay identical

## Implementation Approach

Use `const fn` match-based lookups for both `unit_stats()` and `building_stats()`. The type sets are small and compile-time known, so HashMap resources would be over-engineering. GAM-19 can later wrap these in a config resource loaded from files.

---

## Phase 1: UnitType + UnitStats + Spawn Factory

### Overview
Add a `UnitType` enum, consolidate soldier stats into `UnitStats`, and extract the shared spawn factory. This eliminates the main DRY violation.

### Changes Required:

#### 1. Add UnitType, UnitStats, and spawn factory
**File**: `src/gameplay/units/mod.rs`

Add the `UnitType` enum, `UnitStats` struct, `unit_stats()` lookup, and `spawn_unit()` factory. Remove individual `SOLDIER_*` constants.

```rust
// === Unit Type System ===

/// Types of units in the game.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub enum UnitType {
    Soldier,
}

impl UnitType {
    /// All unit types, for iteration.
    pub const ALL: &[Self] = &[Self::Soldier];

    /// Human-readable display name.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Soldier => "Soldier",
        }
    }
}

/// Stats for a unit type. All values are compile-time constants.
#[derive(Debug, Clone, Copy)]
pub struct UnitStats {
    pub hp: f32,
    pub damage: f32,
    pub attack_speed: f32,
    pub move_speed: f32,
    pub attack_range: f32,
}

/// Look up stats for a unit type.
#[must_use]
pub const fn unit_stats(unit_type: UnitType) -> UnitStats {
    match unit_type {
        UnitType::Soldier => UnitStats {
            hp: 100.0,
            damage: 10.0,
            attack_speed: 1.0,
            move_speed: 50.0,
            attack_range: 30.0,
        },
    }
}
```

The spawn factory function (non-const, uses Commands):
```rust
use crate::gameplay::combat::{
    AttackTimer, HealthBarConfig, UNIT_HEALTH_BAR_HEIGHT, UNIT_HEALTH_BAR_WIDTH,
    UNIT_HEALTH_BAR_Y_OFFSET,
};
use crate::gameplay::{Health, Target, Team};
use crate::screens::GameState;
use crate::Z_UNIT;

/// Spawn a unit entity with all required components.
/// Single source of truth for the unit archetype.
pub fn spawn_unit(
    commands: &mut Commands,
    unit_type: UnitType,
    team: Team,
    position: Vec3,
    assets: &UnitAssets,
) -> Entity {
    let stats = unit_stats(unit_type);
    let material = match team {
        Team::Player => assets.player_material.clone(),
        Team::Enemy => assets.enemy_material.clone(),
    };

    commands
        .spawn((
            Name::new(format!("{team:?} {}", unit_type.display_name())),
            Unit,
            unit_type,
            team,
            Target,
            CurrentTarget(None),
            Health::new(stats.hp),
            HealthBarConfig {
                width: UNIT_HEALTH_BAR_WIDTH,
                height: UNIT_HEALTH_BAR_HEIGHT,
                y_offset: UNIT_HEALTH_BAR_Y_OFFSET,
            },
            CombatStats {
                damage: stats.damage,
                attack_speed: stats.attack_speed,
                range: stats.attack_range,
            },
            Movement {
                speed: stats.move_speed,
            },
            AttackTimer(Timer::from_seconds(
                1.0 / stats.attack_speed,
                TimerMode::Repeating,
            )),
            Mesh2d(assets.mesh.clone()),
            MeshMaterial2d(material),
            Transform::from_xyz(position.x, position.y, Z_UNIT),
            DespawnOnExit(GameState::InGame),
        ))
        .id()
}
```

Also register `UnitType` in the plugin:
```rust
// In plugin():
app.register_type::<UnitType>();
```

**Remove** these individual constants from `units/mod.rs`:
- `SOLDIER_HEALTH`, `SOLDIER_DAMAGE`, `SOLDIER_ATTACK_SPEED`, `SOLDIER_MOVE_SPEED`, `SOLDIER_ATTACK_RANGE`

**Keep**: `UNIT_RADIUS`, `BACKTRACK_DISTANCE`, player/enemy color constants, `UnitAssets` — these are visual/AI concerns, not per-type stats.

#### 2. Update enemy spawner to use factory
**File**: `src/gameplay/units/spawn.rs`

Replace the inline spawn bundle (lines 103-131) with a call to `spawn_unit()`:

```rust
// In tick_enemy_spawner, replace the commands.spawn((...)) block with:
let spawn_x = col_to_world_x(ENEMY_SPAWN_COL);
let spawn_y = row_to_world_y(row);

super::spawn_unit(
    &mut commands,
    super::UnitType::Soldier,
    Team::Enemy,
    Vec3::new(spawn_x, spawn_y, Z_UNIT),
    &unit_assets,
);
```

Remove unused imports that were only needed for the inline bundle:
- `HealthBarConfig`, `UNIT_HEALTH_BAR_*` constants
- `CombatStats`, `CurrentTarget`, `Movement`, `SOLDIER_*` constants

Keep: `Team`, `Z_UNIT` (used in position calculation), battlefield imports.

#### 3. Update barracks production to use factory
**File**: `src/gameplay/building/production.rs`

Replace the inline spawn bundle (lines 34-62) with a call to `spawn_unit()`:

```rust
use crate::gameplay::units::{UnitType, spawn_unit};

// In tick_production_and_spawn_units, replace the commands.spawn((...)) block:
if timer.0.just_finished() {
    let spawn_x = transform.translation.x + CELL_SIZE;
    let spawn_y = transform.translation.y;

    spawn_unit(
        &mut commands,
        UnitType::Soldier,  // TODO: use building's produced_unit (Phase 2)
        crate::gameplay::Team::Player,
        Vec3::new(spawn_x, spawn_y, crate::Z_UNIT),
        &unit_assets,
    );
}
```

Remove unused imports: `AttackTimer`, `HealthBarConfig`, `UNIT_HEALTH_BAR_*`, `CombatStats`, `CurrentTarget`, `Movement`, `SOLDIER_*` constants, `Health`, `Target`, `Team`, `GameState`, `Z_UNIT`.

Keep: `CELL_SIZE` (for spawn offset calculation), `UnitAssets`.

#### 4. Update tests
**File**: `src/gameplay/units/mod.rs` (tests section)

Replace `soldier_stats_are_positive` test:
```rust
#[test]
fn soldier_stats_are_positive() {
    let stats = unit_stats(UnitType::Soldier);
    assert!(stats.hp > 0.0);
    assert!(stats.damage > 0.0);
    assert!(stats.attack_speed > 0.0);
    assert!(stats.move_speed > 0.0);
    assert!(stats.attack_range > 0.0);
}
```

Add new tests:
```rust
#[test]
fn unit_type_display_name() {
    assert_eq!(UnitType::Soldier.display_name(), "Soldier");
}

#[test]
fn unit_type_all_contains_soldier() {
    assert!(UnitType::ALL.contains(&UnitType::Soldier));
}
```

**File**: `src/gameplay/units/spawn.rs` (integration tests)

Update `spawned_enemy_has_all_components` to also check for `UnitType`:
```rust
assert_entity_count::<(With<Unit>, With<super::super::UnitType>)>(&mut app, 1);
```

**File**: `src/gameplay/combat/attack.rs` (integration tests)

The `spawn_attacker` helper (line 204) uses `SOLDIER_*` constants. Update to use `unit_stats()`:
```rust
use crate::gameplay::units::{UnitType, unit_stats};

fn spawn_attacker(world: &mut World, x: f32, target: Option<Entity>) -> Entity {
    let stats = unit_stats(UnitType::Soldier);
    let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
    timer.set_elapsed(Duration::from_nanos(999_000));
    world
        .spawn((
            Unit,
            CurrentTarget(target),
            CombatStats {
                damage: stats.damage,
                attack_speed: stats.attack_speed,
                range: stats.attack_range,
            },
            AttackTimer(timer),
            Movement { speed: stats.move_speed },
            Transform::from_xyz(x, 100.0, 0.0),
            GlobalTransform::from(Transform::from_xyz(x, 100.0, 0.0)),
        ))
        .id()
}
```

Also update `attack_respects_cooldown` test (line 342) which also uses `SOLDIER_*` constants inline.

### Success Criteria:

#### Automated Verification:
- [ ] `cargo clippy -- -D warnings` passes with no new warnings
- [ ] `cargo test` — all existing tests pass (updated to use new API)
- [ ] New unit tests for `UnitType`, `UnitStats`, `unit_stats()` pass
- [ ] No `SOLDIER_*` constants remain as top-level `pub const` in `units/mod.rs`

#### Manual Verification:
- [ ] Run the game — units spawn from barracks and enemy fortress with identical behavior
- [ ] Units have correct health, damage, speed, range (unchanged values)

**Implementation Note**: After Phase 1 automated verification passes, proceed to Phase 2. No manual pause needed — behavior is unchanged.

---

## Phase 2: BuildingStats + Data-Driven Placement + Shop Integration

### Overview
Consolidate all building properties into `BuildingStats`, eliminate per-type match arms in placement, and auto-populate the shop card pool.

### Changes Required:

#### 1. Add BuildingStats and consolidate building properties
**File**: `src/gameplay/building/mod.rs`

Add `BuildingStats` struct and `building_stats()` lookup. Add `BuildingType::ALL` and `display_name()`. Remove individual per-type constants.

```rust
use crate::gameplay::units::UnitType;

// === Building Type Extensions ===

impl BuildingType {
    /// All building types, used by shop card pool.
    pub const ALL: &[Self] = &[Self::Barracks, Self::Farm];

    /// Human-readable display name.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Barracks => "Barracks",
            Self::Farm => "Farm",
        }
    }
}

/// Stats for a building type. All values are compile-time constants.
#[derive(Debug, Clone, Copy)]
pub struct BuildingStats {
    /// Maximum hit points.
    pub hp: f32,
    /// Gold cost to place.
    pub cost: u32,
    /// Sprite color.
    pub color: Color,
    /// Unit type this building produces, if any.
    pub produced_unit: Option<UnitType>,
    /// Production timer interval (seconds), if this building produces units.
    pub production_interval: Option<f32>,
    /// Income timer interval (seconds), if this building generates income.
    pub income_interval: Option<f32>,
}

/// Look up stats for a building type.
#[must_use]
pub const fn building_stats(building_type: BuildingType) -> BuildingStats {
    match building_type {
        BuildingType::Barracks => BuildingStats {
            hp: 300.0,
            cost: 100,
            color: Color::srgb(0.15, 0.2, 0.6),
            produced_unit: Some(UnitType::Soldier),
            production_interval: Some(3.0),
            income_interval: None,
        },
        BuildingType::Farm => BuildingStats {
            hp: 150.0,
            cost: 50,
            color: Color::srgb(0.2, 0.6, 0.1),
            produced_unit: None,
            production_interval: None,
            income_interval: Some(1.0),
        },
    }
}
```

**Update** `building_color()` and `building_hp()` to delegate:
```rust
#[must_use]
pub const fn building_color(building_type: BuildingType) -> Color {
    building_stats(building_type).color
}

#[must_use]
pub const fn building_hp(building_type: BuildingType) -> f32 {
    building_stats(building_type).hp
}
```

**Remove** these constants from `building/mod.rs`:
- `BARRACKS_PRODUCTION_INTERVAL` (now `building_stats(Barracks).production_interval.unwrap()`)
- `BARRACKS_HP`, `FARM_HP` (now in `BuildingStats`)
- `BARRACKS_COLOR`, `FARM_COLOR` (now in `BuildingStats`)

**Keep**: `GRID_CURSOR_COLOR`, `BUILDING_SPRITE_SIZE`, `BUILDING_HEALTH_BAR_*` — these are shared visual constants, not per-type.

#### 2. Refactor building placement to use data-driven dispatch
**File**: `src/gameplay/building/placement.rs`

Replace the per-type match arms (lines 142-157) with data-driven conditional inserts:

```rust
use super::{building_stats, ProductionTimer};
use crate::gameplay::economy::income::IncomeTimer;

// In handle_building_placement, replace the match block:
let stats = building_stats(building_type);

// Check gold using stats
let cost = stats.cost;
if gold.0 < cost {
    return;
}
gold.0 -= cost;

// ... spawn building entity using stats.color, stats.hp ...

// Data-driven timer insertion — no per-type match needed
if let Some(interval) = stats.production_interval {
    entity_commands.insert(ProductionTimer(Timer::from_seconds(
        interval,
        TimerMode::Repeating,
    )));
}
if let Some(interval) = stats.income_interval {
    entity_commands.insert(crate::gameplay::economy::income::IncomeTimer(
        Timer::from_seconds(interval, TimerMode::Repeating),
    ));
}
```

Remove the import of `BARRACKS_PRODUCTION_INTERVAL`.
Remove the `building_cost()` call — use `building_stats().cost` directly.

#### 3. Update economy module
**File**: `src/gameplay/economy/mod.rs`

**Remove** these constants (moved to `BuildingStats`):
- `BARRACKS_COST`
- `FARM_COST`
- `FARM_INCOME_INTERVAL`

**Replace** `building_cost()` to delegate to `building_stats()`:
```rust
/// Get the gold cost for a building type.
#[must_use]
pub const fn building_cost(building_type: BuildingType) -> u32 {
    crate::gameplay::building::building_stats(building_type).cost
}
```

This preserves the existing API for callers that import from economy (like `shop_ui.rs:230`).

#### 4. Update shop card pool
**File**: `src/gameplay/economy/shop.rs`

Replace the hardcoded `BUILDING_POOL`:
```rust
// Remove:
// const BUILDING_POOL: [BuildingType; 2] = [BuildingType::Barracks, BuildingType::Farm];

// In generate_cards(), replace BUILDING_POOL with BuildingType::ALL:
pub fn generate_cards(&mut self) {
    use rand::Rng;
    let mut rng = rand::rng();
    let pool = BuildingType::ALL;
    for card in &mut self.cards {
        let idx = rng.random_range(0..pool.len());
        *card = Some(pool[idx]);
    }
    self.selected = None;
}
```

#### 5. Update shop UI card text
**File**: `src/gameplay/economy/shop_ui.rs`

Replace the per-type match in `update_card_text` (lines 219-224):
```rust
// Replace the name text match:
*text = Text::new(match shop.cards[slot] {
    Some(bt) => bt.display_name(),
    None => "—",
});
```

The cost text already uses `super::building_cost(bt)` which will now delegate to `building_stats()` — no change needed there.

#### 6. Wire production to building's produced_unit
**File**: `src/gameplay/building/production.rs`

Update `tick_production_and_spawn_units` to use the building's `produced_unit` from stats:

```rust
use crate::gameplay::building::building_stats;
use crate::gameplay::units::spawn_unit;

fn tick_production_and_spawn_units(
    time: Res<Time>,
    mut buildings: Query<(&super::Building, &mut ProductionTimer, &Transform)>,
    unit_assets: Res<UnitAssets>,
    mut commands: Commands,
) {
    for (building, mut timer, transform) in &mut buildings {
        timer.0.tick(time.delta());

        if timer.0.just_finished() {
            let stats = building_stats(building.building_type);
            if let Some(unit_type) = stats.produced_unit {
                let spawn_x = transform.translation.x + CELL_SIZE;
                let spawn_y = transform.translation.y;

                spawn_unit(
                    &mut commands,
                    unit_type,
                    crate::gameplay::Team::Player,
                    Vec3::new(spawn_x, spawn_y, crate::Z_UNIT),
                    &unit_assets,
                );
            }
        }
    }
}
```

#### 7. Update tests

**File**: `src/gameplay/building/mod.rs` (tests)

Replace constant-based tests with stats-based:
```rust
#[test]
fn barracks_stats() {
    let stats = building_stats(BuildingType::Barracks);
    assert!(stats.hp > 0.0);
    assert!(stats.cost > 0);
    assert!(stats.produced_unit.is_some());
    assert!(stats.production_interval.is_some());
    assert!(stats.income_interval.is_none());
}

#[test]
fn farm_stats() {
    let stats = building_stats(BuildingType::Farm);
    assert!(stats.hp > 0.0);
    assert!(stats.cost > 0);
    assert!(stats.produced_unit.is_none());
    assert!(stats.production_interval.is_none());
    assert!(stats.income_interval.is_some());
}

#[test]
fn building_type_display_name() {
    assert_eq!(BuildingType::Barracks.display_name(), "Barracks");
    assert_eq!(BuildingType::Farm.display_name(), "Farm");
}

#[test]
fn building_type_all_contains_all_variants() {
    assert!(BuildingType::ALL.contains(&BuildingType::Barracks));
    assert!(BuildingType::ALL.contains(&BuildingType::Farm));
}
```

Update observer tests that reference `BARRACKS_HP`/`FARM_HP`:
```rust
// Replace: Health::new(BARRACKS_HP)
// With:    Health::new(building_stats(BuildingType::Barracks).hp)
```

**File**: `src/gameplay/building/placement.rs` (tests)

Update `placement_deducts_gold` test:
```rust
// Replace: crate::gameplay::economy::STARTING_GOLD - crate::gameplay::economy::BARRACKS_COST
// With:    crate::gameplay::economy::STARTING_GOLD - building_stats(BuildingType::Barracks).cost
```

Update `placement_blocked_when_gold_below_cost`:
```rust
// Replace: crate::gameplay::economy::BARRACKS_COST - 1
// With:    building_stats(BuildingType::Barracks).cost - 1
```

Update `placed_building_has_health`:
```rust
// Replace: BARRACKS_HP
// With:    building_stats(BuildingType::Barracks).hp
```

**File**: `src/gameplay/economy/mod.rs` (tests)

Update `building_cost_*` tests to verify delegation:
```rust
#[test]
fn building_cost_matches_stats() {
    assert_eq!(
        building_cost(BuildingType::Barracks),
        crate::gameplay::building::building_stats(BuildingType::Barracks).cost
    );
    assert_eq!(
        building_cost(BuildingType::Farm),
        crate::gameplay::building::building_stats(BuildingType::Farm).cost
    );
}
```

**File**: `src/gameplay/economy/shop.rs` (tests)

Update `generate_cards_only_uses_pool` to check against `BuildingType::ALL`:
```rust
#[test]
fn generate_cards_only_uses_pool() {
    let mut shop = Shop::default();
    shop.generate_cards();

    for card in &shop.cards {
        let bt = card.unwrap();
        assert!(
            BuildingType::ALL.contains(&bt),
            "Card should be in BuildingType::ALL, got {bt:?}"
        );
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` — all tests pass
- [ ] No per-type constants remain (`BARRACKS_HP`, `FARM_HP`, `BARRACKS_COST`, `FARM_COST`, `BARRACKS_PRODUCTION_INTERVAL`, `FARM_INCOME_INTERVAL`, `BARRACKS_COLOR`, `FARM_COLOR`)
- [ ] No `BUILDING_POOL` constant in `shop.rs`
- [ ] No per-type match arms in `handle_building_placement` for timer insertion
- [ ] `building_stats()` is the single source of truth for building properties

#### Manual Verification:
- [ ] Run the game — buildings place correctly with correct colors
- [ ] Barracks produces units at the same rate
- [ ] Farms generate income at the same rate
- [ ] Shop cards display correct names and costs
- [ ] Gold deduction works correctly for both building types
- [ ] Buildings die at correct HP thresholds

**Implementation Note**: After completing Phase 2 and all automated verification passes, pause for manual confirmation.

---

## Testing Strategy

### Unit Tests:
- `UnitType::display_name()` returns correct strings
- `UnitType::ALL` contains all variants
- `unit_stats()` returns positive values for all fields
- `BuildingType::display_name()` returns correct strings
- `BuildingType::ALL` contains all variants
- `building_stats()` returns correct values for each type
- `building_cost()` delegates correctly to `building_stats().cost`

### Integration Tests:
- Spawned units have `UnitType` component
- Enemy spawner uses factory (same components as before)
- Barracks production uses factory (same components as before)
- Building placement uses stats-based cost check
- Building placement inserts correct timer type based on stats
- Shop card generation uses `BuildingType::ALL`

### Existing Test Updates:
- Replace `SOLDIER_*` constant references with `unit_stats()` calls
- Replace `BARRACKS_HP`/`FARM_HP` with `building_stats().hp`
- Replace `BARRACKS_COST`/`FARM_COST` with `building_stats().cost`
- All ~50 existing tests must continue passing

## Performance Considerations

No performance impact. `const fn` lookups compile to the same code as the current inline constants. The `spawn_unit()` function adds no overhead — it just moves the same code behind a function call.

## References

- Linear ticket: [GAM-16](https://linear.app/tayhu-games/issue/GAM-16/prepare-game-for-a-big-selection-of-units-and-building)
- Blocked by: GAM-11 (DONE), GAM-21 (DONE)
- Blocks: GAM-19 (balance config extraction)
- Future: GAM-17 (Archer), GAM-18 (Knight) — will add new enum variants + stats entries

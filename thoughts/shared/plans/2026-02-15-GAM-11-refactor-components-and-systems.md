# GAM-11: Refactor Components & Systems — Implementation Plan

## Overview

Clean up cross-cutting component placement, split the oversized combat module, add architectural helpers (`gameplay_running`, `DeathCheck` set), fix convention gaps, and add entity names for inspector support. This is the "clean the foundations" ticket before deeper features land.

## Current State Analysis

- `Team`, `Health`, `Target` live in `units/mod.rs` but are used by 6 other modules (combat, battlefield, building, economy, endgame)
- `combat/mod.rs` is 657 lines mixing attack mechanics, death detection, and health bars
- `endgame.rs` and `income.rs` order against `pub fn check_death` directly (coupling to a foreign function)
- 15+ systems repeat `in_state(GameState::InGame).and(in_state(Menu::None))`
- `lib.rs` uses `pub mod` instead of `pub(crate) mod` for domain modules
- No entity names — Bevy inspector shows generic Entity IDs
- Global `Local<u32>` counter in `unit_find_target` causes all units to retarget on the same frame
- `handle_building_placement` doesn't check for UI interaction — clicks pass through shop panel

### Key Discoveries:
- `BARRACKS_PRODUCTION_INTERVAL` in `units/mod.rs:34` describes building production, not unit stats
- `Z_HEALTH_BAR` in `lib.rs:32` has `#[allow(dead_code)]` — health bars use relative child offsets, so the absolute constant is unnecessary
- `BuildSlot` in `battlefield/mod.rs:109` has all-Copy fields (`u16, u16`) but doesn't derive `Copy`
- `open_pause_menu` in `screens/in_game.rs:14` has no `GameSet::Input` annotation
- `combat/mod.rs` tests are well-structured and can be split cleanly with their systems

## Desired End State

After implementation:
- Cross-cutting components (`Team`, `Health`, `Target`) live in `gameplay/mod.rs`
- `combat/` has 3 submodules: `attack.rs`, `death.rs`, `health_bar.rs`
- All gameplay systems use `gameplay_running` helper for run conditions
- `DeathCheck` SystemSet replaces direct function ordering
- All spawned entities have `Name` components
- Unit retargeting is staggered across frames
- UI clicks don't pass through to world placement
- Module visibility follows `pub(crate)` convention

## What We're NOT Doing

- Adding `Health` to buildings (that's GAM-21)
- Restructuring `units/spawn.rs` into a separate `waves/` module
- Adding observer-based entity setup (no current need)
- Changing any game behavior — this is purely structural

## Implementation Approach

Six phases, each independently compilable and testable. Phases ordered by dependency: component relocation first (changes imports everywhere), then structural splits, then convention fixes.

---

## Phase 1: Component Relocation

### Overview
Move `Team`, `Health`, `Target` from `units/mod.rs` to `gameplay/mod.rs`. Move `BARRACKS_PRODUCTION_INTERVAL` to `building/mod.rs`. Add entity archetype documentation.

### Changes Required:

#### 1. Add components to `gameplay/mod.rs`
**File**: `src/gameplay/mod.rs`

Move component definitions (`Team`, `Health` with impl, `Target`) from `units/mod.rs` to here. Add type registrations in the plugin function. Add archetype documentation.

```rust
//! Gameplay domain plugins and cross-cutting components.
//!
//! # Entity Archetypes
//!
//! **Units**: Unit, Team, Target, CurrentTarget, Health, CombatStats, Movement,
//!           AttackTimer, HealthBarConfig, Mesh2d, MeshMaterial2d
//!
//! **Buildings**: Building, Team, Target, ProductionTimer or IncomeTimer
//!           (Health added in GAM-21)
//!
//! **Fortresses**: PlayerFortress/EnemyFortress, Team, Target, Health, HealthBarConfig

pub(crate) mod battlefield;
pub(crate) mod building;
pub(crate) mod combat;
pub(crate) mod economy;
pub(crate) mod endgame;
pub(crate) mod units;

use bevy::prelude::*;

// === Cross-Cutting Components ===

/// Which side an entity belongs to. Used on units, buildings, and fortresses.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub enum Team {
    Player,
    Enemy,
}

/// Hit points for any damageable entity (units, fortresses, future: buildings).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    #[must_use]
    pub const fn new(max: f32) -> Self {
        Self { current: max, max }
    }
}

/// Marker: this entity can be targeted by units.
/// Placed on units, buildings, and fortresses.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Target;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Team>()
        .register_type::<Health>()
        .register_type::<Target>();

    app.add_plugins((
        battlefield::plugin,
        building::plugin,
        combat::plugin,
        economy::plugin,
        endgame::plugin,
        units::plugin,
    ));
}
```

#### 2. Remove relocated items from `units/mod.rs`
**File**: `src/gameplay/units/mod.rs`

- Remove `Team`, `Health` (struct + impl), `Target` definitions
- Remove their `register_type` calls from the plugin function
- Remove `BARRACKS_PRODUCTION_INTERVAL` constant
- Update tests to import from `crate::gameplay` instead of `super`

Keep in `units/mod.rs`: `Unit`, `CombatStats`, `Movement`, `CurrentTarget`, `UnitAssets`, soldier constants, `BACKTRACK_DISTANCE`.

#### 3. Add `BARRACKS_PRODUCTION_INTERVAL` to `building/mod.rs`
**File**: `src/gameplay/building/mod.rs`

Add constant (it describes building production cadence):
```rust
/// Barracks production interval in seconds.
pub const BARRACKS_PRODUCTION_INTERVAL: f32 = 3.0;
```

#### 4. Update all imports across the codebase

Every file that imports `Team`, `Health`, or `Target` from `crate::gameplay::units` changes to `crate::gameplay`:

| File | Old import | New import |
|------|-----------|------------|
| `gameplay/units/ai.rs` | `super::{Target, Team, ...}` | `crate::gameplay::{Target, Team}` + keep `super::{CurrentTarget, Unit, BACKTRACK_DISTANCE}` |
| `gameplay/units/spawn.rs` | `super::{Health, Target, Team, ...}` | `crate::gameplay::{Health, Target, Team}` + keep `super::{CombatStats, CurrentTarget, Movement, Unit, UnitAssets, ...}` |
| `gameplay/units/movement.rs` | No change (doesn't use Team/Health/Target) | No change |
| `gameplay/combat/mod.rs` | `crate::gameplay::units::{CombatStats, CurrentTarget, Health, Unit}` | `crate::gameplay::Health` + `crate::gameplay::units::{CombatStats, CurrentTarget, Unit}` |
| `gameplay/endgame.rs` | `crate::gameplay::units::Health` | `crate::gameplay::Health` |
| `gameplay/economy/income.rs` | `crate::gameplay::units::{Health, Team}` | `crate::gameplay::{Health, Team}` |
| `gameplay/building/placement.rs` | `crate::gameplay::units::{BARRACKS_PRODUCTION_INTERVAL, Target, Team}` | `crate::gameplay::{Target, Team}` + `super::BARRACKS_PRODUCTION_INTERVAL` |
| `gameplay/building/production.rs` | `crate::gameplay::units::{..., Health, Target, Team, ...}` | `crate::gameplay::{Health, Target, Team}` + `crate::gameplay::units::{CombatStats, CurrentTarget, Movement, Unit, UnitAssets, ...}` |
| `gameplay/battlefield/renderer.rs` | `crate::gameplay::units::{Health, Target, Team}` | `crate::gameplay::{Health, Target, Team}` |

Test files within these modules also need import updates where they reference `Team`, `Health`, or `Target`.

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` passes — all imports resolve
- [ ] `cargo test` passes — all existing tests pass with updated imports
- [ ] `cargo clippy` passes — no new warnings

#### Manual Verification:
- [ ] `cargo run` launches and game plays identically

**Implementation Note**: After completing this phase and all automated verification passes, pause for manual confirmation before proceeding.

---

## Phase 2: Split Combat Module + DeathCheck SystemSet

### Overview
Split `combat/mod.rs` (657 lines) into `combat/attack.rs`, `combat/death.rs`, `combat/health_bar.rs`. Add `DeathCheck` SystemSet. Make `check_death` private.

### Changes Required:

#### 1. Create `combat/attack.rs`
**File**: `src/gameplay/combat/attack.rs`

Move from `combat/mod.rs`:
- Constants: `PROJECTILE_SPEED`, `PROJECTILE_RADIUS`, `PROJECTILE_COLOR`
- Components: `AttackTimer`, `Projectile`
- Systems: `unit_attack`, `move_projectiles`
- Plugin function registering types and systems
- Tests: `create_attack_test_app`, `create_projectile_test_app`, `advance_and_update`, `spawn_attacker`, `spawn_target`, and all attack/projectile tests

```rust
//! Attack mechanics: timers, projectiles, and damage application.

use bevy::prelude::*;

use crate::gameplay::Health;
use crate::gameplay::units::{CombatStats, CurrentTarget, Unit};
use crate::screens::GameState;

// Constants, components, systems moved here...

pub(super) fn plugin(app: &mut App) {
    app.register_type::<AttackTimer>()
        .register_type::<Projectile>();

    app.add_systems(
        Update,
        (unit_attack, move_projectiles)
            .chain_ignore_deferred()
            .in_set(crate::GameSet::Combat)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );
}
```

#### 2. Create `combat/death.rs`
**File**: `src/gameplay/combat/death.rs`

Move from `combat/mod.rs`:
- System: `check_death` (now private)
- New: `DeathCheck` SystemSet

```rust
//! Death detection: despawns entities at zero health.

use bevy::prelude::*;

use crate::gameplay::Health;
use crate::screens::GameState;

/// SystemSet for death detection. Other systems can order against this
/// (e.g., `.before(DeathCheck)`) instead of referencing the function directly.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeathCheck;

/// Despawns any entity whose health drops to 0 or below.
fn check_death(mut commands: Commands, query: Query<(Entity, &Health)>) {
    for (entity, health) in &query {
        if health.current <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        check_death
            .in_set(DeathCheck)
            .in_set(crate::GameSet::Death)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );
}
```

#### 3. Create `combat/health_bar.rs`
**File**: `src/gameplay/combat/health_bar.rs`

Move from `combat/mod.rs`:
- Constants: `HEALTH_BAR_BG_COLOR`, `HEALTH_BAR_FILL_COLOR`, `UNIT_HEALTH_BAR_WIDTH`, `UNIT_HEALTH_BAR_HEIGHT`, `UNIT_HEALTH_BAR_Y_OFFSET`
- Components: `HealthBarBackground`, `HealthBarFill`, `HealthBarConfig`
- Systems: `spawn_health_bars`, `update_health_bars`
- Plugin function registering types and systems
- Tests: `create_health_bar_test_app`, `unit_health_bar_config`, and all health bar tests

```rust
//! Health bar rendering: spawns and updates visual health indicators.

use bevy::prelude::*;

use crate::gameplay::Health;
use crate::screens::GameState;

// Constants, components, systems moved here...

pub(super) fn plugin(app: &mut App) {
    app.register_type::<HealthBarBackground>()
        .register_type::<HealthBarFill>()
        .register_type::<HealthBarConfig>();

    app.add_systems(
        Update,
        (spawn_health_bars, update_health_bars)
            .chain_ignore_deferred()
            .in_set(crate::GameSet::Ui)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );
}
```

#### 4. Rewrite `combat/mod.rs` as compositor
**File**: `src/gameplay/combat/mod.rs`

```rust
//! Combat systems: attack mechanics, death detection, and health bars.

mod attack;
mod death;
mod health_bar;

pub use attack::{AttackTimer, Projectile};
pub use death::DeathCheck;
pub use health_bar::{
    HealthBarBackground, HealthBarConfig, HealthBarFill, UNIT_HEALTH_BAR_HEIGHT,
    UNIT_HEALTH_BAR_WIDTH, UNIT_HEALTH_BAR_Y_OFFSET,
};

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    attack::plugin(app);
    death::plugin(app);
    health_bar::plugin(app);
}
```

#### 5. Update ordering references to use `DeathCheck` set

**File**: `src/gameplay/endgame.rs`
```rust
// Old:
.before(crate::gameplay::combat::check_death)
// New:
.before(crate::gameplay::combat::DeathCheck)
```

**File**: `src/gameplay/economy/income.rs`
```rust
// Old:
.before(crate::gameplay::combat::check_death)
// New:
.before(crate::gameplay::combat::DeathCheck)
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` passes
- [ ] `cargo test` passes — all combat tests pass in their new locations
- [ ] `cargo clippy` passes
- [ ] `check_death` is no longer `pub` — verify no external references

#### Manual Verification:
- [ ] Game runs identically — combat, death, health bars all work
- [ ] Endgame detection still triggers correctly (ordering preserved)
- [ ] Kill gold still awarded (ordering preserved)

**Implementation Note**: Pause for manual verification before proceeding.

---

## Phase 3: `gameplay_running()` Helper + Visibility Fixes

### Overview
Extract the repeated `in_state(GameState::InGame).and(in_state(Menu::None))` into a shared run condition. Fix module visibility. Add `GameSet::Input` to `open_pause_menu`.

### Changes Required:

#### 1. Add `gameplay_running` helper to `lib.rs`
**File**: `src/lib.rs`

Add after the `GameSet` enum:

```rust
use screens::GameState;
use menus::Menu;

/// Run condition: true when gameplay is active (InGame state, no menu open).
/// Use with `.run_if(gameplay_running)` on gameplay systems.
pub(crate) fn gameplay_running(
    game_state: Res<State<GameState>>,
    menu: Res<State<Menu>>,
) -> bool {
    game_state.get() == &GameState::InGame && menu.get() == &Menu::None
}
```

#### 2. Replace all `run_if(in_state(...).and(in_state(...)))` with `run_if(gameplay_running)`

Update all 9 files (15+ occurrences):

| File | Systems affected |
|------|-----------------|
| `gameplay/battlefield/mod.rs` | `camera_pan` |
| `gameplay/building/mod.rs` | `update_grid_cursor` chain, `tick_production_and_spawn_units` |
| `gameplay/units/mod.rs` | `unit_find_target`, `unit_movement` |
| `gameplay/combat/attack.rs` | `unit_attack` + `move_projectiles` chain |
| `gameplay/combat/death.rs` | `check_death` |
| `gameplay/combat/health_bar.rs` | `spawn_health_bars` + `update_health_bars` chain |
| `gameplay/endgame.rs` | `detect_endgame` |
| `gameplay/economy/income.rs` | `tick_farm_income`, `award_kill_gold` |
| `gameplay/economy/shop_ui.rs` | `handle_card_click` + `handle_reroll_click`, `update_card_visuals` + `update_card_text` + `update_reroll_text` |
| `gameplay/economy/ui.rs` | `update_gold_display` |
| `screens/in_game.rs` | `open_pause_menu` |

Each changes from:
```rust
.run_if(in_state(GameState::InGame).and(in_state(Menu::None)))
// or
.run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None)))
```
To:
```rust
.run_if(crate::gameplay_running)
```

Remove now-unused `use crate::menus::Menu;` imports in files that only used `Menu` for the run condition.

#### 3. Add `GameSet::Input` to `open_pause_menu`
**File**: `src/screens/in_game.rs`

```rust
// Old:
open_pause_menu.run_if(crate::gameplay_running),
// New:
open_pause_menu
    .in_set(crate::GameSet::Input)
    .run_if(crate::gameplay_running),
```

#### 4. Fix module visibility in `lib.rs`
**File**: `src/lib.rs`

```rust
// Old:
#[cfg(feature = "dev")]
pub mod dev_tools;
pub mod gameplay;
pub mod menus;
pub mod screens;
#[cfg(test)]
pub mod testing;
pub mod theme;
pub mod ui_camera;

// New:
#[cfg(feature = "dev")]
pub(crate) mod dev_tools;
pub(crate) mod gameplay;
pub(crate) mod menus;
pub(crate) mod screens;
#[cfg(test)]
pub(crate) mod testing;
pub(crate) mod theme;
pub(crate) mod ui_camera;
```

Note: `pub fn plugin` stays `pub` (needed by `main.rs`).

#### 5. Fix module visibility in `units/mod.rs`
**File**: `src/gameplay/units/mod.rs`

```rust
// Old:
pub mod spawn;
// New:
pub(crate) mod spawn;
```

#### 6. Fix module visibility in `economy/mod.rs`
**File**: `src/gameplay/economy/mod.rs`

```rust
// Old:
pub mod income;
pub mod shop;
// New:
pub(crate) mod income;
pub(crate) mod shop;
```

#### 7. Update ARCHITECTURE.md
**File**: `ARCHITECTURE.md`

Update visibility rules table:
```
| Module declarations in `lib.rs` | `pub(crate)` | `pub(crate) mod gameplay;` |
```

Update "Component Co-Location" section:
```
Cross-cutting components (e.g., `Health`, `Team`, `Target`) live in the common
parent `gameplay/mod.rs`. Domain-specific components remain in their plugins.
```

Update "Current modules" tree to include combat submodules:
```
│   ├── combat/         # Attack, death detection, health bars
│   │   ├── mod.rs      # Compositor + re-exports
│   │   ├── attack.rs   # AttackTimer, Projectile, unit_attack, move_projectiles
│   │   ├── death.rs    # DeathCheck SystemSet, check_death
│   │   └── health_bar.rs # HealthBarConfig, spawn/update health bars
```

Add `gameplay_running` documentation:
```
### Shared run condition

All gameplay systems use the `gameplay_running` run condition from `lib.rs`:

\`\`\`rust
.run_if(crate::gameplay_running)
\`\`\`

This replaces the repeated `in_state(GameState::InGame).and(in_state(Menu::None))` pattern.
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes — no `redundant_pub_crate` warnings from visibility changes

#### Manual Verification:
- [ ] Game pauses correctly when ESC is pressed (open_pause_menu still works)
- [ ] All gameplay systems pause when menu is open

**Implementation Note**: Pause for manual verification before proceeding.

---

## Phase 4: Entity Naming

### Overview
Add `Name::new(...)` to all spawned entities for Bevy inspector support.

### Changes Required:

#### 1. Battlefield entities
**File**: `src/gameplay/battlefield/renderer.rs`

| Entity | Name |
|--------|------|
| Background | `Name::new("Battlefield Background")` |
| Player fortress | `Name::new("Player Fortress")` |
| Build zone | `Name::new("Build Zone")` |
| Combat zone | `Name::new("Combat Zone")` |
| Enemy fortress | `Name::new("Enemy Fortress")` |
| Build slots | `Name::new(format!("Build Slot ({col}, {row}"))` |

#### 2. Building entities
**File**: `src/gameplay/building/placement.rs`

| Entity | Name |
|--------|------|
| Grid cursor | `Name::new("Grid Cursor")` |
| Building | `Name::new(format!("{:?}", building_type))` |

#### 3. Unit entities
**File**: `src/gameplay/building/production.rs`

| Entity | Name |
|--------|------|
| Player unit | `Name::new("Player Soldier")` |

**File**: `src/gameplay/units/spawn.rs`

| Entity | Name |
|--------|------|
| Enemy unit | `Name::new("Enemy Soldier")` |

#### 4. Combat entities
**File**: `src/gameplay/combat/attack.rs` (after Phase 2 split)

| Entity | Name |
|--------|------|
| Projectile | `Name::new("Projectile")` |

**File**: `src/gameplay/combat/health_bar.rs` (after Phase 2 split)

| Entity | Name |
|--------|------|
| Health bar background | `Name::new("Health Bar BG")` |
| Health bar fill | `Name::new("Health Bar Fill")` |

#### 5. Economy UI entities
**File**: `src/gameplay/economy/shop_ui.rs`

| Entity | Name |
|--------|------|
| Shop panel | `Name::new("Shop Panel")` |
| Card slot | `Name::new(format!("Card Slot {i}"))` |
| Card name text | `Name::new(format!("Card {i} Name"))` |
| Card cost text | `Name::new(format!("Card {i} Cost"))` |
| Reroll button | `Name::new("Reroll Button")` |
| Reroll text | `Name::new("Reroll Text")` |

**File**: `src/gameplay/economy/ui.rs`

| Entity | Name |
|--------|------|
| Gold display | `Name::new("Gold Display")` |

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes

#### Manual Verification:
- [ ] Run with `bevy-inspector-egui` (if available) — all entities show meaningful names
- [ ] No visual changes in-game

**Implementation Note**: Pause for manual verification before proceeding.

---

## Phase 5: Stagger Retargeting + UI Click-Through Fix

### Overview
Distribute retargeting across frames using entity index. Prevent shop clicks from placing buildings.

### Changes Required:

#### 1. Stagger `unit_find_target` retargeting
**File**: `src/gameplay/units/ai.rs`

Replace the global counter logic:

```rust
// Old:
*counter += 1;
let should_retarget = *counter >= RETARGET_INTERVAL_FRAMES;
if should_retarget {
    *counter = 0;
}

for (entity, team, transform, mut current_target) in &mut units {
    let has_valid_target = current_target.0.is_some_and(|e| all_targets.get(e).is_ok());

    // Throttle: skip units with valid targets on non-retarget frames
    if has_valid_target && !should_retarget {
        continue;
    }
    // ...
}

// New:
*counter = counter.wrapping_add(1);

for (entity, team, transform, mut current_target) in &mut units {
    let has_valid_target = current_target.0.is_some_and(|e| all_targets.get(e).is_ok());

    // Stagger: each unit retargets on a different frame based on its entity index
    let should_retarget =
        (entity.index().wrapping_add(*counter)) % RETARGET_INTERVAL_FRAMES == 0;
    if has_valid_target && !should_retarget {
        continue;
    }
    // ... rest unchanged
}
```

This distributes 200 units across 10 frames = ~20 retargets per frame. Zero storage cost.

Existing tests continue to work because:
- Units without targets always evaluate (bypass stagger)
- Running `RETARGET_INTERVAL_FRAMES` updates guarantees every entity hits its retarget frame

#### 2. Fix UI click-through
**File**: `src/gameplay/building/placement.rs`

Add a `Button` interaction query to `handle_building_placement`:

```rust
pub(super) fn handle_building_placement(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredCell>,
    grid_index: Res<GridIndex>,
    occupied: Query<(), With<Occupied>>,
    mut gold: ResMut<crate::gameplay::economy::Gold>,
    mut shop: ResMut<crate::gameplay::economy::shop::Shop>,
    ui_buttons: Query<&Interaction, With<Button>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    // Skip if mouse is over any UI button (prevents click-through from shop panel)
    if ui_buttons.iter().any(|i| *i != Interaction::None) {
        return;
    }

    // ... rest unchanged
}
```

This checks `Button` entities specifically (shop cards, reroll button). Non-button UI nodes (gold display) don't block placement.

#### 3. Add test for UI click-through fix
**File**: `src/gameplay/building/placement.rs` (in `integration_tests` module)

```rust
#[test]
fn clicking_ui_button_does_not_place_building() {
    let mut app = create_placement_test_app();

    // Simulate a UI button being pressed (prevents click-through)
    app.world_mut().spawn((Button, Interaction::Pressed));

    app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();

    assert_entity_count::<With<Building>>(&mut app, 0);
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` passes
- [ ] `cargo test` passes — existing AI tests pass, new click-through test passes
- [ ] `cargo clippy` passes

#### Manual Verification:
- [ ] Units still find targets and switch to closer enemies
- [ ] Clicking shop cards does NOT place buildings on the grid behind them
- [ ] Clicking empty grid cells still places buildings normally

**Implementation Note**: Pause for manual verification before proceeding.

---

## Phase 6: Convention Cleanup

### Overview
Fix remaining convention gaps: unused constants, derives, import style, module naming.

### Changes Required:

#### 1. Remove `Z_HEALTH_BAR` constant
**File**: `src/lib.rs`

Remove the constant and its `#[allow(dead_code)]`:
```rust
// Remove:
/// Health bars (future: Ticket 5).
#[allow(dead_code)]
pub(crate) const Z_HEALTH_BAR: f32 = 5.0;
```

Update `z_layers_are_ordered` test to remove the `Z_HEALTH_BAR` assertion.

Health bars use relative child offsets (1.0, 1.1) which are local to the parent entity, so an absolute Z constant is incorrect.

#### 2. Add `Copy` derive to `BuildSlot`
**File**: `src/gameplay/battlefield/mod.rs`

```rust
// Old:
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct BuildSlot {
// New:
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct BuildSlot {
```

Both fields (`row: u16`, `col: u16`) are `Copy`.

#### 3. Rename `endgame.rs` → `endgame_detection.rs`
**File**: rename `src/gameplay/endgame.rs` → `src/gameplay/endgame_detection.rs`

Update `gameplay/mod.rs`:
```rust
// Old:
pub(crate) mod endgame;
// ...
endgame::plugin,
// New:
pub(crate) mod endgame_detection;
// ...
endgame_detection::plugin,
```

This prevents confusion with `menus/endgame.rs` (the UI overlay for victory/defeat).

#### 4. Fix inline import style
**File**: Various — scan for inline `crate::` paths in system parameters and replace with `use` imports at the top of the file.

Known instances:
- `gameplay/units/mod.rs`: `crate::GameSet::Ai`, `crate::menus::Menu::None` → add to `use` block
- `gameplay/building/mod.rs`: `crate::GameSet::Input`, `crate::GameSet::Production` → add to `use` block
- `gameplay/economy/income.rs`: `crate::GameSet::Production`, `crate::GameSet::Death` → add to `use` block

After Phase 3 replaces `run_if(in_state(...))` with `gameplay_running`, many inline `crate::menus::Menu` references will already be gone. This phase catches remaining inline `crate::` patterns.

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` passes
- [ ] `cargo test` passes
- [ ] `cargo clippy` passes — no dead_code warnings on Z constants

#### Manual Verification:
- [ ] Game runs identically
- [ ] No visual or behavioral changes

---

## Testing Strategy

### Approach
This is a refactor — existing tests should continue to pass with updated imports. New tests added:

| Test | Location | What it verifies |
|------|----------|-----------------|
| `clicking_ui_button_does_not_place_building` | `building/placement.rs` | UI click-through fix |
| `gameplay_running` unit tests | `lib.rs` | Run condition returns correct bool |

### Existing test updates
- Import paths change from `crate::gameplay::units::{Health, Team, Target}` to `crate::gameplay::{Health, Team, Target}`
- Combat tests move to their respective submodules (no logic changes)
- `endgame.rs` tests move to `endgame_detection.rs` (no logic changes)

### Coverage
No coverage regression expected — same tests, just relocated. New tests for `gameplay_running` and UI click-through add small coverage increase.

## Performance Considerations

- **Staggered retargeting**: Distributes N units across 10 frames instead of processing all N on the same frame. With 200 units and 222 targets, reduces worst-case per-frame distance calculations from 44,400 to ~4,440.
- **`gameplay_running` function**: Trivial cost — reads two `Res<State<T>>` and compares. Same as the `in_state` combinators it replaces.

## References

- Linear ticket: [GAM-11](https://linear.app/tayhu-games/issue/GAM-11/refactor-components-and-system)
- Architecture doc: `ARCHITECTURE.md`
- Blocks: GAM-15 (fortress overhaul), GAM-16 (unit/building selection), GAM-21 (building health), GAM-22 (visual polish)

# Consolidate Tier-2 Integration Test Helpers (GAM-35)

## Overview

Consolidate duplicated test helpers (timer near-expiry, entity spawn, resource init) into `src/testing.rs`, migrate all existing tests to use the shared versions, and add integration tests for uncovered input-handler modules.

## Current State Analysis

**`src/testing.rs`** — 6 existing helpers: `create_test_app`, `create_test_app_with_state`, `create_base_test_app`, `create_base_test_app_no_input`, `transition_to_ingame`, `count_entities`, `assert_entity_count`, `tick_multiple`.

**Duplicated timer near-expiry** — 5 modules, 4 forms:
- `income.rs:79-83` — `nearly_elapsed_income_timer()` returns `IncomeTimer` (0.001s, 999_000ns)
- `production.rs:90-94` — `nearly_elapsed_timer()` returns raw `Timer` (0.001s, 999_000ns)
- `spawn.rs:233-239` — `nearly_expire_timer(app)` mutates `EnemySpawnTimer` resource (`duration - 1ns`)
- `attack.rs:251-252` — inline in `spawn_attacker` (0.001s, 999_000ns)
- `ai.rs:175-188` — `set_retarget_for_entity(app, entity)` mutates `RetargetTimer` (`duration - 1ns`)

**Duplicated entity spawn helpers** — 3 modules:
- `movement.rs:113-142` — `spawn_unit_at(world, x, speed, target)`, `spawn_target_at(world, x)`
- `ai.rs:136-162` — `spawn_unit(world, team, x, y)`, `spawn_target(world, team, x, y)`
- `attack.rs:249-284` — `spawn_attacker(world, x, target)`, `spawn_target(world, x, hp)`

**Duplicated resource init** — 4+ modules:
- `Assets<Mesh>` + `Assets<ColorMaterial>` in `units/mod.rs:285-286`, `placement.rs:178-179`, `production.rs:71-72`, `economy/mod.rs:107-108,119-120,146-147`
- `Gold` + `Shop` in `placement.rs:180-181`, `production.rs:73-74`
- `ButtonInput<KeyCode>` + `ButtonInput<MouseButton>` in `production.rs:68-70`, `placement.rs:238-240`

**Uncovered modules** (practical candidates):
- `screens/in_game.rs` — `open_pause_menu` (1 system, feasible)
- `menus/main_menu.rs` — `handle_main_menu_input` (1 system, feasible)
- `menus/pause.rs` — `handle_pause_input` (1 system, feasible)
- `economy/ui.rs` — `update_gold_display` (1 system, feasible)
- Camera/renderer/theme/compositors/dev_tools — no testable logic or require render pipeline (skip)

### Key Discoveries:
- All timer helpers do the same thing: set elapsed to `duration - epsilon` so the next tick triggers `just_finished()`
- Entity spawn helpers differ in component sets but share a core pattern (Transform + GlobalTransform + Collider)
- The `#[cfg(test)]` testing module at crate root can import from all `pub(crate)` gameplay modules without circular deps
- `gameplay/units/mod.rs:87-143` defines the canonical unit archetype — test helpers should mirror it minus render/physics-layer components

## Desired End State

- `testing.rs` exports: `nearly_expire_timer`, `init_asset_resources`, `init_economy_resources`, `init_input_resources`, `spawn_test_unit`, `spawn_test_target`
- All per-module timer/entity/resource-init duplicates are removed
- 4 new test modules added (screens/in_game, menus/main_menu, menus/pause, economy/ui)
- `make check` and `make test` pass with no reduction in test count

## What We're NOT Doing

- Refactoring test app setup patterns (e.g., `create_base_test_app` variations) — existing pattern works fine
- Adding tests for camera.rs, renderer.rs, or theme/ — these require render pipeline or have no logic
- Adding tests for dev_tools/ — feature-gated, requires Gizmos/render infrastructure
- Creating a builder pattern for test entities — too much abstraction for the current scale
- Adding `spawn_test_projectile` to shared helpers — only used in attack.rs, too specialized

## Implementation Approach

Three phases: (1) add shared helpers, (2) migrate existing tests, (3) add new tests. Each phase is independently verifiable.

---

## Phase 1: Add Shared Helpers to `testing.rs`

### Overview
Add 6 new helpers to `src/testing.rs` covering the three categories of duplication.

### Changes Required:

#### 1. Timer Helper
**File**: `src/testing.rs`

```rust
use std::time::Duration;

/// Set a timer's elapsed to `duration - 1ns` so the next `tick()` with any
/// positive delta triggers `just_finished()`.
///
/// Works for any `Timer` regardless of duration or mode.
pub fn nearly_expire_timer(timer: &mut Timer) {
    let duration = timer.duration();
    timer.set_elapsed(duration - Duration::from_nanos(1));
}
```

#### 2. Resource Init Helpers
**File**: `src/testing.rs`

```rust
/// Init `Assets<Mesh>` and `Assets<ColorMaterial>` — needed by any test that
/// uses `UnitAssets` or spawns mesh-based entities.
pub fn init_asset_resources(app: &mut App) {
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<ColorMaterial>>();
}

/// Init `Gold` and `Shop` resources — needed by building placement and
/// production tests.
pub fn init_economy_resources(app: &mut App) {
    app.init_resource::<crate::gameplay::economy::Gold>();
    app.init_resource::<crate::gameplay::economy::shop::Shop>();
}

/// Init `ButtonInput<KeyCode>` and `ButtonInput<MouseButton>` — needed when
/// `InputPlugin` is skipped to avoid `just_pressed` being cleared in `PreUpdate`.
pub fn init_input_resources(app: &mut App) {
    app.init_resource::<ButtonInput<KeyCode>>();
    app.init_resource::<ButtonInput<MouseButton>>();
}
```

#### 3. Entity Spawn Helpers
**File**: `src/testing.rs`

```rust
use avian2d::prelude::*;
use crate::gameplay::{Team, Target, CurrentTarget, Movement, CombatStats, Health};
use crate::gameplay::combat::AttackTimer;
use crate::gameplay::units::{Unit, UnitType, unit_stats, UNIT_RADIUS};
use crate::gameplay::units::pathfinding::NavPath;

/// Spawn a test unit with the full Soldier archetype at `(x, y)`.
///
/// Includes: Unit, UnitType::Soldier, Team, Target, CurrentTarget(None),
/// Health, CombatStats, Movement, AttackTimer, Transform, GlobalTransform,
/// Collider, LinearVelocity, NavPath.
///
/// Callers can override specific components via `world.entity_mut(id).insert(...)`.
pub fn spawn_test_unit(world: &mut World, team: Team, x: f32, y: f32) -> Entity {
    let stats = unit_stats(UnitType::Soldier);
    world
        .spawn((
            Unit,
            UnitType::Soldier,
            team,
            Target,
            CurrentTarget(None),
            Health::new(stats.hp),
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
            Transform::from_xyz(x, y, 0.0),
            GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
            Collider::circle(UNIT_RADIUS),
            LinearVelocity::ZERO,
            NavPath::default(),
        ))
        .id()
}

/// Spawn a non-unit targetable entity at `(x, y)`.
///
/// Includes: Team, Target, Transform, GlobalTransform, Collider (5px radius).
/// Add `Health` via `world.entity_mut(id).insert(Health::new(hp))` for attack tests.
pub fn spawn_test_target(world: &mut World, team: Team, x: f32, y: f32) -> Entity {
    world
        .spawn((
            team,
            Target,
            Transform::from_xyz(x, y, 0.0),
            GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
            Collider::circle(5.0),
        ))
        .id()
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (no compile errors, no clippy warnings)
- [ ] `make test` passes (no test regressions)

#### Manual Verification:
- [ ] Review `testing.rs` — helpers are clear, well-documented, and follow existing style

**Implementation Note**: After completing this phase, pause for verification before proceeding.

---

## Phase 2: Migrate Existing Tests to Shared Helpers

### Overview
Replace per-module duplicates with calls to the shared helpers. Delete the old local helpers. Module by module:

### Changes Required:

#### 1. `gameplay/economy/income.rs` — Timer + Resource
**Current** (`income.rs:72,79-83`):
```rust
app.init_resource::<Gold>();
// ...
fn nearly_elapsed_income_timer() -> IncomeTimer {
    let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
    timer.set_elapsed(Duration::from_nanos(999_000));
    IncomeTimer(timer)
}
```
**After**: Delete `nearly_elapsed_income_timer`. Use:
```rust
fn nearly_elapsed_income_timer() -> IncomeTimer {
    let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
    crate::testing::nearly_expire_timer(&mut timer);
    IncomeTimer(timer)
}
```
Note: Keep the local wrapper since it returns `IncomeTimer`, not a raw `Timer`. The inner timer creation uses the shared helper.

#### 2. `gameplay/building/production.rs` — Timer + Resources
**Current** (`production.rs:68-74, 90-94`):
```rust
app.init_resource::<ButtonInput<KeyCode>>()
   .init_resource::<ButtonInput<MouseButton>>();
app.init_resource::<Assets<Mesh>>();
app.init_resource::<Assets<ColorMaterial>>();
app.init_resource::<crate::gameplay::economy::Gold>();
app.init_resource::<crate::gameplay::economy::shop::Shop>();
// ...
fn nearly_elapsed_timer() -> Timer {
    let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
    timer.set_elapsed(Duration::from_nanos(999_000));
    timer
}
```
**After**: Delete `nearly_elapsed_timer`. Replace resource init with shared helpers:
```rust
crate::testing::init_input_resources(&mut app);
crate::testing::init_asset_resources(&mut app);
crate::testing::init_economy_resources(&mut app);
// ...
fn nearly_elapsed_timer() -> Timer {
    let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
    crate::testing::nearly_expire_timer(&mut timer);
    timer
}
```
Note: Keep `nearly_elapsed_timer` as a thin wrapper since it constructs a 0.001s timer specifically for production tests. The shared helper handles the elapsed-setting.

#### 3. `gameplay/units/spawn.rs` — Timer
**Current** (`spawn.rs:233-239`):
```rust
fn nearly_expire_timer(app: &mut App) {
    let duration = app.world().resource::<EnemySpawnTimer>().timer.duration();
    app.world_mut()
        .resource_mut::<EnemySpawnTimer>()
        .timer
        .set_elapsed(duration - Duration::from_nanos(1));
}
```
**After**: Rewrite to use shared helper:
```rust
fn nearly_expire_spawn_timer(app: &mut App) {
    crate::testing::nearly_expire_timer(
        &mut app.world_mut().resource_mut::<EnemySpawnTimer>().timer,
    );
}
```
Update call sites from `nearly_expire_timer(app)` → `nearly_expire_spawn_timer(app)` (renamed to avoid shadowing the shared fn).

#### 4. `gameplay/combat/attack.rs` — Timer + Entity helpers
**Current** (`attack.rs:249-284`): `spawn_attacker` with inline timer, `spawn_target` with Health.
**After**: Refactor `spawn_attacker` to use shared helpers:
```rust
fn spawn_attacker(world: &mut World, x: f32, target: Option<Entity>) -> Entity {
    let id = crate::testing::spawn_test_unit(world, Team::Player, x, 100.0);
    if let Some(t) = target {
        world.entity_mut(id).insert(CurrentTarget(Some(t)));
    }
    // Nearly-expire the attack timer for immediate attack
    crate::testing::nearly_expire_timer(
        &mut world.entity_mut(id).get_mut::<AttackTimer>().unwrap().0,
    );
    id
}
```
Replace local `spawn_target`:
```rust
fn spawn_target(world: &mut World, x: f32, hp: f32) -> Entity {
    let id = crate::testing::spawn_test_target(world, Team::Enemy, x, 100.0);
    world.entity_mut(id).insert(Health::new(hp));
    id
}
```
Keep `spawn_test_projectile` as-is (too specialized).

Also replace the inline timer in `fortress_can_attack_in_range` test (`attack.rs:529-530`):
```rust
let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
crate::testing::nearly_expire_timer(&mut timer);
```

#### 5. `gameplay/ai.rs` — Timer + Entity helpers
**Current** (`ai.rs:136-162, 175-188`): `spawn_unit`, `spawn_target`, `set_retarget_for_entity`.
**After**: Replace entity helpers:
```rust
fn spawn_unit(world: &mut World, team: Team, x: f32, y: f32) -> Entity {
    crate::testing::spawn_test_unit(world, team, x, y)
}

fn spawn_target(world: &mut World, team: Team, x: f32, y: f32) -> Entity {
    crate::testing::spawn_test_target(world, team, x, y)
}
```
These thin wrappers can be inlined at call sites instead of kept as local fns. Prefer inlining to reduce code.

Refactor `set_retarget_for_entity`:
```rust
fn set_retarget_for_entity(app: &mut App, entity: Entity) {
    let entity_slot = entity.index().index() % RETARGET_SLOTS;
    let prev_slot = if entity_slot == 0 {
        RETARGET_SLOTS - 1
    } else {
        entity_slot - 1
    };
    let mut timer = app.world_mut().resource_mut::<RetargetTimer>();
    timer.current_slot = prev_slot;
    crate::testing::nearly_expire_timer(&mut timer.timer);
}
```

#### 6. `gameplay/units/movement.rs` — Entity helpers
**Current** (`movement.rs:113-142`): `spawn_unit_at(world, x, speed, target)`, `spawn_target_at(world, x)`.
**After**: Replace with shared helpers + overrides:
```rust
fn spawn_unit_at(world: &mut World, x: f32, speed: f32, target: Option<Entity>) -> Entity {
    let id = crate::testing::spawn_test_unit(world, Team::Player, x, 100.0);
    world.entity_mut(id).insert((
        Movement { speed },
        CurrentTarget(target),
    ));
    id
}

fn spawn_target_at(world: &mut World, x: f32) -> Entity {
    crate::testing::spawn_test_target(world, Team::Player, x, 100.0)
}
```
Keep thin wrappers since `spawn_unit_at` customizes speed and target — the wrapper is cleaner than inlining overrides at every call site.

#### 7. `gameplay/units/mod.rs` — Resource init
**Current** (`units/mod.rs:285-286`):
```rust
app.init_resource::<Assets<Mesh>>();
app.init_resource::<Assets<ColorMaterial>>();
```
**After**:
```rust
crate::testing::init_asset_resources(&mut app);
```

#### 8. `gameplay/building/placement.rs` — Resource init
**Current** (`placement.rs:178-181, 238-245`): Two setup functions with assets + economy + input init.
**After**: Replace with shared helpers:
```rust
crate::testing::init_asset_resources(&mut app);
crate::testing::init_economy_resources(&mut app);
// ...
crate::testing::init_input_resources(&mut app);
crate::testing::init_economy_resources(&mut app);
```

#### 9. `gameplay/economy/mod.rs` — Resource init
**Current** (`economy/mod.rs:107-108, 119-120, 146-147`): Assets init repeated 3 times.
**After**: Replace all 3 with `crate::testing::init_asset_resources(&mut app);`.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — same test count, no regressions
- [ ] No timer `set_elapsed` calls remain outside `testing.rs` (except wrapped calls to `nearly_expire_timer`)
- [ ] No `init_resource::<Assets<Mesh>>()` or `init_resource::<Assets<ColorMaterial>>()` calls remain outside `testing.rs`

#### Manual Verification:
- [ ] Spot-check 3 migrated modules to verify helpers are used correctly
- [ ] Verify no test semantics changed (same assertions, same entity setups)

**Implementation Note**: After completing this phase and all automated verification passes, pause for confirmation before proceeding.

---

## Phase 3: Add Tests for Uncovered Modules

### Overview
Add integration tests to 4 modules that currently have no tests. All are thin input handlers or UI update systems.

### Changes Required:

#### 1. `screens/in_game.rs` — `open_pause_menu`
**File**: `src/screens/in_game.rs`

```rust
#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use crate::menus::Menu;

    #[test]
    fn escape_opens_pause_menu() {
        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        app.add_systems(Update, super::open_pause_menu);
        crate::testing::transition_to_ingame(&mut app);

        // Press Escape
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        let next = app.world().resource::<NextState<Menu>>();
        // NextState should contain Menu::Pause
        assert!(
            matches!(next.as_ref(), NextState(bevy::state::state::Pending::Set(Menu::Pause))),
            "Expected NextState to be Menu::Pause"
        );
    }
}
```

Note: The exact `NextState` assertion pattern depends on Bevy 0.18's `NextState` internals. Will verify against source during implementation.

#### 2. `menus/main_menu.rs` — `handle_main_menu_input`
**File**: `src/menus/main_menu.rs`

```rust
#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use crate::menus::Menu;
    use crate::screens::GameState;

    #[test]
    fn space_starts_game() {
        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        app.add_systems(Update, super::handle_main_menu_input);

        // Press Space
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();

        // Verify state transitions
        // NextState<GameState> should be InGame
        // NextState<Menu> should be None
    }
}
```

#### 3. `menus/pause.rs` — `handle_pause_input`
**File**: `src/menus/pause.rs`

```rust
#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use crate::menus::Menu;
    use crate::screens::GameState;

    #[test]
    fn escape_unpauses() {
        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        app.add_systems(Update, super::handle_pause_input);
        crate::testing::transition_to_ingame(&mut app);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        // NextState<Menu> should be Menu::None
    }

    #[test]
    fn q_quits_to_main_menu() {
        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        app.add_systems(Update, super::handle_pause_input);
        crate::testing::transition_to_ingame(&mut app);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.update();

        // NextState<GameState> should be GameState::MainMenu
    }
}
```

#### 4. `gameplay/economy/ui.rs` — `update_gold_display`
**File**: `src/gameplay/economy/ui.rs`

```rust
#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::GoldDisplay;
    use crate::gameplay::economy::Gold;

    #[test]
    fn gold_display_updates_on_change() {
        let mut app = crate::testing::create_test_app();
        app.init_resource::<Gold>();
        app.add_systems(Update, super::update_gold_display);

        // Spawn a GoldDisplay entity
        app.world_mut()
            .spawn((Text::new("Gold: 0"), GoldDisplay));
        app.update();

        // Change gold
        app.world_mut().resource_mut::<Gold>().0 = 999;
        app.update();

        // Verify text updated
        let text = app
            .world_mut()
            .query_filtered::<&Text, With<GoldDisplay>>()
            .single(app.world())
            .unwrap();
        assert_eq!(text.0, "Gold: 999");
    }
}
```

Note: `Text::new("Gold: 999")` display format must match what `update_gold_display` produces. Will verify exact format during implementation.

### Modules Assessed and Skipped:
- **`battlefield/camera.rs`** — Requires `Camera2d` + `Projection` + `Window` entities from render pipeline. `Single<>` queries fail without them. Not feasible with `MinimalPlugins`.
- **`battlefield/renderer.rs`** — Spawn logic already integration-tested via `battlefield/mod.rs`. No standalone logic to test.
- **`screens/mod.rs`, `screens/loading.rs`, `screens/main_menu.rs`** — Pure state definitions or trivial one-liners (`NextState::set`). Already covered by `lib.rs` state tests.
- **`theme/`** — Constants and bundle constructors. No logic.
- **`third_party/mod.rs`, `third_party/vleue_navigator.rs`** — Plugin wiring, no testable logic.
- **`gameplay/mod.rs`, `gameplay/combat/mod.rs`** — Compositors, no logic of their own.
- **`dev_tools/`** — Feature-gated, requires Gizmos/render infrastructure. Skipped.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — test count increased by at least 4 new tests
- [ ] All new tests pass individually: `cargo test --lib screens::in_game::tests`, `cargo test --lib menus::main_menu::tests`, `cargo test --lib menus::pause::tests`, `cargo test --lib gameplay::economy::ui::tests`

#### Manual Verification:
- [ ] Review new tests — they exercise the system logic correctly
- [ ] Confirm no false positives (tests that pass for wrong reasons)

---

## Testing Strategy

### Unit Tests:
- No new unit tests needed — the shared helpers are tested implicitly through their usage in existing integration tests

### Integration Tests:
- 4 new test modules added in Phase 3
- All existing integration tests remain, using shared helpers instead of local duplicates

### Verification Command:
```bash
# Full suite
make check && make test

# Count tests before/after
cargo test --lib 2>&1 | tail -1
```

## Performance Considerations

None — this is a pure refactoring of test infrastructure with no runtime impact.

## References

- Linear ticket: [GAM-35](https://linear.app/tayhu-games/issue/GAM-35/consolidate-tier-2-integration-test-helpers-into-shared-testingrs)
- Related: [GAM-29](https://linear.app/tayhu-games/issue/GAM-29) (hitbox/hurtbox collision layer wiring test)
- Existing test infrastructure: `src/testing.rs`
- Canonical unit archetype: `src/gameplay/units/mod.rs:87-143`

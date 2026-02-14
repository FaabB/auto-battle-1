# Victory/Defeat & Game Loop Implementation Plan

## Overview

Implement win/lose conditions, end screen overlays, and return-to-menu — completing the full playable game loop. When a fortress is destroyed, the game freezes with the battlefield visible behind a semi-transparent overlay showing the result. The player then returns to the main menu and can start a new game.

## Current State Analysis

- **Fortress entities** have `Health`, marker components (`PlayerFortress`/`EnemyFortress`), `Target`, and `Team` — spawned in `src/gameplay/battlefield/renderer.rs:52-120`
- **`check_death`** (`src/gameplay/combat/mod.rs:165-171`) generically despawns entities with `health.current <= 0.0` — no game-over detection exists
- **`Menu` state** (`src/menus/mod.rs:12-23`) provides overlays orthogonal to `GameState`. `Menu::Pause` is the reference pattern: overlay UI + frozen gameplay + input handling
- **All resources reset on `OnEnter(GameState::InGame)`**: Gold, Shop, EnemySpawnTimer, UnitAssets, GridIndex (repopulated), camera position. Entity cleanup via `DespawnOnExit(GameState::InGame)`
- **Resource reset is automatic** — all `OnEnter(GameState::InGame)` systems handle reset, so returning to main menu then starting a new game gives a clean slate

### Key Discoveries:
- `GameSet::Death` runs after `GameSet::Combat` — detection system can slot into Death before `check_death`
- Pause menu pattern (`src/menus/pause.rs:8-59`): `OnEnter(Menu::Pause)` spawns overlay, `Update` system handles input, `DespawnOnExit(Menu::Pause)` for cleanup
- Gameplay systems all use `.run_if(in_state(GameState::InGame).and(in_state(Menu::None)))` — setting Menu to Victory/Defeat automatically freezes everything
- Widget constructors (`src/theme/widget.rs`) provide `header()`, `label()`, `overlay()` — reuse for end screens

## Desired End State

After implementation:
1. Destroying the enemy fortress → `Menu::Victory` overlay with "VICTORY!" text
2. Losing the player fortress → `Menu::Defeat` overlay with "DEFEAT" text
3. Both overlays show battlefield behind semi-transparent background
4. Q key → return to main menu from either overlay
5. Full game loop: Main Menu → InGame → Victory/Defeat → Main Menu → new game
6. 90%+ test coverage on new code

### Verification:
- `make check` passes (clippy, formatting)
- `make test` passes with new tests covering detection, UI, and menu return
- Manual: play game, destroy enemy fortress → Victory screen → Q → main menu → Space → fresh game
- Manual: let enemies destroy player fortress → Defeat screen → Q → main menu

## What We're NOT Doing

- Score tracking or statistics
- Animations on victory/defeat (fade-in, particles)
- Sound effects
- Different difficulty levels
- Save/load game state
- Victory/defeat conditions beyond fortress HP (no timer, no unit count)

## Implementation Approach

The implementation follows the existing Menu overlay pattern exactly. Two new `Menu` variants (Victory, Defeat) freeze gameplay and show overlays. A detection system in `GameSet::Death` checks fortress health before `check_death` despawns them. From either overlay, Q returns to the main menu where the player can start a new game with Space.

## Phase 1: State Scaffolding & Module Creation

### Overview
Add new state variants, create module files, wire up plugins. No logic yet — just the skeleton.

### Changes Required:

#### 1. Add Menu variants
**File**: `src/menus/mod.rs`
**Changes**: Add `Victory` and `Defeat` variants to `Menu` enum

```rust
pub enum Menu {
    #[default]
    None,
    Main,
    Pause,
    /// Victory overlay (enemy fortress destroyed).
    Victory,
    /// Defeat overlay (player fortress destroyed).
    Defeat,
}
```

Also add `endgame` module declaration and plugin registration:

```rust
mod endgame;
mod main_menu;
mod pause;

// In plugin():
app.add_plugins((main_menu::plugin, pause::plugin, endgame::plugin));
```

#### 2. Create gameplay endgame detection module
**File**: `src/gameplay/endgame.rs` (NEW)
**Changes**: Fortress death detection system

```rust
//! Endgame detection: checks fortress health and triggers victory/defeat.

use bevy::prelude::*;

use crate::gameplay::battlefield::{EnemyFortress, PlayerFortress};
use crate::gameplay::units::Health;
use crate::menus::Menu;
use crate::screens::GameState;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        detect_endgame
            .in_set(crate::GameSet::Death)
            .before(crate::gameplay::combat::check_death)
            .run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
    );
}

/// Checks fortress health each frame. If either fortress is dead, transitions
/// to the appropriate Menu overlay (Victory or Defeat).
fn detect_endgame(
    player_fortress: Query<&Health, With<PlayerFortress>>,
    enemy_fortress: Query<&Health, With<EnemyFortress>>,
    mut next_menu: ResMut<NextState<Menu>>,
) {
    // Check defeat first (player fortress destroyed)
    if let Ok(health) = player_fortress.single() {
        if health.current <= 0.0 {
            next_menu.set(Menu::Defeat);
            return;
        }
    }

    // Check victory (enemy fortress destroyed)
    if let Ok(health) = enemy_fortress.single() {
        if health.current <= 0.0 {
            next_menu.set(Menu::Victory);
        }
    }
}
```

Register in `src/gameplay/mod.rs`:

```rust
pub(crate) mod endgame;

// In plugin():
app.add_plugins((
    battlefield::plugin,
    building::plugin,
    combat::plugin,
    economy::plugin,
    endgame::plugin,
    units::plugin,
));
```

#### 3. Create menu endgame overlay module
**File**: `src/menus/endgame.rs` (NEW)
**Changes**: Victory/Defeat overlay UI and input handling

```rust
//! Victory/Defeat overlay UI and input handling.

use bevy::prelude::*;

use super::Menu;
use crate::screens::GameState;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Victory), spawn_victory_screen);
    app.add_systems(OnEnter(Menu::Defeat), spawn_defeat_screen);
    app.add_systems(
        Update,
        handle_endgame_input.run_if(in_state(Menu::Victory).or(in_state(Menu::Defeat))),
    );
}

fn spawn_victory_screen(mut commands: Commands) {
    spawn_endgame_overlay(&mut commands, "VICTORY!", Menu::Victory);
}

fn spawn_defeat_screen(mut commands: Commands) {
    spawn_endgame_overlay(&mut commands, "DEFEAT", Menu::Defeat);
}

/// Shared overlay spawning for both victory and defeat screens.
/// Uses the same pattern as pause menu: overlay + header + prompt.
fn spawn_endgame_overlay(commands: &mut Commands, title: &str, menu: Menu) {
    // Semi-transparent overlay
    commands.spawn((crate::theme::widget::overlay(), DespawnOnExit(menu)));

    // Result text (64px header)
    commands.spawn((
        crate::theme::widget::header(title),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(40.0),
            ..default()
        },
        DespawnOnExit(menu),
    ));

    // Action prompt (24px)
    commands.spawn((
        Text::new("Press Q to Continue"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(crate::theme::palette::BODY_TEXT),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(55.0),
            ..default()
        },
        DespawnOnExit(menu),
    ));
}

fn handle_endgame_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::KeyQ) {
        next_game_state.set(GameState::MainMenu);
        // Menu::Main will be set by the MainMenu screen's OnEnter system.
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (new files compile, clippy clean)
- [x] `make test` passes (existing tests unaffected)

#### Manual Verification:
- [ ] Game still loads and plays normally (new states don't interfere)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for confirmation before proceeding.

---

## Phase 2: Tests

### Overview
Add comprehensive tests for all new systems: detection, overlay spawning, restart flow, and full game loop.

### Changes Required:

#### 1. Detection system tests
**File**: `src/gameplay/endgame.rs` (append tests module)

Tests to add:
- `detect_endgame_triggers_defeat_when_player_fortress_dead` — spawn player fortress with 0 HP, run system, verify Menu transitions to Defeat
- `detect_endgame_triggers_victory_when_enemy_fortress_dead` — spawn enemy fortress with 0 HP, verify Victory
- `detect_endgame_does_nothing_when_both_alive` — both fortresses with full HP, verify Menu stays None
- `detect_endgame_prioritizes_defeat_over_victory` — both at 0 HP, verify Defeat (player fortress checked first)

Test app pattern (no `InputPlugin` needed — detection doesn't read input):
```rust
fn create_detection_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<GameState>();
    app.init_state::<Menu>();
    // Must be in InGame + Menu::None for system to run
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InGame);
    app.world_mut()
        .resource_mut::<NextState<Menu>>()
        .set(Menu::None);
    app.add_systems(Update, detect_endgame.run_if(
        in_state(GameState::InGame).and(in_state(Menu::None))
    ));
    app.update(); // Apply state transitions
    app
}
```

#### 2. Overlay UI spawn tests
**File**: `src/menus/endgame.rs` (append tests module)

Tests:
- `victory_screen_spawns_overlay_and_text` — trigger OnEnter(Menu::Victory), verify 3 entities (overlay + header + prompt)
- `defeat_screen_spawns_overlay_and_text` — same for Defeat

#### 3. Integration tests for menu return
**File**: `src/gameplay/endgame.rs` (integration_tests module)

Tests:
- `return_to_menu_cleans_up_gameplay_entities` — play → defeat → Q to menu → verify no gameplay entities remain

#### 4. Update existing state tests
**File**: `src/lib.rs` (tests module)

Add new Menu variants to `menu_states_are_distinct`:
```rust
assert_ne!(Menu::Pause, Menu::Victory);
assert_ne!(Menu::Victory, Menu::Defeat);
```

(No changes needed to `GameState` tests — no new variants added.)

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes with all new tests green (151 total, +7 new)
- [x] No decrease in overall test coverage

#### Manual Verification:
- [ ] Play a full game loop: Main Menu → InGame → destroy enemy fortress → Victory → Q → Main Menu → Space → fresh game
- [ ] Verify battlefield is visible behind overlay
- [ ] Verify game is frozen when overlay is showing
- [ ] Verify starting a new game from main menu gives 200 gold, wave 1, clean battlefield

---

## Testing Strategy

### Unit Tests:
- Detection system: fortress alive (no trigger), player dead (defeat), enemy dead (victory), both dead (defeat priority)
- Overlay spawning: correct entity count, correct DespawnOnExit markers

### Integration Tests:
- Return to menu: InGame → Victory/Defeat → MainMenu with entity cleanup

### Manual Testing Steps:
1. Start game, build barracks, wait for units to reach enemy fortress
2. When enemy fortress destroyed → "VICTORY!" overlay appears
3. Press Q → returns to main menu
4. Press Space → new game starts with 200 gold, empty build zone, fresh waves
5. Let enemies overwhelm defenses → player fortress destroyed → "DEFEAT" overlay
6. Press Q → returns to main menu, can start again with Space
7. Verify ESC does NOT work during Victory/Defeat (only Pause uses ESC)

## Performance Considerations

- Detection system runs one query per fortress per frame (trivial cost)
- Only runs when `Menu::None` — no cost during overlay or main menu

## Verified API Patterns (Bevy 0.18)

These were verified against the actual crate source:

- `Query::single()` returns `Result` — use `if let Ok(health) = query.single()`
- `DespawnOnExit(Menu::Victory)` works because `Menu` derives `States` with `#[states(scoped_entities)]`
- `.run_if(in_state(X).or(in_state(Y)))` — valid combinator for matching multiple states
- `NextState::set()` during same frame: only the last `set` per state type takes effect
- System ordering: `.before(system_fn)` orders within the same `SystemSet`

## References

- Linear ticket: [GAM-9](https://linear.app/tayhu-home-lab/issue/GAM-9/victorydefeat-game-loop)
- Pause menu pattern: `src/menus/pause.rs` (reference implementation for overlays)
- Death system: `src/gameplay/combat/mod.rs:165-171`
- Fortress spawning: `src/gameplay/battlefield/renderer.rs:52-120`
- Resource reset systems: Gold (`economy/mod.rs:59`), Shop (`economy/shop.rs:101`), EnemySpawnTimer (`units/spawn.rs:77`)

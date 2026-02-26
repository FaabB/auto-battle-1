# Fix Application Hang on Exit (GAM-53) — Implementation Plan

## Overview

After playing a game and returning to the main menu, the application hangs when the user tries to exit. This plan fixes two bugs discovered during investigation: (1) `Time<Virtual>` remains permanently paused after returning from a game, and (2) no defensive cleanup of navmesh obstacle markers before `DespawnOnExit` fires.

## Current State Analysis

### The exit flow (after playing a game)

1. User clicks "Exit Game" in pause menu → `next_game.set(GameState::MainMenu)` (`menus/pause.rs:52-53`)
2. `Menu` state stays at `Menu::Pause` (NOT reset to `None`)
3. `OnExit(GameState::InGame)` fires:
   - `strip_buildings_before_despawn` removes `Building` markers
   - `DespawnOnExit(GameState::InGame)` despawns all game entities (units, buildings, fortresses, navmesh)
4. `OnEnter(GameState::MainMenu)` fires `open_main_menu` → sets `Menu::Main`
5. Menu transitions: `Menu::Pause` → `Menu::Main` (skips `Menu::None` entirely)
6. `unpause_virtual_time` only fires on `OnEnter(Menu::None)` — **never called**
7. `Time<Virtual>` stays paused for the rest of the session

### Same flow for endgame (Victory/Defeat)

Endgame "Exit to Menu" (`menus/endgame.rs:65`) has the identical pattern: only sets `GameState::MainMenu`, `Menu` goes from `Victory/Defeat` → `Main` (skipping `None`).

### Key Discoveries

- **`Time<Virtual>` paused bug** (`menus/mod.rs:37-38`): `pause_virtual_time` fires on `OnExit(Menu::None)`, `unpause_virtual_time` fires on `OnEnter(Menu::None)`. When `Menu` transitions `Pause→Main` or `Victory/Defeat→Main`, `None` is never entered, so virtual time stays paused. This affects all timer-based systems and physics (avian2d runs in `FixedPostUpdate` off `Time<Virtual>`).
- **Navmesh async tasks** (`vleue_navigator updater.rs:472-487`): `NavmeshUpdaterPlugin` spawns navmesh rebuild tasks on `AsyncComputeTaskPool` and `.detach()`s them. During app shutdown, Bevy waits for all task pool futures to complete. If a rebuild was in-flight when the game ended (or triggered by `DespawnOnExit` removing `NavObstacle` entities), the task pool shutdown could stall.
- **No `NavObstacle` pre-despawn cleanup**: Unlike `Building` (which has `strip_buildings_before_despawn`), `NavObstacle` markers on fortresses and buildings are not stripped before `DespawnOnExit` fires. This means `NavmeshUpdaterPlugin` sees `RemovedComponents<NavObstacle>` and may attempt a late rebuild of a navmesh entity that's about to be despawned.

## Desired End State

- `Time<Virtual>` is always unpaused when returning to the main menu, regardless of which `Menu` state was active during gameplay
- `NavObstacle` markers are stripped before `DespawnOnExit` fires, preventing late navmesh rebuild triggers
- Application exits cleanly after any game session (pause exit, victory, defeat)

### How to verify

- **Automated**: `make check && make test` — all tests pass, no compilation errors
- **Manual**: Start a game → pause → exit to menu → click "Exit Game" → app closes cleanly (no hang)
- **Manual**: Start a game → let it play to victory/defeat → exit to menu → click "Exit Game" → app closes cleanly
- **Manual**: Start a game → exit → start another game → exit → quit — still closes cleanly (no stale state)

## What We're NOT Doing

- Not changing the `Menu` state machine or transition logic (the `Pause→Main` skip is intentional and correct for UI purposes)
- Not modifying vleue_navigator or avian2d source code
- Not adding a custom shutdown system (the fix prevents the hang conditions from occurring in the first place)

## Implementation Approach

Two small, targeted fixes in a single phase:

1. **Unpause `Time<Virtual>` on `OnExit(GameState::InGame)`** — guarantees virtual time is unpaused regardless of the `Menu` transition path
2. **Strip `NavObstacle` markers on `OnExit(GameState::InGame)`** — prevents `NavmeshUpdaterPlugin` from detecting obstacle removals during `DespawnOnExit` and triggering a late rebuild

Both systems follow the existing `strip_buildings_before_despawn` pattern.

## Phase 1: Fix Hang on Exit

### Changes Required

#### 1. Unpause virtual time on game exit
**File**: `src/menus/mod.rs`
**Changes**: Add an `OnExit(GameState::InGame)` system that unpauses `Time<Virtual>`.

```rust
// In plugin():
app.add_systems(OnExit(GameState::InGame), unpause_virtual_time_on_game_exit);

// New system:
fn unpause_virtual_time_on_game_exit(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}
```

This import is needed at the top of the file:
```rust
use crate::screens::GameState;
```

**Rationale**: This is the simplest, most robust fix. Rather than changing how `Menu` transitions work (which could break other things), we ensure virtual time is always unpaused when leaving the game state. The existing `OnExit(Menu::None)/OnEnter(Menu::None)` systems continue to handle pause/unpause during gameplay (when the pause menu opens/closes).

#### 2. Strip NavObstacle markers before despawn
**File**: `src/third_party/vleue_navigator.rs`
**Changes**: Add a system on `OnExit(GameState::InGame)` that removes `NavObstacle` from all entities before `DespawnOnExit` fires.

```rust
use crate::screens::GameState;

// In plugin():
app.add_systems(OnExit(GameState::InGame), strip_nav_obstacles_before_despawn);

// New system:
fn strip_nav_obstacles_before_despawn(
    mut commands: Commands,
    obstacles: Query<Entity, With<NavObstacle>>,
) {
    for entity in &obstacles {
        commands.entity(entity).remove::<NavObstacle>();
    }
}
```

**Rationale**: Same pattern as `strip_buildings_before_despawn` in `building/mod.rs`. By removing `NavObstacle` markers explicitly (via commands that apply during the state transition), the `NavmeshUpdaterPlugin` doesn't see them as "removed" from despawned entities and won't trigger a rebuild. This prevents any late async navmesh build tasks from being spawned during the exit-from-game sequence.

#### 3. Tests
**File**: `src/menus/mod.rs` (existing test module)
**Changes**: Add tests verifying `Time<Virtual>` is unpaused after transitioning from InGame → MainMenu.

```rust
#[test]
fn virtual_time_unpaused_after_ingame_to_main_menu() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::state::app::StatesPlugin);
    app.init_state::<GameState>();
    app.init_state::<Menu>();
    app.add_systems(OnExit(Menu::None), pause_virtual_time);
    app.add_systems(OnEnter(Menu::None), unpause_virtual_time);
    app.add_systems(OnExit(GameState::InGame), unpause_virtual_time_on_game_exit);
    app.add_systems(OnEnter(GameState::MainMenu), |mut next_menu: ResMut<NextState<Menu>>| {
        next_menu.set(Menu::Main);
    });
    app.update();

    // Enter InGame
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::InGame);
    app.update();

    // Open pause menu (Time<Virtual> gets paused via OnExit(Menu::None))
    app.world_mut()
        .resource_mut::<NextState<Menu>>()
        .set(Menu::Pause);
    app.update();

    let time = app.world().resource::<Time<Virtual>>();
    assert!(time.is_paused(), "Should be paused when menu is open");

    // Exit game: GameState → MainMenu (Menu goes Pause → Main, skipping None)
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::MainMenu);
    app.update();

    let time = app.world().resource::<Time<Virtual>>();
    assert!(
        !time.is_paused(),
        "Time<Virtual> should be unpaused after exiting InGame"
    );
}
```

Also add a test for the endgame path (Victory/Defeat → MainMenu):

```rust
#[test]
fn virtual_time_unpaused_after_endgame_to_main_menu() {
    // Same setup as above but with Menu::Victory instead of Menu::Pause
    // Verifies the fix works for all exit paths
}
```

### Success Criteria

#### Automated Verification:
- [x] `make check` passes (no compilation errors, no clippy warnings)
- [x] `make test` passes (all existing + new tests)
- [x] New tests verify `Time<Virtual>` is unpaused after InGame → MainMenu via pause
- [x] New tests verify `Time<Virtual>` is unpaused after InGame → MainMenu via endgame

#### Manual Verification:
- [x] Start game → pause → "Exit Game" → on main menu → "Exit Game" → app closes cleanly (no hang)
- [x] Start game → play to victory → "Exit to Menu" → "Exit Game" → app closes cleanly
- [x] Start game → play to defeat → "Exit to Menu" → "Exit Game" → app closes cleanly
- [x] Start game → exit → start another game → play normally (timers work, physics works, units move) — confirms virtual time was properly unpaused
- [x] Start game → exit → start another game → exit → quit — no stale state issues

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation that the hang is resolved.

## Testing Strategy

### Unit Tests:
- `virtual_time_unpaused_after_ingame_to_main_menu` — pause menu exit path
- `virtual_time_unpaused_after_endgame_to_main_menu` — endgame overlay exit path

### Integration Tests:
- Existing `virtual_time_paused_on_menu_exit_none` and `virtual_time_unpaused_on_menu_enter_none` continue to pass (pause/unpause during gameplay unaffected)

### Manual Testing Steps:
1. `cargo run` → Start Battle → press ESC → "Exit Game" → "Exit Game" → verify clean exit
2. `cargo run` → Start Battle → let enemy fortress die → "Exit to Menu" → "Exit Game" → verify clean exit
3. `cargo run` → Start Battle → let player fortress die → "Exit to Menu" → "Exit Game" → verify clean exit
4. `cargo run` → Start Battle → exit → Start Battle → verify gameplay works normally (no paused timers)

## Performance Considerations

None. Both systems are trivial O(n) iterations that run once during a state transition, not per-frame.

## References

- Linear ticket: [GAM-53](https://linear.app/tayhu-games/issue/GAM-53/application-hangs-on-exit-after-leaving-a-game)
- Existing pre-despawn pattern: `src/gameplay/building/mod.rs:181-191` (`strip_buildings_before_despawn`)
- Virtual time pause/unpause: `src/menus/mod.rs:37-47`
- vleue_navigator async task detach: `~/.cargo/registry/src/.../vleue_navigator-0.15.0/src/updater.rs:472-487`

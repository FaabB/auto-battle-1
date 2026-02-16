# Pause Physics When Game Is Paused (GAM-27) — Implementation Plan

## Overview

Units keep drifting when the game is paused because avian2d's physics solver continues integrating velocities. The fix is to pause `Time<Virtual>` when any menu overlay opens, which stops all time-dependent systems including physics (`Time<Fixed>` accumulates from `Time<Virtual>`, so `FixedPostUpdate` starves and avian2d stops stepping).

## Current State Analysis

- **Gameplay systems** use `.run_if(gameplay_running)` to stop during pause — this gates non-time-based systems (input, AI, building placement, death, etc.)
- **avian2d physics** runs in `FixedPostUpdate` independently — not controlled by `gameplay_running`
- **`unit_movement`** sets `LinearVelocity` each frame; when it stops running (pause), the *last-set* velocity persists and physics keeps integrating it

### Key Discoveries:
- `Time<Virtual>` has `pause()` / `unpause()` — pausing preserves `relative_speed`, so future speed control (2x button) works for free
- avian2d's `Time<Physics>` follows `Time<Fixed>`, which accumulates from `Time<Virtual>` — so pausing `Time<Virtual>` stops physics without touching avian2d directly
- `Menu` state transitions cover all pause/unpause events: `OnExit(Menu::None)` = something opened, `OnEnter(Menu::None)` = everything closed

## Desired End State

- Units freeze in place when any menu is open (Pause, Victory, Defeat)
- Physics resumes seamlessly when returning to gameplay
- All timers (production, attack, income, waves) also stop during pause (belt-and-suspenders with existing `run_if`)
- `run_if(gameplay_running)` remains for non-time-based system gating
- Future speed control is possible via `Time<Virtual>::set_relative_speed()`

### Verification:
1. `make check && make test` passes
2. Run game → spawn units → press ESC → units freeze → press ESC → units resume

## What We're NOT Doing

- Removing `run_if(gameplay_running)` — still needed for non-time-based systems (input, AI, death)
- Pausing `Time<Physics>` separately — unnecessary since `Time<Virtual>` pause starves `Time<Fixed>`
- Implementing speed-up button — future ticket, but this plan enables it
- Zeroing velocities on pause — not needed with proper time pausing

## Implementation Approach

Add two tiny systems to the menu plugin (`src/menus/mod.rs`) that pause/unpause `Time<Virtual>` on `Menu` state transitions:
- `OnExit(Menu::None)` → `time.pause()` (any menu opened)
- `OnEnter(Menu::None)` → `time.unpause()` (all menus closed)

## Phase 1: Pause Virtual Time on Menu Transitions

### Changes Required:

#### 1. Menu Plugin — Add Time Pause Systems
**File**: `src/menus/mod.rs`
**Changes**: Add `pause_virtual_time` and `unpause_virtual_time` systems

```rust
fn pause_virtual_time(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn unpause_virtual_time(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}
```

Register in `plugin()`:
```rust
app.add_systems(OnExit(Menu::None), pause_virtual_time);
app.add_systems(OnEnter(Menu::None), unpause_virtual_time);
```

**No other files change.** The existing `run_if(gameplay_running)` and physics setup remain untouched.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (clippy + compile)
- [x] `make test` passes (all existing tests still work)
- [x] New unit tests pass:
  - `virtual_time_paused_on_menu_exit_none` — transition from `Menu::None` → `Menu::Pause` pauses `Time<Virtual>`
  - `virtual_time_unpaused_on_menu_enter_none` — transition from `Menu::Pause` → `Menu::None` unpauses `Time<Virtual>`

#### Manual Verification:
- [x] Run game → build barracks → wait for units → press ESC → units freeze completely (no drift)
- [x] Press ESC again → units resume moving normally
- [x] Let game reach victory/defeat → units freeze when overlay appears

---

## Testing Strategy

### Unit Tests (in `src/menus/mod.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn create_menu_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.init_state::<Menu>();
        app.add_systems(OnExit(Menu::None), pause_virtual_time);
        app.add_systems(OnEnter(Menu::None), unpause_virtual_time);
        app.update(); // Initialize
        app
    }

    #[test]
    fn virtual_time_paused_on_menu_exit_none() {
        let mut app = create_menu_test_app();
        // Transition to Pause
        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::Pause);
        app.update();

        let time = app.world().resource::<Time<Virtual>>();
        assert!(time.is_paused(), "Time<Virtual> should be paused when menu is open");
    }

    #[test]
    fn virtual_time_unpaused_on_menu_enter_none() {
        let mut app = create_menu_test_app();
        // Transition to Pause
        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::Pause);
        app.update();

        // Transition back to None
        app.world_mut()
            .resource_mut::<NextState<Menu>>()
            .set(Menu::None);
        app.update();

        let time = app.world().resource::<Time<Virtual>>();
        assert!(!time.is_paused(), "Time<Virtual> should be unpaused when menu closes");
    }
}
```

### Edge Cases Covered:
- **Quit from pause** (Pause → MainMenu): `OnExit(Menu::None)` already fired on pause entry. Time stays paused. When new game starts and `Menu` → `None`, `OnEnter(Menu::None)` fires → unpause. ✓
- **Victory/Defeat**: Same flow — `OnExit(Menu::None)` pauses. New game start unpauses. ✓
- **Game startup**: App starts with `Menu::None` (default). No pause/unpause event. Time runs. ✓

## Verified API Patterns (Bevy 0.18 + avian2d 0.5)

- `Time<Virtual>` has `pause()`, `unpause()`, `is_paused()` — standard Bevy time API
- `pause()` preserves `relative_speed` — `unpause()` resumes at the same speed
- `Time<Fixed>` accumulates from `Time<Virtual>` — pausing Virtual starves Fixed → FixedPostUpdate won't fire
- avian2d's `PhysicsSchedule` runs inside `FixedPostUpdate` — it stops when Virtual is paused

## References

- Linear ticket: [GAM-27](https://linear.app/tayhu-games/issue/GAM-27/units-keep-moving-when-game-is-paused-physics-not-paused)
- avian2d time API: `~/.cargo/registry/src/.../avian2d-0.5.0/src/schedule/time.rs`
- Menu state: `src/menus/mod.rs`
- Gameplay running condition: `src/lib.rs:61-66`

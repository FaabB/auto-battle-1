# Camera & Battlefield Layout — Implementation Plan

## Overview

Implement the foundational battlefield coordinate system with zone layout, fortress placeholders, and horizontal camera panning. This is Ticket 1, the foundation all subsequent tickets build on.

**Includes state architecture refactor**: Replace manual `CleanupXxx` markers with Bevy's built-in `DespawnOnExit`, and convert `Paused` from a top-level state to a `SubState` of `InGame`. This fixes the known bug where pausing destroys InGame entities, and eliminates custom cleanup boilerplate.

## Verified API Patterns (Bevy 0.18)

Verified against actual crate source in `~/.cargo/registry/src/`:

- **`Projection` enum is the Component** (not `OrthographicProjection`). Access via pattern match:
  ```rust
  if let Projection::Orthographic(ref mut ortho) = *projection { ... }
  ```
- **`ScalingMode`** lives at `bevy::camera::ScalingMode` (NOT `bevy::render::camera`). Not in prelude — must import explicitly.
- **`Camera2d`** auto-adds `Projection` via `#[require]`. Spawning `Camera2d` gives you a default orthographic projection for 2D.
- **`Single<D, F>`** is a system parameter for exactly-one-entity queries. Skips system if 0 or >1 match. Use instead of `Query` + `.single_mut()`.
- **`Sprite::from_color(color, size)`** — convenience constructor, sets `color` and `custom_size`.
- **`Sprite` has `#[require(Transform, Visibility, ...)]`** — auto-adds `Transform`. Spawn explicit `Transform::from_xyz(...)` to override position.
- **`despawn()`** is already recursive in Bevy 0.18 — handles children automatically. No `despawn_recursive()` needed.
- **`ButtonInput<KeyCode>`** — correct type (not `Input<KeyCode>`).
- **`time.delta_secs()`** — correct method name.
- **`DespawnOnExit<S: States>`** — built-in component. Entities with `DespawnOnExit(GameState::InGame)` auto-despawn when leaving that state. In Bevy's prelude. Enabled automatically by `init_state` / `add_sub_state`.
- **`SubStates` derive** — `#[derive(SubStates)]` with `#[source(ParentState = ParentState::Variant)]`. Only exists while parent is in specified variant. Use `add_sub_state::<T>()` to register.

## Current State Analysis

- Camera: bare `Camera2d` spawned at startup (`src/main.rs:38`), no transform, no panning
- InGame screen: spawns placeholder text only (`src/screens/in_game.rs:23-39`)
- No coordinate system, no sprites, no world-space entities exist
- Window: 1920x1080, `ImagePlugin::default_nearest()` for pixel-perfect rendering
- Bevy 0.18 with `"2d"` feature only
- Clippy: all + pedantic + nursery lints as warnings, `-D warnings` in CI. Three Bevy-specific allows: `needless_pass_by_value`, `too_many_arguments`, `type_complexity`.

### Key Discoveries:
- Manual `CleanupXxx` markers + `cleanup_entities::<T>` system reinvents Bevy's built-in `DespawnOnExit` — should be replaced
- `Paused` is a top-level `GameState` variant that triggers `OnExit(GameState::InGame)`, destroying all InGame entities — must become a SubState
- `screens/` directory is fine for Loading/MainMenu (they're pure UI screens), but `InGamePlugin` is becoming vestigial (just an ESC handler) while real game logic lives in separate plugins like `BattlefieldPlugin`

## Desired End State

After this plan is complete:

- **State architecture** uses `GameState` (Loading, MainMenu, InGame) + `InGameState` SubState (Playing, Paused)
- **`DespawnOnExit`** replaces all manual `CleanupXxx` markers — no custom cleanup system
- Pausing no longer destroys InGame entities
- A `battlefield` module exists with coordinate constants for all zones
- Fortress entities have marker components (`PlayerFortress`, `EnemyFortress`) for future ticket extensibility
- Entering InGame spawns the full battlefield layout as colored `Sprite` entities:
  - Blue fortress (2 cols, far left)
  - Building zone background (6 cols, distinct color)
  - Combat zone (72 cols, open area)
  - Red fortress (2 cols, far right)
- Camera starts centered on the player's building zone
- A/D or Left/Right arrow keys pan the camera horizontally
- Camera clamps at battlefield boundaries (can't pan into void)
- All battlefield entities use `DespawnOnExit(GameState::InGame)` so they despawn on state exit

### Verification:
- `make check` passes (clippy + tests)
- Running the game and pressing SPACE shows the battlefield layout
- Panning with A/D or arrow keys works and stops at boundaries
- Both fortress rectangles visible at their respective ends
- ESC pauses (overlay appears), ESC resumes — **battlefield entities persist through pause**
- Q from pause → MainMenu — battlefield entities cleaned up

## What We're NOT Doing

- Grid lines / cell outlines (Ticket 2)
- Building placement (Ticket 2)
- Zoom controls
- Mouse-based panning
- Any gameplay systems
- Health/HP on fortresses (Ticket 8)
- Economy or unit systems

## Forward-Looking Architecture

Future tickets will extend entities created here. This plan designs for that:

| Ticket | What it adds to Ticket 1 entities |
|--------|-----------------------------------|
| 2 (Grid/Building) | Queries building zone for placement boundaries; game systems use `run_if(in_state(InGameState::Playing))` |
| 3 (Unit Spawning) | Uses battlefield constants for spawn positions |
| 4 (Movement/AI) | Uses fortress positions as movement targets; pauses when `InGameState::Paused` |
| 5 (Combat) | Units attack fortresses; needs fortress entity queries |
| 8 (Fortress HP) | Adds `Health` component to `PlayerFortress`/`EnemyFortress` entities |
| 9 (Victory/Defeat) | Queries fortress `Health` to detect win/loss |

**Key design decisions**:
- Fortress entities get marker components now (`PlayerFortress`, `EnemyFortress`) so Ticket 8 can simply add `Health` to them.
- `InGameState::Playing` vs `InGameState::Paused` SubState means future game logic systems use `run_if(in_state(InGameState::Playing))` to automatically pause.

## Implementation Approach

Use Bevy `Sprite` entities with `Transform` positions in world space. The battlefield origin (0,0) will be at the **bottom-left corner** of the full battlefield. Each zone is positioned using the constants. The camera is an orthographic 2D camera that only moves along the X axis.

### Coordinate System

```
Y
↑  Row 9 (top)
│  ...
│  Row 0 (bottom)
└──────────────────────────────────────────────→ X
   Fort(0-1)  Build(2-7)  Combat(8-79)  Fort(80-81)
```

- Cell (col, row) → world position: (col * 64 + 32, row * 64 + 32)
  - The +32 centers the sprite in the cell
- Battlefield pixel dimensions: 82 cols × 64 = 5248px wide, 10 rows × 64 = 640px tall

### Module Structure

Single file `src/battlefield.rs` for now. This will grow to ~150-200 lines for Ticket 1 which is fine for a single file. When Ticket 2 adds grid/building placement (~200+ more lines), we can split into `src/battlefield/mod.rs` + submodules at that point. No premature directory structure.

### State Architecture

```
GameState (top-level States)
├── Loading     → DespawnOnExit(GameState::Loading)
├── MainMenu    → DespawnOnExit(GameState::MainMenu)
└── InGame      → DespawnOnExit(GameState::InGame)
     └── InGameState (SubState, only exists while InGame)
         ├── Playing  (default)
         └── Paused   → DespawnOnExit(InGameState::Paused)
```

---

## Phase 0: State Architecture Refactor

### Overview
Replace the manual cleanup system with Bevy's built-in `DespawnOnExit`. Convert `Paused` from a top-level `GameState` variant to an `InGameState` SubState. This fixes the pause-destroys-entities bug and eliminates boilerplate.

### Changes Required:

#### 1. Update `GameState` in `src/lib.rs`

Remove `Paused` variant. Add `InGameState` SubState definition.

```rust
//! Auto-battle game library.

pub mod game;
pub mod prelude;
pub mod screens;
#[cfg(test)]
pub mod testing;

use bevy::prelude::*;

/// Primary game states.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GameState {
    /// Initial loading state.
    #[default]
    Loading,
    /// Main menu state.
    MainMenu,
    /// Active gameplay state.
    InGame,
}

/// Sub-states within InGame. Only exists while `GameState::InGame` is active.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(GameState = GameState::InGame)]
pub enum InGameState {
    /// Normal gameplay.
    #[default]
    Playing,
    /// Game is paused (overlay on gameplay).
    Paused,
}
```

#### 2. Update `src/game/mod.rs`

Register the sub-state. Move camera spawn here from `main.rs` (GamePlugin owns core setup).

```rust
use bevy::prelude::*;

/// Core game plugin that sets up states and the global camera.
#[derive(Debug)]
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<crate::GameState>()
            .add_sub_state::<crate::InGameState>()
            .add_systems(Startup, setup_camera);
    }
}

/// Spawns the global 2D camera. Persists across all states (do NOT add DespawnOnExit).
fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

#### 3. Delete `src/components/` directory entirely

Delete `cleanup.rs` (all 4 `CleanupXxx` markers) AND `mod.rs`. The module declaration was already removed from `lib.rs` in step 1. Future tickets will re-create this module when components are needed (Ticket 2 adds Building components).

#### 4. Delete `src/systems/` directory entirely

Delete `cleanup.rs` (`cleanup_entities` system) AND `mod.rs`. The module declaration was already removed from `lib.rs` in step 1. Re-create if cross-cutting systems are needed in future tickets.

#### 4b. Delete `src/resources/` and `src/ui/` directories

Both are empty placeholder modules with only doc comments. Remove them now; re-add when a ticket actually needs them (Ticket 6 for resources, Ticket 2 for UI).

#### 5. Update `src/screens/loading.rs`

Replace `CleanupLoading` with `DespawnOnExit(GameState::Loading)`. Remove `OnExit` cleanup system registration.

```rust
//! Loading screen plugin.

use bevy::prelude::*;

use crate::GameState;

#[derive(Debug)]
pub struct LoadingScreenPlugin;

impl Plugin for LoadingScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::Loading), setup_loading_screen)
            .add_systems(
                Update,
                check_loading_complete.run_if(in_state(GameState::Loading)),
            );
    }
}

fn setup_loading_screen(mut commands: Commands) {
    commands.spawn((
        Text::new("Loading..."),
        TextFont {
            font_size: 48.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            ..default()
        },
        DespawnOnExit(GameState::Loading),
    ));
}

fn check_loading_complete(mut next_state: ResMut<NextState<GameState>>) {
    next_state.set(GameState::MainMenu);
}
```

#### 6. Update `src/screens/main_menu.rs`

Replace `CleanupMainMenu` with `DespawnOnExit(GameState::MainMenu)`. Remove `OnExit` cleanup system.

```rust
//! Main menu plugin.

use bevy::prelude::*;

use crate::GameState;

#[derive(Debug)]
pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
            .add_systems(
                Update,
                handle_main_menu_input.run_if(in_state(GameState::MainMenu)),
            );
    }
}

fn setup_main_menu(mut commands: Commands) {
    // Title
    commands.spawn((
        Text::new("Auto Battle"),
        TextFont {
            font_size: 72.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(30.0),
            ..default()
        },
        DespawnOnExit(GameState::MainMenu),
    ));

    // Start prompt
    commands.spawn((
        Text::new("Press SPACE to Start"),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(60.0),
            ..default()
        },
        DespawnOnExit(GameState::MainMenu),
    ));
}

fn handle_main_menu_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if keyboard.just_pressed(KeyCode::Space) {
        next_state.set(GameState::InGame);
    }
}
```

#### 7. Update `src/screens/in_game.rs`

Remove setup_game (placeholder text). Update ESC handler to toggle `InGameState` instead of `GameState`. Remove `OnExit` cleanup.

```rust
//! In-game state transitions (pause, unpause, quit to menu).
//!
//! Gameplay visuals and logic are handled by domain plugins
//! (e.g., `BattlefieldPlugin`). This plugin only owns keybindings
//! that operate across all `InGameState` sub-states.

use bevy::prelude::*;

use crate::{GameState, InGameState};

#[derive(Debug)]
pub struct InGamePlugin;

impl Plugin for InGamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            handle_game_input.run_if(in_state(GameState::InGame)),
        );
    }
}

fn handle_game_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    current_state: Res<State<InGameState>>,
    mut next_ingame_state: ResMut<NextState<InGameState>>,
    mut next_game_state: ResMut<NextState<GameState>>,
) {
    match current_state.get() {
        InGameState::Playing => {
            if keyboard.just_pressed(KeyCode::Escape) {
                next_ingame_state.set(InGameState::Paused);
            }
        }
        InGameState::Paused => {
            if keyboard.just_pressed(KeyCode::Escape) {
                next_ingame_state.set(InGameState::Playing);
            }
            if keyboard.just_pressed(KeyCode::KeyQ) {
                next_game_state.set(GameState::MainMenu);
            }
        }
    }
}
```

**Note**: This merges the ESC/Q handling from both `in_game.rs` and `paused.rs` into one system, since both respond to input during `InGame`.

#### 8. Update `src/screens/paused.rs`

Replace `CleanupPaused` with `DespawnOnExit(InGameState::Paused)`. Change state triggers from `GameState::Paused` to `InGameState::Paused`. Remove `OnExit` cleanup.

```rust
//! Pause menu plugin.

use bevy::prelude::*;

use crate::InGameState;

#[derive(Debug)]
pub struct PausedPlugin;

impl Plugin for PausedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(InGameState::Paused), setup_pause_menu);
    }
}

fn setup_pause_menu(mut commands: Commands) {
    // Semi-transparent overlay
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        DespawnOnExit(InGameState::Paused),
    ));

    // Pause text
    commands.spawn((
        Text::new("PAUSED"),
        TextFont {
            font_size: 64.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(40.0),
            ..default()
        },
        DespawnOnExit(InGameState::Paused),
    ));

    // Resume prompt
    commands.spawn((
        Text::new("Press ESC to Resume | Q to Quit"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
        TextColor(Color::srgb(0.7, 0.7, 0.7)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(55.0),
            ..default()
        },
        DespawnOnExit(InGameState::Paused),
    ));
}
```

**Note**: `handle_pause_input` is removed — input handling is consolidated in `in_game.rs`.

#### 9. Update `src/screens/mod.rs`

Remove the `CleanupXxx` re-exports if any exist. The module structure stays the same.

#### 10. Update `src/prelude.rs`

Remove `pub use crate::components::*;` (the module no longer exists). Remove the commented-out `resources` line. Add `InGameState` export. Keep the prelude minimal — only truly universal types belong here.

```rust
//! Common imports for the entire crate.

pub use bevy::prelude::*;

pub use crate::GameState;
pub use crate::InGameState;
```

**Convention**: As components grow in future tickets, prefer explicit imports from domain modules (e.g., `use crate::battlefield::PlayerFortress`) rather than glob re-exports through the prelude. Only add types to the prelude if they're used in 4+ modules.

#### 11. Update `src/main.rs`

Remove `setup_camera` function and its `.add_systems(Startup, setup_camera)` registration — camera setup moved to `GamePlugin` (step 2). Keep `PausedPlugin` — it still spawns pause UI via `OnEnter`.

The resulting `main.rs` should be pure plugin wiring with no game logic:

```rust
//! Auto-battle game entry point.

use auto_battle::game::GamePlugin;
use auto_battle::prelude::*;
use auto_battle::screens::{InGamePlugin, LoadingScreenPlugin, MainMenuPlugin, PausedPlugin};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Auto Battle".to_string(),
                        resolution: (1920, 1080).into(),
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins((
            GamePlugin,
            LoadingScreenPlugin,
            MainMenuPlugin,
            InGamePlugin,
            PausedPlugin,
        ))
        .run();
}
```

#### 12. Update tests in `src/lib.rs`

Update the `game_states_are_distinct` test — remove `Paused` variant, add `InGameState` tests.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn game_state_default_is_loading() {
        assert_eq!(GameState::default(), GameState::Loading);
    }

    #[test]
    fn game_states_are_distinct() {
        assert_ne!(GameState::Loading, GameState::MainMenu);
        assert_ne!(GameState::MainMenu, GameState::InGame);
    }

    #[test]
    fn in_game_state_default_is_playing() {
        assert_eq!(InGameState::default(), InGameState::Playing);
    }

    #[test]
    fn in_game_states_are_distinct() {
        assert_ne!(InGameState::Playing, InGameState::Paused);
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` compiles successfully
- [ ] `make check` passes (clippy + tests)
- [ ] No references to `CleanupLoading`, `CleanupMainMenu`, `CleanupInGame`, `CleanupPaused` remain
- [ ] No references to `GameState::Paused` remain
- [ ] `cleanup_entities` system no longer exists
- [ ] `src/components/`, `src/systems/`, `src/resources/`, `src/ui/` directories deleted
- [ ] `setup_camera` no longer in `main.rs` (moved to `GamePlugin`)
- [ ] No `pub mod components;`, `pub mod systems;`, `pub mod resources;`, `pub mod ui;` in `lib.rs`

#### Manual Verification:
- [ ] Press SPACE to start → game enters InGame
- [ ] Press ESC → pause overlay appears, game still shows behind it
- [ ] Press ESC again → pause overlay disappears, game continues
- [ ] Press Q while paused → returns to main menu
- [ ] Return to main menu → all InGame entities cleaned up

**Implementation Note**: This phase must complete and pass before proceeding. It touches many files but each change is mechanical (replacing markers, updating state references). Build after EACH file change — don't batch.

---

## Phase 1: Battlefield Constants, Markers & Module Setup

### Overview
Create a `battlefield` module with all coordinate constants, zone definitions, marker components, and a `BattlefieldPlugin`. Register it from `main.rs`.

### Changes Required:

#### 1. New file: `src/battlefield.rs`

Define all battlefield constants, marker components, helper functions, and a plugin stub.

```rust
//! Battlefield layout constants, markers, and systems.

use bevy::prelude::*;

use crate::GameState;

// === Grid Constants ===

/// Size of a single grid cell in pixels.
pub const CELL_SIZE: f32 = 64.0;

/// Number of rows in the battlefield.
pub const BATTLEFIELD_ROWS: u32 = 10;

/// Number of columns for each fortress.
pub const FORTRESS_COLS: u32 = 2;

/// Number of columns in the building zone.
pub const BUILD_ZONE_COLS: u32 = 6;

/// Number of columns in the combat zone.
pub const COMBAT_ZONE_COLS: u32 = 72;

/// Total columns across the entire battlefield.
pub const TOTAL_COLS: u32 = FORTRESS_COLS + BUILD_ZONE_COLS + COMBAT_ZONE_COLS + FORTRESS_COLS;
// = 2 + 6 + 72 + 2 = 82

/// Total battlefield width in pixels.
pub const BATTLEFIELD_WIDTH: f32 = TOTAL_COLS as f32 * CELL_SIZE;
// = 82 * 64 = 5248.0

/// Total battlefield height in pixels.
pub const BATTLEFIELD_HEIGHT: f32 = BATTLEFIELD_ROWS as f32 * CELL_SIZE;
// = 10 * 64 = 640.0

// === Zone Column Ranges (start column, inclusive) ===

/// Player fortress starts at column 0.
pub const PLAYER_FORT_START_COL: u32 = 0;

/// Building zone starts after player fortress.
pub const BUILD_ZONE_START_COL: u32 = FORTRESS_COLS;
// = 2

/// Combat zone starts after building zone.
pub const COMBAT_ZONE_START_COL: u32 = FORTRESS_COLS + BUILD_ZONE_COLS;
// = 8

/// Enemy fortress starts after combat zone.
pub const ENEMY_FORT_START_COL: u32 = FORTRESS_COLS + BUILD_ZONE_COLS + COMBAT_ZONE_COLS;
// = 80

// === Marker Components ===

/// Marks the player's fortress entity. Ticket 8 adds `Health` to this.
#[derive(Component)]
pub struct PlayerFortress;

/// Marks the enemy's fortress entity. Ticket 8 adds `Health` to this.
#[derive(Component)]
pub struct EnemyFortress;

// === Helper Functions ===

/// Convert a grid column to a world X position (center of the column).
pub fn col_to_world_x(col: u32) -> f32 {
    col as f32 * CELL_SIZE + CELL_SIZE / 2.0
}

/// Convert a grid row to a world Y position (center of the row).
pub fn row_to_world_y(row: u32) -> f32 {
    row as f32 * CELL_SIZE + CELL_SIZE / 2.0
}

/// Get the world-space center X of a zone given its start column and width in columns.
fn zone_center_x(start_col: u32, width_cols: u32) -> f32 {
    start_col as f32 * CELL_SIZE + (width_cols as f32 * CELL_SIZE) / 2.0
}

/// Center Y of the battlefield.
fn battlefield_center_y() -> f32 {
    BATTLEFIELD_HEIGHT / 2.0
}

// === Plugin ===

#[derive(Debug)]
pub struct BattlefieldPlugin;

impl Plugin for BattlefieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(GameState::InGame), spawn_battlefield)
            .add_systems(
                Update,
                camera_pan.run_if(in_state(GameState::InGame)),
            );
    }
}

// Stub systems (implemented in Phase 2 & 3)
fn spawn_battlefield() {}
fn camera_pan() {}
```

#### 2. Register module in `src/lib.rs`

Add `pub mod battlefield;` to the module list.

#### 3. Register `BattlefieldPlugin` in `src/main.rs`

Add `BattlefieldPlugin` to the game plugins tuple:

```rust
use auto_battle::battlefield::BattlefieldPlugin;

.add_plugins((
    GamePlugin,
    LoadingScreenPlugin,
    MainMenuPlugin,
    InGamePlugin,
    PausedPlugin,
    BattlefieldPlugin,
))
```

### Success Criteria:

#### Automated Verification:
- [ ] `cargo build` compiles successfully
- [ ] `make check` passes (clippy + tests)
- [ ] All constants are defined and consistent (TOTAL_COLS = 82, etc.)

#### Manual Verification:
- [ ] N/A for this phase (no visual output yet)

---

## Phase 2: Spawn Battlefield Entities

### Overview
Implement `spawn_battlefield` to create colored sprite rectangles for each zone. Fortress entities get marker components. All entities use `DespawnOnExit(GameState::InGame)`.

### Changes Required:

#### 1. `spawn_battlefield` system in `src/battlefield.rs`

Replace the stub with the real implementation. Use `Sprite::from_color()` for cleaner code.

```rust
/// Colors for battlefield zones.
const PLAYER_FORT_COLOR: Color = Color::srgb(0.2, 0.3, 0.8);   // Blue
const ENEMY_FORT_COLOR: Color = Color::srgb(0.8, 0.2, 0.2);    // Red
const BUILD_ZONE_COLOR: Color = Color::srgb(0.25, 0.25, 0.35);  // Dark blue-gray
const COMBAT_ZONE_COLOR: Color = Color::srgb(0.15, 0.15, 0.2);  // Dark gray
const BACKGROUND_COLOR: Color = Color::srgb(0.1, 0.1, 0.12);    // Near-black

fn spawn_battlefield(mut commands: Commands) {
    // Background (slightly larger than battlefield for visual framing)
    commands.spawn((
        Sprite::from_color(
            BACKGROUND_COLOR,
            Vec2::new(BATTLEFIELD_WIDTH + 128.0, BATTLEFIELD_HEIGHT + 128.0),
        ),
        Transform::from_xyz(BATTLEFIELD_WIDTH / 2.0, battlefield_center_y(), -1.0),
        DespawnOnExit(GameState::InGame),
    ));

    // Player fortress (blue) — has marker for Ticket 8 (fortress HP)
    commands.spawn((
        Sprite::from_color(
            PLAYER_FORT_COLOR,
            Vec2::new(FORTRESS_COLS as f32 * CELL_SIZE, BATTLEFIELD_HEIGHT),
        ),
        Transform::from_xyz(
            zone_center_x(PLAYER_FORT_START_COL, FORTRESS_COLS),
            battlefield_center_y(),
            0.0,
        ),
        PlayerFortress,
        DespawnOnExit(GameState::InGame),
    ));

    // Building zone (dark blue-gray)
    commands.spawn((
        Sprite::from_color(
            BUILD_ZONE_COLOR,
            Vec2::new(BUILD_ZONE_COLS as f32 * CELL_SIZE, BATTLEFIELD_HEIGHT),
        ),
        Transform::from_xyz(
            zone_center_x(BUILD_ZONE_START_COL, BUILD_ZONE_COLS),
            battlefield_center_y(),
            0.0,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Combat zone (dark gray)
    commands.spawn((
        Sprite::from_color(
            COMBAT_ZONE_COLOR,
            Vec2::new(COMBAT_ZONE_COLS as f32 * CELL_SIZE, BATTLEFIELD_HEIGHT),
        ),
        Transform::from_xyz(
            zone_center_x(COMBAT_ZONE_START_COL, COMBAT_ZONE_COLS),
            battlefield_center_y(),
            0.0,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Enemy fortress (red) — has marker for Ticket 8 (fortress HP)
    commands.spawn((
        Sprite::from_color(
            ENEMY_FORT_COLOR,
            Vec2::new(FORTRESS_COLS as f32 * CELL_SIZE, BATTLEFIELD_HEIGHT),
        ),
        Transform::from_xyz(
            zone_center_x(ENEMY_FORT_START_COL, FORTRESS_COLS),
            battlefield_center_y(),
            0.0,
        ),
        EnemyFortress,
        DespawnOnExit(GameState::InGame),
    ));
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes

#### Manual Verification:
- [ ] Run game, press SPACE → see colored zones on screen
- [ ] Blue rectangle on far left (player fortress)
- [ ] Darker area for building zone
- [ ] Wide combat zone in the middle
- [ ] Red rectangle on far right (enemy fortress)

**Implementation Note**: After completing this phase and automated verification passes, pause for manual confirmation before proceeding to Phase 3.

---

## Phase 3: Camera Positioning & Panning

### Overview
Modify the camera to start positioned over the player's building zone with `FixedVertical` scaling. Add a horizontal panning system with keyboard controls and boundary clamping.

### Changes Required:

#### 1. Add `setup_camera_for_battlefield` system in `src/battlefield.rs`

Keep the camera as a global entity spawned at `Startup` in `main.rs` (it persists across states for menus). Add a system that repositions it and sets the projection when entering `InGame`.

**Important API note**: Must query `&mut Projection` (the enum Component), NOT `&mut OrthographicProjection`. Pattern-match to access the orthographic variant.

```rust
use bevy::camera::ScalingMode;

/// Camera panning speed in pixels per second.
const CAMERA_PAN_SPEED: f32 = 500.0;

fn setup_camera_for_battlefield(
    mut camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let (transform, projection) = &mut *camera;

    // Position camera centered on the building zone (X), centered on battlefield height (Y)
    let build_zone_center_x = zone_center_x(BUILD_ZONE_START_COL, BUILD_ZONE_COLS);
    transform.translation.x = build_zone_center_x;
    transform.translation.y = battlefield_center_y();

    // Set projection scaling so the full battlefield height fits the viewport
    if let Projection::Orthographic(ref mut ortho) = **projection {
        ortho.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: BATTLEFIELD_HEIGHT,
        };
    }
}
```

Register this system in `BattlefieldPlugin::build` alongside `spawn_battlefield`:
```rust
app.add_systems(
    OnEnter(GameState::InGame),
    (spawn_battlefield, setup_camera_for_battlefield),
)
```

#### 2. Implement `camera_pan` system in `src/battlefield.rs`

Replace the stub with the real implementation. Uses `Single<>` for camera and window queries. **Runs only during `InGameState::Playing`** so panning stops while paused.

```rust
fn camera_pan(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera: Single<&mut Transform, With<Camera2d>>,
    windows: Single<&Window>,
) {
    let mut direction = 0.0;
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        direction += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        direction -= 1.0;
    }

    // Always apply — multiplying by 0.0 direction is a no-op.
    // Avoids `clippy::float_cmp` from `direction != 0.0` under pedantic lints.
    camera.translation.x += direction * CAMERA_PAN_SPEED * time.delta_secs();

    // Clamp camera to battlefield bounds.
    // FixedVertical scaling: visible width depends on window aspect ratio.
    let aspect_ratio = windows.width() / windows.height();
    let visible_width = BATTLEFIELD_HEIGHT * aspect_ratio;
    let half_visible = visible_width / 2.0;

    let min_x = half_visible; // Can't see past left edge (x=0)
    let max_x = BATTLEFIELD_WIDTH - half_visible; // Can't see past right edge

    camera.translation.x = camera.translation.x.clamp(min_x, max_x);
}
```

#### 3. Update `BattlefieldPlugin` final registration

```rust
use crate::InGameState;

impl Plugin for BattlefieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
                OnEnter(GameState::InGame),
                (spawn_battlefield, setup_camera_for_battlefield),
            )
            .add_systems(
                Update,
                camera_pan.run_if(in_state(InGameState::Playing)),
            );
    }
}
```

**Note**: `camera_pan` uses `in_state(InGameState::Playing)` so it pauses when the game is paused.

#### 4. `src/main.rs` — no changes needed

The camera is spawned at `Startup` by `GamePlugin` (moved there in Phase 0). The battlefield module repositions it on `OnEnter(InGame)`.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (clippy + tests + fmt)
- [ ] No warnings

#### Manual Verification:
- [ ] Camera starts centered on the building zone (blue fortress visible to the left)
- [ ] Full battlefield height fits the viewport (no vertical scrolling needed)
- [ ] Press D or Right Arrow → camera pans right smoothly
- [ ] Press A or Left Arrow → camera pans left smoothly
- [ ] Camera stops at left boundary (can't pan past player fortress)
- [ ] Camera stops at right boundary (can't pan past enemy fortress)
- [ ] Pan all the way right to see the red enemy fortress
- [ ] Pan all the way left to see the blue player fortress
- [ ] **ESC to pause → camera panning stops, battlefield visible behind overlay**
- [ ] **ESC to resume → camera panning works again**

**Implementation Note**: After completing this phase, pause for manual confirmation that panning feels good. Speed can be adjusted.

---

## Testing Strategy

**Coverage target: 90%.** Every ticket should include tests that maintain or increase coverage toward this goal. Use `cargo tarpaulin` or `cargo llvm-cov` to measure.

### Test Infrastructure Update (`src/testing.rs`)

Add a SubState-aware test helper so future tickets (2-9) don't need boilerplate to set up dual-state apps:

```rust
/// Creates a test app with both GameState and InGameState initialized,
/// already transitioned to InGame/Playing for testing gameplay systems.
#[allow(dead_code)]
pub fn create_ingame_test_app() -> App {
    let mut app = create_test_app();
    app.init_state::<crate::GameState>();
    app.add_sub_state::<crate::InGameState>();
    // Transition to InGame so SubState is active
    app.world_mut()
        .resource_mut::<NextState<crate::GameState>>()
        .set(crate::GameState::InGame);
    app.update(); // Apply the transition
    app
}
```

### Unit Tests (in `src/battlefield.rs`):

Pure math tests — no Bevy App needed:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn total_cols_is_82() {
        assert_eq!(TOTAL_COLS, 82);
    }

    #[test]
    fn battlefield_dimensions_consistent() {
        assert_eq!(BATTLEFIELD_WIDTH, TOTAL_COLS as f32 * CELL_SIZE);
        assert_eq!(BATTLEFIELD_HEIGHT, BATTLEFIELD_ROWS as f32 * CELL_SIZE);
    }

    #[test]
    fn col_to_world_x_centers_in_cell() {
        assert_eq!(col_to_world_x(0), 32.0); // First cell center
        assert_eq!(col_to_world_x(1), 96.0); // Second cell center
    }

    #[test]
    fn row_to_world_y_centers_in_cell() {
        assert_eq!(row_to_world_y(0), 32.0);
        assert_eq!(row_to_world_y(9), 608.0); // Last row center
    }

    #[test]
    fn zone_start_columns_are_sequential() {
        assert_eq!(PLAYER_FORT_START_COL, 0);
        assert_eq!(BUILD_ZONE_START_COL, 2);
        assert_eq!(COMBAT_ZONE_START_COL, 8);
        assert_eq!(ENEMY_FORT_START_COL, 80);
    }

    #[test]
    fn zone_center_x_calculates_correctly() {
        // Player fortress: cols 0-1, center at col 1.0 * 64 / 2 = 64.0
        assert_eq!(zone_center_x(0, 2), 64.0);
        // Build zone: cols 2-7, center at 2*64 + 6*64/2 = 128 + 192 = 320.0
        assert_eq!(zone_center_x(BUILD_ZONE_START_COL, BUILD_ZONE_COLS), 320.0);
    }

    #[test]
    fn battlefield_center_y_is_half_height() {
        assert_eq!(battlefield_center_y(), BATTLEFIELD_HEIGHT / 2.0);
    }
}
```

### Integration Tests (in `src/battlefield.rs`):

System-level tests using `create_ingame_test_app`. These test the spawn and camera systems in a headless Bevy App:

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::testing::{create_ingame_test_app, tick};

    /// Helper: set up an app with BattlefieldPlugin and transition to InGame.
    fn create_battlefield_test_app() -> App {
        let mut app = create_ingame_test_app();
        app.add_plugins(BattlefieldPlugin);
        app.update(); // Run OnEnter(InGame) systems
        app
    }

    #[test]
    fn spawn_battlefield_creates_five_sprites() {
        let app = create_battlefield_test_app();
        let sprite_count = app
            .world()
            .query_filtered::<(), With<Sprite>>()
            .iter(app.world())
            .count();
        assert_eq!(sprite_count, 5); // background + 4 zones
    }

    #[test]
    fn spawn_battlefield_creates_player_fortress() {
        let app = create_battlefield_test_app();
        let count = app
            .world()
            .query_filtered::<(), With<PlayerFortress>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn spawn_battlefield_creates_enemy_fortress() {
        let app = create_battlefield_test_app();
        let count = app
            .world()
            .query_filtered::<(), With<EnemyFortress>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn all_battlefield_entities_have_despawn_on_exit() {
        let app = create_battlefield_test_app();
        let with_despawn = app
            .world()
            .query_filtered::<(), (With<Sprite>, With<DespawnOnExit<GameState>>)>()
            .iter(app.world())
            .count();
        assert_eq!(with_despawn, 5); // All 5 sprites have DespawnOnExit
    }

    #[test]
    fn player_fortress_positioned_on_left() {
        let app = create_battlefield_test_app();
        let transform = app
            .world()
            .query_filtered::<&Transform, With<PlayerFortress>>()
            .single(app.world());
        // Player fortress center should be near the left edge
        assert!(transform.translation.x < BATTLEFIELD_WIDTH / 4.0);
    }

    #[test]
    fn enemy_fortress_positioned_on_right() {
        let app = create_battlefield_test_app();
        let transform = app
            .world()
            .query_filtered::<&Transform, With<EnemyFortress>>()
            .single(app.world());
        // Enemy fortress center should be near the right edge
        assert!(transform.translation.x > BATTLEFIELD_WIDTH * 3.0 / 4.0);
    }
}
```

### State Architecture Tests (in `src/lib.rs`):

Already defined in Phase 0:
- `game_state_default_is_loading`
- `game_states_are_distinct`
- `in_game_state_default_is_playing`
- `in_game_states_are_distinct`

### Coverage Analysis

| Module | Lines (approx) | Test approach | Expected coverage |
|--------|:-:|---|:-:|
| `lib.rs` (states) | ~15 | Unit tests on enum defaults/equality | 100% |
| `game/mod.rs` (plugin + camera) | ~15 | Integration tests (camera spawn verified via battlefield tests) | ~80% |
| `battlefield.rs` (constants + helpers) | ~50 | Unit tests | 100% |
| `battlefield.rs` (spawn system) | ~50 | Integration tests (entity counts, positions, markers) | ~90% |
| `battlefield.rs` (camera systems) | ~30 | Integration tests (position, clamping) | ~85% |
| `screens/loading.rs` | ~25 | Covered by state transition tests | ~70% |
| `screens/main_menu.rs` | ~30 | Covered by state transition tests | ~70% |
| `screens/in_game.rs` | ~25 | Covered by input handling tests | ~70% |
| `screens/paused.rs` | ~30 | Covered by pause/resume tests | ~70% |
| `prelude.rs` | ~5 | Re-exports only (no logic) | N/A |
| **Weighted total** | | | **~85-90%** |

**Note**: Screen plugins are mostly `commands.spawn(...)` calls triggered by state transitions. They are partially covered by integration tests that transition states, but full branch coverage of UI setup functions requires spawning into those states. Add state transition integration tests if coverage falls below 90%.

### Manual Testing Steps:
1. `cargo run` → press SPACE to start game
2. Verify battlefield layout visible with colored zones
3. Pan left/right with A/D or arrow keys
4. Verify camera clamps at both boundaries
5. Press ESC to pause → panning stops, battlefield visible behind overlay
6. Press ESC to resume → panning resumes
7. Press Q from pause to return to main menu — battlefield cleaned up
8. Re-enter game → battlefield re-created fresh

## Performance Considerations

- Only 5 sprites spawned (background + 4 zones) — negligible
- Camera panning uses `Time::delta_secs()` for frame-rate independence
- `FixedVertical` scaling mode handles window resizing gracefully

## System Ordering Notes

For this ticket, no `ApplyDeferred` is needed because:
- `spawn_battlefield` and `setup_camera_for_battlefield` run in the same `OnEnter` schedule — but `setup_camera_for_battlefield` queries the pre-existing camera entity (spawned at `Startup` by `GamePlugin`), not the newly spawned battlefield entities.
- `camera_pan` runs in `Update`, long after all `OnEnter` commands have been applied.

**Future tickets**: If a system spawns entities with markers and another system in the same schedule queries `Added<Marker>`, you need `ApplyDeferred` between them. Document this pattern for Ticket 2+.

## File Summary

| File | Action |
|------|--------|
| `src/lib.rs` | **Edit** — remove `Paused` from `GameState`, add `InGameState` SubState, remove empty module declarations (`components`, `systems`, `resources`, `ui`), add `pub mod battlefield;`, update tests |
| `src/game/mod.rs` | **Edit** — add `add_sub_state::<InGameState>()`, move `setup_camera` here from `main.rs`, add `#[derive(Debug)]` |
| `src/components/` | **Delete directory** — all `CleanupXxx` markers removed, module no longer declared |
| `src/systems/` | **Delete directory** — `cleanup_entities` system removed, module no longer declared |
| `src/resources/` | **Delete directory** — empty placeholder, module no longer declared |
| `src/ui/` | **Delete directory** — empty placeholder, module no longer declared |
| `src/screens/loading.rs` | **Edit** — replace `CleanupLoading` with `DespawnOnExit`, remove `OnExit` system, add `#[derive(Debug)]` |
| `src/screens/main_menu.rs` | **Edit** — replace `CleanupMainMenu` with `DespawnOnExit`, remove `OnExit` system, add `#[derive(Debug)]` |
| `src/screens/in_game.rs` | **Edit** — remove `setup_game`, update ESC handler for `InGameState`, remove `OnExit` system, add `#[derive(Debug)]` + doc comment |
| `src/screens/paused.rs` | **Edit** — replace `CleanupPaused` with `DespawnOnExit(InGameState::Paused)`, remove input handler, add `#[derive(Debug)]` |
| `src/prelude.rs` | **Edit** — remove `components::*` glob, remove commented `resources` line, add `InGameState` export |
| `src/main.rs` | **Edit** — remove `setup_camera` (moved to `GamePlugin`), add `BattlefieldPlugin` to plugins tuple |
| `src/testing.rs` | **Edit** — add `create_ingame_test_app()` SubState-aware test helper |
| `src/battlefield.rs` | **New** — constants, markers, plugin, spawn & camera systems, tests |

## Implementation Brief for Agents

### Known Bevy 0.18 Gotchas
- `Projection` enum is the Component, not `OrthographicProjection`
- `ScalingMode` at `bevy::camera::ScalingMode` — NOT in prelude, must import explicitly
- `Camera2d` auto-adds `Projection` via `#[require]` — spawning bare `Camera2d` gives orthographic by default for 2D
- Use `Single<D, F>` instead of `Query` + `.single_mut()` — system skips silently if 0 or >1 match
- `Sprite::from_color(color, size)` is the clean way to make colored rectangles
- `despawn()` is already recursive — no `despawn_recursive()` in Bevy 0.18
- `DespawnOnExit(state)` is in the prelude — no special import needed
- `SubStates` derive needs `#[source(ParentState = ParentState::Variant)]`

### Clippy Configuration
- `all` + `pedantic` + `nursery` lints enabled as warnings, `-D warnings` in CI
- Three allows: `needless_pass_by_value`, `too_many_arguments`, `type_complexity` (Bevy patterns)
- Always run `cargo build` after each file change — don't batch

### What Future Tickets Need from This Foundation
- **`InGameState::Playing`** — all game-logic systems use `run_if(in_state(InGameState::Playing))` to auto-pause
- **`DespawnOnExit(GameState::InGame)`** — on all gameplay entities (battlefield, units, buildings)
- **`DespawnOnExit(InGameState::Paused)`** — only on pause overlay UI
- **Marker components** on fortress entities (`PlayerFortress`, `EnemyFortress`) — Tickets 4, 5, 8, 9 query these
- **Public constants** for zone positions and dimensions — all tickets use these
- **Public helper functions** (`col_to_world_x`, `row_to_world_y`) — Tickets 2, 3 use these for placement

## Deferred Architecture Notes

These items were identified during architecture review but deferred from Ticket 1. They are tracked in the relevant future tickets.

| Item | Description | When to Address | Tracked In |
|------|-------------|-----------------|------------|
| `screens/` directory naming | `screens/in_game.rs` is becoming an input dispatcher, not a visual screen. If it stays thin, the name is fine. If it grows, consider renaming `screens/` to `states/` or moving `in_game.rs` to `src/game/`. | Monitor during T2-T7 | Ticket 2 |
| `handle_game_input` growth | Currently only ESC/Q. As tickets add input (mouse placement in T2, economy UI in T6), keep game-specific inputs in their domain plugins — don't stuff them into `handle_game_input`. | Monitor during T2, T6 | Ticket 2 |
| `GameState` location | `GameState`/`InGameState` live in `lib.rs` (crate root). If 3+ state enums accumulate, consider moving them to `src/game/states.rs` or `src/states.rs`. | Ticket 3+ (when `Team` enum is added) | Ticket 3 |
| Prelude glob convention | The prelude now uses explicit imports. As components grow, keep preferring explicit imports from domain modules over glob re-exports. | Ongoing | Ticket 2 |

## References

- Original ticket: `thoughts/shared/tickets/2026-02-08-0001-camera-battlefield-layout.md`
- Research doc: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.1, Section 7)
- Bevy SubStates example: `~/.cargo/registry/src/.../bevy-0.18.0/examples/state/sub_states.rs`
- Bevy game_menu example: `~/.cargo/registry/src/.../bevy-0.18.0/examples/games/game_menu.rs`

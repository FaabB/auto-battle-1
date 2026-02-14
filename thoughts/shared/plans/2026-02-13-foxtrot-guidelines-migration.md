# Foxtrot Guidelines Migration Plan

## Overview

Migrate the auto-battle-1 codebase from struct-based plugins to function plugins, tighten visibility, move states to `screens/`, create a theme system, add architecture documentation, and update all future tickets. Based on the Foxtrot analysis at `thoughts/shared/research/2026-02-10-foxtrot-analysis-project-guidelines.md`.

## Current State Analysis

The codebase has 6 struct plugins, all components/constants use `pub` visibility, states live centrally in `lib.rs`, and there's no shared theme system for UI. The structure is sound but doesn't follow the Foxtrot conventions for scalability.

### Key Discoveries:
- 6 struct plugins: `CoreGamePlugin`, `LoadingScreenPlugin`, `MainMenuScreenPlugin`, `InGameScreenPlugin`, `BattlefieldPlugin`, `BuildingPlugin` — all need conversion to function plugins
- `lib.rs:32-52` — States (`GameState`, `InGameState`) are here but should move to `screens/`
- `prelude.rs` — Only re-exports `bevy::prelude::*` + 2 state types; most files already import directly
- `game/mod.rs` — Only 17 lines (camera spawn + state init); should be flat `game.rs`
- `screens/mod.rs` — Currently just re-exports 3 plugin structs; will become plugin compositor
- No `#[states(scoped_entities)]` attribute on state enums
- All components are `pub` instead of `pub(crate)`
- No shared theme system — each screen has inline colors and UI patterns
- Tickets 3-9 reference wrong paths (`src/components/mod.rs`, `src/resources/mod.rs`)

## Desired End State

After this plan is complete:
1. All plugins use `pub(super) fn plugin(app: &mut App)` pattern
2. `lib.rs` has a `pub fn plugin` that composes all top-level plugins (clean API for `main.rs`)
3. States live in `screens/mod.rs` (GameState) and `screens/in_game.rs` (InGameState)
4. Both states have `#[states(scoped_entities)]`
5. `prelude.rs` is deleted
6. All visibility is tightened: `pub(crate)` for cross-module types, `pub(super)` for plugins, private for systems
7. `src/theme/` module exists with palette and widget helpers
8. `building.rs` promoted to `building/` directory (`mod.rs` + `placement.rs`)
9. `src/dev_tools/` module exists, feature-gated on `dev`, ready for ticket 4's debug spawner
10. Global `GameSet` SystemSet enum defines Update schedule ordering for all domain plugins
11. Test helpers refactored with entity counting utilities and consistent app creation
12. `ARCHITECTURE.md` documents all conventions
13. All future tickets (3-9) reference correct file paths and patterns
14. All tests pass, `make check` succeeds

## What We're NOT Doing

- **Not creating `third_party/` module** — no complex third-party plugins beyond Bevy itself
- **Not adding observers** — no entity lifecycle patterns that need them yet; our spawn-with-all-components pattern works well

## Implementation Approach

Five phases, each with a clean compile gate:

1. **Phase 1**: Core refactoring — plugins, visibility, states, prelude, building/ promotion, dev_tools, SystemSets (biggest change)
2. **Phase 2**: Theme system — extract shared UI patterns
3. **Phase 3**: Test refactoring — entity counting helpers, consistent app creation
4. **Phase 4**: Architecture documentation
5. **Phase 5**: Ticket updates

---

## Phase 1: Core Plugin & Visibility Refactoring

### Overview
Convert all 6 struct plugins to function plugins, move states to `screens/`, eliminate prelude, tighten visibility, and add `#[states(scoped_entities)]`. This is one atomic change — the intermediate steps don't compile independently.

### Changes Required:

#### 1. Delete `src/prelude.rs`
**File**: `src/prelude.rs`
**Action**: Delete entirely. All files already import `bevy::prelude::*` directly.

#### 2. Demote `src/game/mod.rs` → `src/game.rs`
**Action**: Delete `src/game/mod.rs` and `src/game/` directory. Create `src/game.rs`.

**File**: `src/game.rs` (new)
```rust
//! Global game setup (camera, cross-cutting systems).

use bevy::prelude::*;

/// Global camera — persists across all states.
pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, setup_camera);
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
```

Note: `init_state::<GameState>()` moves to `screens::plugin` since states move there.

#### 3. Rewrite `src/screens/mod.rs`
**File**: `src/screens/mod.rs`
**Changes**: Own `GameState`, re-export `InGameState`, compose sub-plugins.

```rust
//! Screen plugins and state management.

mod in_game;
mod loading;
mod main_menu;

use bevy::prelude::*;

/// Primary game states.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[states(scoped_entities)]
pub(crate) enum GameState {
    /// Initial loading state.
    #[default]
    Loading,
    /// Main menu state.
    MainMenu,
    /// Active gameplay state.
    InGame,
}

pub(crate) use in_game::InGameState;

pub(super) fn plugin(app: &mut App) {
    app.init_state::<GameState>();
    app.add_plugins((
        loading::plugin,
        main_menu::plugin,
        in_game::plugin,
    ));
}
```

#### 4. Rewrite `src/screens/in_game.rs`
**File**: `src/screens/in_game.rs`
**Changes**: Own `InGameState`, convert to function plugin.

```rust
//! In-game screen plugin: pause/unpause input, pause overlay UI, quit to menu.
//!
//! Gameplay visuals and logic are handled by domain plugins
//! (e.g., `BattlefieldPlugin`). This plugin owns the pause overlay
//! and keybindings that operate across all `InGameState` sub-states.

use bevy::prelude::*;

use super::GameState;

/// Sub-states within `InGame`. Only exists while `GameState::InGame` is active.
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(GameState = GameState::InGame)]
#[states(scoped_entities)]
pub(crate) enum InGameState {
    /// Normal gameplay.
    #[default]
    Playing,
    /// Game is paused (overlay on gameplay).
    Paused,
}

pub(super) fn plugin(app: &mut App) {
    app.add_sub_state::<InGameState>()
        .add_systems(OnEnter(InGameState::Paused), setup_pause_menu)
        .add_systems(
            Update,
            handle_game_input.run_if(in_state(GameState::InGame)),
        );
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

#### 5. Rewrite `src/screens/loading.rs`
**File**: `src/screens/loading.rs`
**Changes**: Convert to function plugin, use `super::GameState`.

```rust
//! Loading screen plugin.

use bevy::prelude::*;

use super::GameState;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameState::Loading), setup_loading_screen)
        .add_systems(
            Update,
            check_loading_complete.run_if(in_state(GameState::Loading)),
        );
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

#### 6. Rewrite `src/screens/main_menu.rs`
**File**: `src/screens/main_menu.rs`
**Changes**: Convert to function plugin, use `super::GameState`.

```rust
//! Main menu screen plugin.

use bevy::prelude::*;

use super::GameState;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameState::MainMenu), setup_main_menu)
        .add_systems(
            Update,
            handle_main_menu_input.run_if(in_state(GameState::MainMenu)),
        );
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

#### 7. Rewrite `src/battlefield/mod.rs`
**File**: `src/battlefield/mod.rs`
**Changes**: Convert to function plugin, tighten visibility, update state imports.

Key changes:
- `pub struct BattlefieldPlugin;` + `impl Plugin` → `pub(super) fn plugin(app: &mut App)`
- All `pub` components → `pub(crate)` (PlayerFortress, EnemyFortress, BuildZone, CombatZone, BattlefieldBackground, BuildSlot)
- All `pub` constants → `pub(crate)` (CELL_SIZE, BATTLEFIELD_ROWS, etc.)
- `pub struct BattlefieldSetup;` → `pub(crate) struct BattlefieldSetup;`
- Resource `GridIndex` methods: `pub fn` → `pub(crate) fn`
- Helper functions: `pub fn col_to_world_x` → `pub(crate) fn`
- Import: `use crate::{GameState, InGameState}` → `use crate::screens::{GameState, InGameState}`

```rust
//! Battlefield layout constants, markers, and systems.

mod camera;
mod renderer;

use std::collections::HashMap;

use bevy::prelude::*;

use crate::screens::{GameState, InGameState};

// === Grid Constants ===

/// Size of a single grid cell in pixels.
pub(crate) const CELL_SIZE: f32 = 64.0;

/// Number of rows in the battlefield.
pub(crate) const BATTLEFIELD_ROWS: u16 = 10;

/// Number of columns for each fortress.
pub(crate) const FORTRESS_COLS: u16 = 2;

/// Number of columns in the building zone.
pub(crate) const BUILD_ZONE_COLS: u16 = 6;

/// Number of columns in the combat zone.
pub(crate) const COMBAT_ZONE_COLS: u16 = 72;

/// Total columns across the entire battlefield.
pub(crate) const TOTAL_COLS: u16 = FORTRESS_COLS + BUILD_ZONE_COLS + COMBAT_ZONE_COLS + FORTRESS_COLS;

/// Total battlefield width in pixels.
pub(crate) const BATTLEFIELD_WIDTH: f32 = TOTAL_COLS as f32 * CELL_SIZE;

/// Total battlefield height in pixels.
pub(crate) const BATTLEFIELD_HEIGHT: f32 = BATTLEFIELD_ROWS as f32 * CELL_SIZE;

// === Zone Column Ranges ===

pub(crate) const PLAYER_FORT_START_COL: u16 = 0;
pub(crate) const BUILD_ZONE_START_COL: u16 = FORTRESS_COLS;
pub(crate) const COMBAT_ZONE_START_COL: u16 = FORTRESS_COLS + BUILD_ZONE_COLS;
pub(crate) const ENEMY_FORT_START_COL: u16 = FORTRESS_COLS + BUILD_ZONE_COLS + COMBAT_ZONE_COLS;

// === Zone Pixel Boundaries ===

pub(crate) const BUILD_ZONE_START_X: f32 = BUILD_ZONE_START_COL as f32 * CELL_SIZE;
pub(crate) const BUILD_ZONE_END_X: f32 = (BUILD_ZONE_START_COL + BUILD_ZONE_COLS) as f32 * CELL_SIZE;

// === Marker Components ===

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub(crate) struct PlayerFortress;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub(crate) struct EnemyFortress;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub(crate) struct BuildZone;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub(crate) struct CombatZone;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub(crate) struct BattlefieldBackground;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub(crate) struct BuildSlot {
    pub(crate) row: u16,
    pub(crate) col: u16,
}

// === Resources ===

#[derive(Resource, Default, Debug)]
pub(crate) struct GridIndex {
    slots: HashMap<(u16, u16), Entity>,
}

impl GridIndex {
    pub(crate) fn insert(&mut self, col: u16, row: u16, entity: Entity) {
        self.slots.insert((col, row), entity);
    }

    #[must_use]
    pub(crate) fn get(&self, col: u16, row: u16) -> Option<Entity> {
        self.slots.get(&(col, row)).copied()
    }
}

// === Helper Functions ===

#[must_use]
pub(crate) fn col_to_world_x(col: u16) -> f32 {
    f32::from(col).mul_add(CELL_SIZE, CELL_SIZE / 2.0)
}

#[must_use]
pub(crate) fn row_to_world_y(row: u16) -> f32 {
    f32::from(row).mul_add(CELL_SIZE, CELL_SIZE / 2.0)
}

#[must_use]
pub(crate) fn zone_center_x(start_col: u16, width_cols: u16) -> f32 {
    f32::from(start_col).mul_add(CELL_SIZE, (f32::from(width_cols) * CELL_SIZE) / 2.0)
}

#[must_use]
pub(crate) fn battlefield_center_y() -> f32 {
    BATTLEFIELD_HEIGHT / 2.0
}

// === System Sets ===

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct BattlefieldSetup;

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<PlayerFortress>()
        .register_type::<EnemyFortress>()
        .register_type::<BuildZone>()
        .register_type::<CombatZone>()
        .register_type::<BattlefieldBackground>()
        .register_type::<BuildSlot>()
        .init_resource::<GridIndex>();

    app.add_systems(
        OnEnter(GameState::InGame),
        (
            renderer::spawn_battlefield,
            camera::setup_camera_for_battlefield,
        )
            .chain()
            .in_set(BattlefieldSetup),
    )
    .add_systems(
        Update,
        camera::camera_pan.run_if(in_state(InGameState::Playing)),
    );
}

// Tests remain the same but with updated imports (use crate::screens:: instead of crate::)
```

#### 8. Update `src/battlefield/camera.rs`
**File**: `src/battlefield/camera.rs`
**Changes**: No structural changes needed — already uses `super::` imports and `pub(super)` visibility. No changes required.

#### 9. Update `src/battlefield/renderer.rs`
**File**: `src/battlefield/renderer.rs`
**Changes**: Update state import path.

```rust
// Change:
use crate::{GameState, Z_BACKGROUND, Z_GRID, Z_ZONE};
// To:
use crate::screens::GameState;
use crate::{Z_BACKGROUND, Z_GRID, Z_ZONE};
```

#### 10. Promote `src/building.rs` → `src/building/` directory
**Action**: Delete `src/building.rs`. Create `src/building/mod.rs` and `src/building/placement.rs`.

`building.rs` is already ~500 lines and will grow with tickets 3 (production) and 6 (costs, selector). Promoting now avoids churn later.

**Split strategy**:
- `building/mod.rs` — plugin fn, components, resources, constants, helper functions (shared API)
- `building/placement.rs` — placement systems (spawn_grid_cursor, update_grid_cursor, handle_building_placement)

**File**: `src/building/mod.rs` (new)
```rust
//! Building placement: grid cursor, hover highlight, and click-to-place buildings.

mod placement;

use bevy::prelude::*;

use crate::battlefield::{
    BATTLEFIELD_HEIGHT, BUILD_ZONE_START_COL, BattlefieldSetup, CELL_SIZE, GridIndex,
};
use crate::screens::{GameState, InGameState};
use crate::{Z_BUILDING, Z_GRID_CURSOR};

// === Constants ===

/// Color for the grid cursor hover highlight.
const GRID_CURSOR_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.2);

/// Barracks building color (dark blue).
const BARRACKS_COLOR: Color = Color::srgb(0.15, 0.2, 0.6);
/// Farm building color (green).
const FARM_COLOR: Color = Color::srgb(0.2, 0.6, 0.1);

/// Building sprite size (slightly smaller than cell to show grid outline).
const BUILDING_SPRITE_SIZE: f32 = CELL_SIZE - 4.0;

// === Components ===

#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub(crate) struct Building {
    pub(crate) building_type: BuildingType,
    pub(crate) grid_col: u16,
    pub(crate) grid_row: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub(crate) enum BuildingType {
    Barracks,
    Farm,
}

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub(crate) struct Occupied;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub(crate) struct GridCursor;

#[derive(Resource, Default, Debug, Reflect)]
#[reflect(Resource)]
pub(crate) struct HoveredCell(pub(crate) Option<(u16, u16)>);

// === Helper Functions ===

#[must_use]
pub(crate) fn world_to_build_grid(world_pos: Vec2) -> Option<(u16, u16)> {
    use crate::battlefield::{BUILD_ZONE_END_X, BUILD_ZONE_START_X};
    // ... (same logic as current building.rs)
}

#[must_use]
pub(crate) const fn building_color(building_type: BuildingType) -> Color {
    match building_type {
        BuildingType::Barracks => BARRACKS_COLOR,
        BuildingType::Farm => FARM_COLOR,
    }
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Building>()
        .register_type::<BuildingType>()
        .register_type::<Occupied>()
        .register_type::<GridCursor>()
        .register_type::<HoveredCell>()
        .init_resource::<HoveredCell>();

    app.add_systems(
        OnEnter(GameState::InGame),
        placement::spawn_grid_cursor.after(BattlefieldSetup),
    )
    .add_systems(
        Update,
        (placement::update_grid_cursor, placement::handle_building_placement)
            .chain_ignore_deferred()
            .run_if(in_state(InGameState::Playing)),
    );
}

// Unit tests for helpers stay here (world_to_build_grid, building_color)
// Integration tests for placement move to building/placement.rs
```

**File**: `src/building/placement.rs` (new)
```rust
//! Building placement systems: grid cursor spawning, hover tracking, click-to-place.

use bevy::prelude::*;

use super::{
    Building, BuildingType, GridCursor, HoveredCell, Occupied,
    BUILDING_SPRITE_SIZE, GRID_CURSOR_COLOR, building_color,
};
use crate::battlefield::{BUILD_ZONE_START_COL, GridIndex, col_to_world_x, row_to_world_y};
use crate::screens::{GameState, InGameState};
use crate::{Z_BUILDING, Z_GRID_CURSOR};

use super::CELL_SIZE;

pub(super) fn spawn_grid_cursor(mut commands: Commands) {
    // ... (same as current)
}

pub(super) fn update_grid_cursor(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut cursor: Single<(&mut Transform, &mut Visibility), With<GridCursor>>,
    mut hovered: ResMut<HoveredCell>,
) {
    // ... (same as current)
}

pub(super) fn handle_building_placement(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredCell>,
    grid_index: Res<GridIndex>,
    occupied: Query<(), With<Occupied>>,
) {
    // ... (same as current)
}

// Integration tests for placement systems live here
```

#### 11. Rewrite `src/lib.rs`
**File**: `src/lib.rs`
**Changes**: Remove states (moved to screens/), remove prelude module, add `pub fn plugin` compositor, add `GameSet` SystemSet, add dev_tools module, keep Z-layers.

```rust
//! Auto-battle game library.

pub mod battlefield;
pub mod building;
#[cfg(feature = "dev")]
pub mod dev_tools;
pub mod game;
pub mod screens;
pub mod theme;
#[cfg(test)]
pub mod testing;

use bevy::prelude::*;

// === Z-Layer Constants ===
// Cross-cutting sprite ordering used by multiple domain plugins.

/// Background layer (behind everything).
pub(crate) const Z_BACKGROUND: f32 = -1.0;
/// Zone sprites (fortresses, build zone, combat zone).
pub(crate) const Z_ZONE: f32 = 0.0;
/// Grid cell sprites in the build zone.
pub(crate) const Z_GRID: f32 = 1.0;
/// Grid cursor / hover highlight.
pub(crate) const Z_GRID_CURSOR: f32 = 2.0;
/// Placed buildings.
pub(crate) const Z_BUILDING: f32 = 3.0;
/// Units (future: Ticket 3).
pub(crate) const Z_UNIT: f32 = 4.0;
/// Health bars (future: Ticket 5).
pub(crate) const Z_HEALTH_BAR: f32 = 5.0;

// === Global System Ordering ===
// Domain plugins register their Update systems in the appropriate set.
// Sets are chained so they run in order every frame.

/// Global system sets for the Update schedule.
/// Domain plugins use `.in_set(GameSet::Xxx)` to slot into the correct phase.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum GameSet {
    /// Input handling: camera pan, building placement, UI interaction.
    Input,
    /// Building production: barracks spawn timers, unit creation.
    Production,
    /// AI: target finding, decision making.
    Ai,
    /// Movement: units moving toward targets.
    Movement,
    /// Combat: attack timers, damage application.
    Combat,
    /// Death: despawn dead entities, cleanup.
    Death,
    /// UI: health bars, gold display, wave counter.
    Ui,
}

/// Composes all game plugins. Call from `main.rs`.
pub fn plugin(app: &mut App) {
    // Global system ordering
    app.configure_sets(
        Update,
        (
            GameSet::Input,
            GameSet::Production,
            GameSet::Ai,
            GameSet::Movement,
            GameSet::Combat,
            GameSet::Death,
            GameSet::Ui,
        )
            .chain(),
    );

    app.add_plugins((
        game::plugin,
        screens::plugin,
        battlefield::plugin,
        building::plugin,
        theme::plugin,
    ));

    #[cfg(feature = "dev")]
    app.add_plugins(dev_tools::plugin);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    use crate::screens::{GameState, InGameState};

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

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn z_layers_are_ordered() {
        assert!(Z_BACKGROUND < Z_ZONE);
        assert!(Z_ZONE < Z_GRID);
        assert!(Z_GRID < Z_GRID_CURSOR);
        assert!(Z_GRID_CURSOR < Z_BUILDING);
        assert!(Z_BUILDING < Z_UNIT);
        assert!(Z_UNIT < Z_HEALTH_BAR);
    }
}
```

Note: `pub mod theme;` is added here — Phase 2 creates the theme module.
For Phase 1 to compile independently, we need a minimal `theme/mod.rs` stub:

```rust
//! Theme system (palette, widgets). Populated in Phase 2.

pub(super) fn plugin(_app: &mut bevy::prelude::App) {}
```

#### 11b. Create `src/dev_tools/mod.rs`
**File**: `src/dev_tools/mod.rs` (new)
**Purpose**: Feature-gated module for debug-only tools. Ticket 4's debug enemy spawner will live here.

```rust
//! Development tools — only included with `cargo run --features dev`.
//!
//! Debug overlays, test spawners, and inspector setup go here.
//! This module is stripped from release builds.

use bevy::prelude::*;

pub(super) fn plugin(_app: &mut App) {
    // Future: ticket 4 adds debug enemy spawner here
    // Future: inspector, state logging, performance overlay
}
```

#### 11c. Update `Cargo.toml`
**File**: `Cargo.toml`
**Changes**: Add `dev` feature flag for dev_tools.

```toml
[features]
default = ["dev"]
dev = ["bevy/dynamic_linking"]
```

The `default = ["dev"]` means `cargo run` includes dev tools. Release builds use `cargo run --release --no-default-features`.

#### 11d. Wire existing systems into `GameSet`
**File**: `src/battlefield/mod.rs` — camera_pan goes into `GameSet::Input`:

```rust
.add_systems(
    Update,
    camera::camera_pan
        .in_set(crate::GameSet::Input)
        .run_if(in_state(InGameState::Playing)),
)
```

**File**: `src/building/mod.rs` — placement systems go into `GameSet::Input`:

```rust
.add_systems(
    Update,
    (placement::update_grid_cursor, placement::handle_building_placement)
        .chain_ignore_deferred()
        .in_set(crate::GameSet::Input)
        .run_if(in_state(InGameState::Playing)),
)
```

#### 12. Rewrite `src/main.rs`
**File**: `src/main.rs`
**Changes**: Ultra-clean composition root. Just DefaultPlugins + our plugin.

```rust
//! Auto-battle game entry point.

use bevy::prelude::*;

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
        .add_plugins(auto_battle::plugin)
        .run();
}
```

#### 13. Update `src/testing.rs`
**File**: `src/testing.rs`
**Changes**: Update state import paths.

```rust
// All state references change:
// crate::GameState → crate::screens::GameState
// crate::InGameState → crate::screens::InGameState

pub fn create_base_test_app() -> App {
    let mut app = create_test_app();
    app.add_plugins(StatesPlugin);
    app.add_plugins(InputPlugin);
    app.add_plugins(WindowPlugin::default());
    app.init_state::<crate::screens::GameState>();
    app.add_sub_state::<crate::screens::InGameState>();
    app.world_mut().spawn(Camera2d);
    app
}

pub fn create_base_test_app_no_input() -> App {
    let mut app = create_test_app();
    app.add_plugins(StatesPlugin);
    app.add_plugins(WindowPlugin::default());
    app.init_state::<crate::screens::GameState>();
    app.add_sub_state::<crate::screens::InGameState>();
    app.world_mut().spawn(Camera2d);
    app
}

pub fn transition_to_ingame(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<crate::screens::GameState>>()
        .set(crate::screens::GameState::InGame);
    app.update();
    app.update();
}
```

#### 14. Update test imports across all files
All integration test modules need updated state imports:

**`src/battlefield/mod.rs` tests**: Change `use crate::{GameState, InGameState}` to `use crate::screens::{GameState, InGameState}` in test modules. Replace `BattlefieldPlugin` references with `plugin` (since tests are inside the module, they can access the function directly).

**`src/building.rs` tests**: Same state import change. Replace `BuildingPlugin` references with `plugin`.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (lint + test)
- [x] `cargo build` compiles without warnings
- [x] `cargo build --no-default-features` compiles (verifies dev_tools is cleanly feature-gated)
- [x] All existing tests pass with updated imports
- [ ] No `pub` items except: `lib.rs` module declarations, `lib.rs::plugin`
- [x] `src/building/` directory exists with `mod.rs` and `placement.rs`
- [x] `src/dev_tools/mod.rs` exists and is feature-gated
- [x] `GameSet` enum has 7 variants and is configured in `lib.rs::plugin`

#### Manual Verification:
- [ ] Game launches and shows loading → main menu transition
- [ ] Press SPACE → enters game with battlefield visible
- [ ] Camera panning works (A/D keys)
- [ ] Building placement works (click grid cells)
- [ ] ESC pauses, ESC again resumes, Q quits to menu
- [ ] No visual regressions

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to Phase 2.

---

## Phase 2: Theme System

### Overview
Create `src/theme/` with shared color palette and widget constructors. Refactor all screen plugins to use the theme.

### Changes Required:

#### 1. Create `src/theme/mod.rs`
**File**: `src/theme/mod.rs` (replace stub from Phase 1)

```rust
//! Shared UI theme: color palette and reusable widget constructors.

pub(crate) mod palette;
pub(crate) mod widget;

pub(super) fn plugin(_app: &mut bevy::prelude::App) {
    // No runtime setup needed yet.
    // Future: register interaction systems, theme resources.
}
```

#### 2. Create `src/theme/palette.rs`
**File**: `src/theme/palette.rs` (new)

```rust
//! Color constants for consistent UI theming.

use bevy::prelude::*;

/// Header/title text color (white).
pub(crate) const HEADER_TEXT: Color = Color::WHITE;

/// Body/subtitle text color (light gray).
pub(crate) const BODY_TEXT: Color = Color::srgb(0.7, 0.7, 0.7);

/// Semi-transparent dark overlay for pause/modal screens.
pub(crate) const OVERLAY_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);
```

#### 3. Create `src/theme/widget.rs`
**File**: `src/theme/widget.rs` (new)

```rust
//! Reusable UI widget constructors.
//!
//! Each function returns an `impl Bundle` containing the widget's styling.
//! Callers add layout (`Node`) and lifecycle (`DespawnOnExit`) separately.

use bevy::prelude::*;

use super::palette;

/// Large header text (64px, white).
pub(crate) fn header(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: 64.0,
            ..default()
        },
        TextColor(palette::HEADER_TEXT),
    )
}

/// Medium label text (32px, gray).
pub(crate) fn label(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(palette::BODY_TEXT),
    )
}

/// Full-screen semi-transparent overlay.
pub(crate) fn overlay() -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(palette::OVERLAY_BACKGROUND),
    )
}
```

#### 4. Refactor `src/screens/loading.rs` to use theme
**File**: `src/screens/loading.rs`
**Changes**: Use `widget::header` for the loading text.

```rust
use crate::theme::widget;

fn setup_loading_screen(mut commands: Commands) {
    commands.spawn((
        widget::header("Loading..."),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(50.0),
            ..default()
        },
        DespawnOnExit(GameState::Loading),
    ));
}
```

Note: The loading text was 48px before, but using `widget::header` (64px) is fine since the loading screen is temporary. If we want exact 48px, we could add a `widget::subheader` or override TextFont.

#### 5. Refactor `src/screens/main_menu.rs` to use theme
**File**: `src/screens/main_menu.rs`
**Changes**: Use `widget::header` for title, `widget::label` for prompt.

```rust
use crate::theme::widget;

fn setup_main_menu(mut commands: Commands) {
    commands.spawn((
        widget::header("Auto Battle"),
        TextFont {
            font_size: 72.0,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(30.0),
            ..default()
        },
        DespawnOnExit(GameState::MainMenu),
    ));

    commands.spawn((
        widget::label("Press SPACE to Start"),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(60.0),
            ..default()
        },
        DespawnOnExit(GameState::MainMenu),
    ));
}
```

Note: Title overrides font_size to 72px (header default is 64px). This works because spawning with `TextFont` after `widget::header` overrides the widget's TextFont.

#### 6. Refactor `src/screens/in_game.rs` to use theme
**File**: `src/screens/in_game.rs`
**Changes**: Use `widget::overlay`, `widget::header`, `widget::label`.

```rust
use crate::theme::widget;

fn setup_pause_menu(mut commands: Commands) {
    commands.spawn((
        widget::overlay(),
        DespawnOnExit(InGameState::Paused),
    ));

    commands.spawn((
        widget::header("PAUSED"),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(50.0),
            top: Val::Percent(40.0),
            ..default()
        },
        DespawnOnExit(InGameState::Paused),
    ));

    commands.spawn((
        widget::label("Press ESC to Resume | Q to Quit"),
        TextFont {
            font_size: 24.0,
            ..default()
        },
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

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `cargo build` compiles
- [x] All tests pass

#### Manual Verification:
- [ ] Loading screen text looks correct
- [ ] Main menu title and prompt look correct
- [ ] Pause overlay renders with correct semi-transparent background
- [ ] Pause text and prompt look correct
- [ ] No visual regressions from before theme refactor

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to Phase 3.

---

## Phase 3: Test Refactoring

### Overview
Add entity counting helpers and improve test app creation consistency. This is a quality-of-life improvement that makes future ticket tests easier to write.

### Changes Required:

#### 1. Add entity counting helper to `src/testing.rs`
**File**: `src/testing.rs`
**Changes**: Add a generic `count_entities` function and a `has_entity` assertion helper.

```rust
/// Count entities that match a query filter.
///
/// Usage: `assert_eq!(count_entities::<With<PlayerFortress>>(&mut app), 1);`
pub fn count_entities<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<(), F>()
        .iter(app.world())
        .count()
}

/// Assert exactly N entities match a query filter.
///
/// Panics with a descriptive message including the type name.
pub fn assert_entity_count<F: bevy::ecs::query::QueryFilter>(app: &mut App, expected: usize) {
    let actual = count_entities::<F>(app);
    assert_eq!(
        actual, expected,
        "Expected {expected} entities matching filter, found {actual}"
    );
}
```

#### 2. Refactor existing integration tests to use helpers
Replace verbose patterns like:
```rust
let count = app
    .world_mut()
    .query_filtered::<(), With<PlayerFortress>>()
    .iter(app.world())
    .count();
assert_eq!(count, 1);
```

With:
```rust
assert_entity_count::<With<PlayerFortress>>(&mut app, 1);
```

This affects all integration test modules in `battlefield/mod.rs` and `building/placement.rs`.

#### 3. Consolidate test app helpers
Ensure all test files use the shared helpers from `testing.rs` instead of duplicating setup logic. The pattern is:
- `create_base_test_app()` — for tests needing full input + window
- `create_base_test_app_no_input()` — for tests that manually inject button presses
- Domain-specific helpers (e.g., `create_battlefield_test_app()`) stay in their test modules but call the shared base helpers

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] All tests pass
- [x] No duplicate test setup code (each pattern exists in one place)

#### Manual Verification:
- [x] Test output is clear and descriptive when assertions fail

**Implementation Note**: This phase has no manual verification gate — proceed to Phase 4 after automated checks pass.

---

## Phase 4: Architecture Documentation

### Overview
Create `ARCHITECTURE.md` documenting the project's conventions and patterns, and update `CLAUDE.md` to reference it.

### Changes Required:

#### 1. Create `ARCHITECTURE.md`
**File**: `ARCHITECTURE.md` (new, project root)

This document codifies the conventions adopted from the Foxtrot analysis. It serves as the authoritative reference for how code should be organized.

Contents should cover:
1. **Plugin Architecture** — Function plugins with `pub(super) fn plugin`, composition via `add_plugins` tuples
2. **Module Structure** — When to use flat files vs directories, naming conventions
3. **Visibility Rules** — `pub(crate)` default, `pub(super)` for plugins, private for systems
4. **State Management** — States live in the module that manages them, `#[states(scoped_entities)]`
5. **Component Co-Location** — Components live with their systems, never in a shared `components/` module
6. **Theme System** — Shared colors in `theme/palette.rs`, widget constructors in `theme/widget.rs`
7. **System Registration Order** — states → resources → sub-plugins → observers → systems
8. **Global System Ordering** — `GameSet` enum and how to use `.in_set(GameSet::Xxx)`
9. **Dev Tools** — Feature-gated `dev_tools/` module, how to add debug features
10. **Testing Patterns** — `testing.rs` helpers, entity counting utilities, headless App setup, 90% coverage target
11. **Naming Conventions** — `plugin` for plugin fns, `spawn_*`/`setup_*` for OnEnter, DespawnOnExit for cleanup

#### 2. Update `CLAUDE.md`
**File**: `CLAUDE.md`
**Changes**: Add reference to `ARCHITECTURE.md`, update plugin architecture section to reflect function plugins.

Update the "Architecture" section to mention:
- Function plugins (not struct plugins)
- `pub(crate)` visibility convention
- States in `screens/` module
- Theme system in `theme/`
- `GameSet` for Update schedule ordering
- `dev_tools/` feature-gated module
- Reference `ARCHITECTURE.md` for full details

### Success Criteria:

#### Automated Verification:
- [x] `ARCHITECTURE.md` exists and is well-formatted markdown
- [x] `CLAUDE.md` references `ARCHITECTURE.md`

#### Manual Verification:
- [ ] Documentation is accurate and matches the actual codebase
- [ ] New contributors could follow the conventions by reading `ARCHITECTURE.md`

---

## Phase 5: Ticket Updates

### Overview
Update all future tickets (3-9) to reflect the new module structure, plugin pattern, and conventions.

### Changes Required:

Common changes across all tickets:
- Replace `src/components/mod.rs` references → domain module co-location
- Replace `src/resources/mod.rs` references → domain module
- Replace `src/screens/in_game.rs` for registering systems → domain plugins register their own systems
- Replace struct plugin pattern → function plugin pattern
- Update "Relevant files" sections with correct paths

#### Ticket 3: Unit Spawning
**File**: `thoughts/shared/tickets/2026-02-08-0003-unit-spawning.md`

Changes:
- "Relevant files" section: Replace `src/components/mod.rs` → new `src/units/mod.rs` (or `src/units.rs`)
- Remove "New plugin: building production system" → production system lives in `src/units/` or `src/building/`
- Remove `src/screens/in_game.rs` — domain plugins register their own systems
- Architecture Notes: Remove note about re-creating `src/components/mod.rs` — confirmed: domain co-location is the pattern
- Architecture Notes: Keep note about `GameState` location but update: states now live in `screens/mod.rs`, `Team` enum should live in `src/units/mod.rs` (or wherever the team concept is managed)
- Production systems use `.in_set(GameSet::Production)`

#### Ticket 4: Movement & AI
**File**: `thoughts/shared/tickets/2026-02-08-0004-movement-and-ai.md`

Changes:
- "Relevant files": Replace `src/components/mod.rs` → `src/units/mod.rs` for Target component
- "Relevant files": Replace `src/screens/in_game.rs` → systems register in `src/units/` plugin
- New systems `unit_find_target`, `unit_movement` → live in `src/units/ai.rs` or `src/units/movement.rs`
- **Debug enemy spawner** → lives in `src/dev_tools/` (feature-gated), not in `src/screens/in_game.rs`
- Movement/AI systems use `.in_set(GameSet::Ai)` and `.in_set(GameSet::Movement)`

#### Ticket 5: Combat
**File**: `thoughts/shared/tickets/2026-02-08-0005-combat.md`

Changes:
- "Relevant files": Replace `src/components/mod.rs` → `src/combat/mod.rs` or `src/units/mod.rs`
- "Relevant files": Replace `src/screens/in_game.rs` → `src/combat/` plugin
- New systems → live in `src/combat/` domain module
- Combat systems use `.in_set(GameSet::Combat)`, death systems use `.in_set(GameSet::Death)`
- Health bar rendering uses `.in_set(GameSet::Ui)`

#### Ticket 6: Economy
**File**: `thoughts/shared/tickets/2026-02-08-0006-economy.md`

Changes:
- "Relevant files": Replace `src/resources/mod.rs` → `src/economy/mod.rs` (Gold resource lives there)
- "Relevant files": Replace `src/components/mod.rs` → `src/economy/mod.rs` for GoldReward
- "Relevant files": Replace `src/screens/in_game.rs` → HUD lives in `src/economy/` or `src/screens/in_game.rs` (HUD is screen UI, but gold logic is economy domain)
- Building selector UI → may stay in `src/screens/in_game.rs` or move to `src/building/`

#### Ticket 7: Wave System
**File**: `thoughts/shared/tickets/2026-02-08-0007-wave-system.md`

Changes:
- "Relevant files": Replace `src/screens/in_game.rs` → `src/waves/mod.rs` (new domain module)
- Wave UI → screen UI in `src/screens/in_game.rs`, wave logic in `src/waves/`
- Remove debug spawner → handled in `src/units/` or `src/waves/`

#### Ticket 8: Fortresses as Damageable Entities
**File**: `thoughts/shared/tickets/2026-02-08-0008-fortresses-damageable.md`

Changes:
- "Relevant files": Replace `src/screens/in_game.rs` → fortress upgrade happens in `src/battlefield/` (entities already exist there)
- Replace `src/components/mod.rs` → Fortress marker already in `src/battlefield/mod.rs`, Health component in `src/combat/` or `src/units/`
- Fortress health bars → reuse health bar system from `src/combat/`

#### Ticket 9: Victory/Defeat & Game Loop
**File**: `thoughts/shared/tickets/2026-02-08-0009-victory-defeat-game-loop.md`

Changes:
- "Relevant files": Replace `src/lib.rs` for states → Victory/Defeat states should be `InGameState` variants (e.g., `InGameState::Victory`, `InGameState::Defeat`) or new sub-states, defined in `src/screens/in_game.rs`
- Replace `src/systems/cleanup.rs` → uses `DespawnOnExit` + `#[states(scoped_entities)]`, no custom cleanup system needed
- Detection systems → live in appropriate domain plugins, state transitions happen via `NextState`

### Success Criteria:

#### Automated Verification:
- [x] All ticket files are valid markdown
- [x] No references to `src/components/mod.rs` or `src/resources/mod.rs` remain (in tickets 3-9)

#### Manual Verification:
- [ ] Ticket file paths match the actual (or planned) module structure
- [ ] Architecture notes are consistent with the new conventions

---

## Testing Strategy

### Existing Tests
All existing tests are preserved with updated imports:
- `crate::GameState` → `crate::screens::GameState`
- `crate::InGameState` → `crate::screens::InGameState`
- `BattlefieldPlugin` → `plugin` (in tests inside the module)
- `BuildingPlugin` → `plugin` (in tests inside the module)
- Building integration tests move from `building.rs` to `building/placement.rs` (co-located with placement systems)

### New Tests
- Theme widgets: unit tests for `widget::header()`, `widget::label()`, `widget::overlay()` verifying they produce the correct bundle types
- Entity counting helpers: tested implicitly through refactored integration tests
- `GameSet` configuration: integration test verifying all sets are configured and chained

### Test Refactoring (Phase 3)
- All verbose `query_filtered → iter → count → assert_eq` patterns replaced with `assert_entity_count::<Filter>(&mut app, N)`
- Test app creation uses shared helpers consistently
- No test logic changes — same assertions, cleaner syntax

### Coverage Impact
No coverage regression expected. Theme tests and `GameSet` test may slightly increase coverage. The refactoring is structural, not behavioral.

## Performance Considerations

None. Function plugins are zero-cost compared to struct plugins — they compile to the same code. Visibility changes are compile-time only.

## Migration Notes

- `prelude.rs` is deleted — any external code importing `auto_battle::prelude::*` must update
- States move to `screens/` — any external code importing `auto_battle::GameState` must use `auto_battle::screens::GameState`
- Plugin structs are gone — `BattlefieldPlugin`, `BuildingPlugin`, etc. no longer exist as types
- `auto_battle::plugin` is the new single entry point for adding all game plugins
- `building.rs` → `building/mod.rs` + `building/placement.rs` — imports from `auto_battle::building::*` still work
- `dev` feature is default-enabled — `cargo build --no-default-features` for release-like builds
- All Update systems should use `.in_set(GameSet::Xxx)` for correct ordering

## References

- Guidelines source: `thoughts/shared/research/2026-02-10-foxtrot-analysis-project-guidelines.md`
- Current tickets: `thoughts/shared/tickets/2026-02-08-*.md`
- Research doc: `thoughts/shared/research/2026-02-04-tano-style-game-research.md`

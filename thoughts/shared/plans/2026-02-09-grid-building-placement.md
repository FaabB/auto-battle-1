# Grid & Building Placement — Implementation Plan

## Overview

Implement the building zone grid visual, mouse-to-grid coordinate conversion, hover highlighting, and click-to-place building mechanics. This is Ticket 2, building on the battlefield layout from Ticket 1.

## Verified API Patterns (Bevy 0.18)

Verified against actual crate source in `~/.cargo/registry/src/`:

- **`Camera::viewport_to_world_2d(&self, &GlobalTransform, Vec2) -> Result<Vec2, ViewportConversionError>`** — converts screen cursor position to 2D world coordinates. Must query `&Camera` (not `&Camera2d`) and `&GlobalTransform`.
- **`Window::cursor_position() -> Option<Vec2>`** — returns cursor position in logical pixels (window coords), `None` if cursor is outside window.
- **`ButtonInput<MouseButton>`** — resource for mouse input. `mouse.just_pressed(MouseButton::Left)` for left click.
- **`MouseButton::Left`** — enum variant for left mouse button.
- **Z-ordering**: `Transform.translation.z` — higher values render in front.
- **`Sprite::from_color(color, size)`** — convenience constructor for colored rectangles.
- **`Single<(&Camera, &GlobalTransform), With<Camera2d>>`** — system parameter for the camera entity.

## Current State Analysis

- 60 `BuildSlot` entities exist (10 rows x 6 cols) — data-only, no visual sprite (`src/battlefield/renderer.rs:94-106`)
- Grid helpers exist: `col_to_world_x()`, `row_to_world_y()` (`src/battlefield/mod.rs:97-106`)
- Zone constants: `BUILD_ZONE_START_COL=2`, `BUILD_ZONE_COLS=6`, `BATTLEFIELD_ROWS=10`, `CELL_SIZE=64.0`
- Z-values are hardcoded: background at `-1.0`, zones at `0.0`, BuildSlots at `0.0`
- No mouse interaction exists anywhere in the codebase
- Camera panning already uses `Single<&Window>` and `Single<&mut Transform, With<Camera2d>>` (`src/battlefield/camera.rs:29-57`)

### Key Discoveries:
- `BuildSlot.col` is local (0-5), not global — matches what `world_to_build_grid()` should return
- BuildSlots already have `Transform` and `DespawnOnExit` — adding `Sprite` to them is safe (Bevy's `#[require]` preserves existing components)
- `viewport_to_world_2d` automatically accounts for camera panning via `GlobalTransform`
- Existing test `spawn_battlefield_creates_five_sprites` will break when BuildSlots get sprites (5 → 65)

## Desired End State

After this plan is complete:

- A visible grid of outlined squares covers the build zone (6 cols x 10 rows)
- Moving the mouse over the grid highlights the cell under the cursor
- Left-clicking an empty cell places a dark blue Barracks building
- Left-clicking an occupied cell does nothing (no duplicate placement)
- `Building` component and `BuildingType` enum exist for Barracks and Farm
- Z-layer ordering constants are defined for the entire game
- All new entities use `DespawnOnExit(GameState::InGame)`
- Pausing and resuming preserves all placed buildings

### Verification:
- `make check` passes (clippy + tests)
- Running the game: grid lines visible in build zone, hover highlight tracks cursor
- Click empty cells → blue building squares appear
- Click occupied cells → nothing happens
- ESC pause/resume → buildings persist

## What We're NOT Doing

- Building type selector UI (Ticket 6 — placement is hardcoded to Barracks)
- Building costs / gold (Ticket 6)
- Unit spawning from buildings (Ticket 3)
- Farm mechanics (Ticket 6)
- Any non-building-zone interaction

## Forward-Looking Architecture

| Ticket | What it needs from this foundation |
|--------|------------------------------------|
| 3 (Unit Spawning) | Queries `Building` entities with `BuildingType::Barracks` to add production timers, spawns units at building position |
| 6 (Economy) | Adds cost checks to placement, building type selector UI changes which `BuildingType` is placed, queries `BuildingType::Farm` for passive income |
| 8 (Fortress HP) | Uses z-layer constants for health bar rendering |

**Key design decisions:**
- `Building` component stores `BuildingType`, `grid_col`, `grid_row` — Ticket 3 needs the type, Ticket 6 queries by type
- Building entities are separate from `BuildSlot` entities — slots get `Occupied` marker, buildings are independent entities with their own sprites
- `HoveredCell` resource shares cursor state — future tickets (6: building selector) can read it without recalculating
- `world_to_build_grid()` is a `pub` helper — Ticket 6's selector UI may also need it

## Implementation Approach

The building module is a new domain plugin (`BuildingPlugin`) that adds grid interaction to the existing `BuildSlot` entities spawned by `BattlefieldPlugin`. Separation of concerns:
- **`battlefield/`** owns the world layout (zones, slots, camera)
- **`building.rs`** owns player interaction with the grid (hover, placement, building entities)

Grid cells become visible by adding `Sprite` to the existing `BuildSlot` entities at spawn time in `renderer.rs`. The hover cursor is a semi-transparent overlay entity that tracks the mouse. Buildings are separate entities spawned at the grid cell's world position.

### System Ordering

```
OnEnter(GameState::InGame):
  spawn_battlefield (existing) → spawn_grid_cursor (new)
    [chain — cursor needs battlefield entities to exist for first frame]

Update (InGameState::Playing):
  update_grid_cursor → handle_building_placement
    [chain_ignore_deferred — placement reads HoveredCell written by cursor]
```

---

## Phase 1: Z-Layer Constants & Grid Cell Visuals

### Overview
Define z-ordering constants for the entire game. Add visual sprites to BuildSlot entities to make the grid visible. Update existing hardcoded z-values.

### Changes Required:

#### 1. Add z-layer constants to `src/battlefield/mod.rs`

Add after the zone column range constants (after line 56):

```rust
// === Z-Layer Constants ===
// Used across the codebase for consistent sprite ordering.

/// Background layer (behind everything).
pub const Z_BACKGROUND: f32 = -1.0;
/// Zone sprites (fortresses, build zone, combat zone).
pub const Z_ZONE: f32 = 0.0;
/// Grid cell sprites in the build zone.
pub const Z_GRID: f32 = 1.0;
/// Grid cursor / hover highlight.
pub const Z_GRID_CURSOR: f32 = 2.0;
/// Placed buildings.
pub const Z_BUILDING: f32 = 3.0;
/// Units (future: Ticket 3).
pub const Z_UNIT: f32 = 4.0;
/// Health bars (future: Ticket 5).
pub const Z_HEALTH_BAR: f32 = 5.0;
```

#### 2. Update `src/battlefield/renderer.rs` — use z-layer constants

Import the new constants and use them instead of hardcoded values.

Add to imports:
```rust
use super::{Z_BACKGROUND, Z_GRID, Z_ZONE};
```

Update spawn_battlefield:
- Background: `Transform::from_xyz(..., ..., Z_BACKGROUND)` (was `-1.0`)
- Player fortress: `Transform::from_xyz(..., ..., Z_ZONE)` (was `0.0`)
- Build zone: `Transform::from_xyz(..., ..., Z_ZONE)` (was `0.0`)
- Combat zone: `Transform::from_xyz(..., ..., Z_ZONE)` (was `0.0`)
- Enemy fortress: `Transform::from_xyz(..., ..., Z_ZONE)` (was `0.0`)

#### 3. Update `src/battlefield/renderer.rs` — add sprites to BuildSlots

Add grid cell color constant:
```rust
const GRID_CELL_COLOR: Color = Color::srgb(0.3, 0.3, 0.4);
```

Update the BuildSlot spawn loop (lines 94-106) to include a sprite:
```rust
// Build slots: 10 rows x 6 cols — visible grid cells
for row in 0..BATTLEFIELD_ROWS {
    for col in 0..BUILD_ZONE_COLS {
        commands.spawn((
            BuildSlot { row, col },
            Sprite::from_color(GRID_CELL_COLOR, Vec2::splat(CELL_SIZE - 2.0)),
            Transform::from_xyz(
                col_to_world_x(BUILD_ZONE_START_COL + col),
                row_to_world_y(row),
                Z_GRID,
            ),
            DespawnOnExit(GameState::InGame),
        ));
    }
}
```

The sprite is `CELL_SIZE - 2.0` = 62px, creating a 1px gap on each side. The build zone background shows through the gaps, forming grid lines.

#### 4. Update existing tests in `src/battlefield/mod.rs`

Two tests need count updates:

`spawn_battlefield_creates_five_sprites` — rename and update:
```rust
#[test]
fn spawn_battlefield_creates_expected_sprites() {
    let mut app = create_battlefield_test_app();
    let sprite_count = app
        .world_mut()
        .query_filtered::<(), With<Sprite>>()
        .iter(app.world())
        .count();
    assert_eq!(sprite_count, 65); // 5 zones + 60 grid cells
}
```

`all_battlefield_entities_have_despawn_on_exit` — update count:
```rust
#[test]
fn all_battlefield_entities_have_despawn_on_exit() {
    let mut app = create_battlefield_test_app();
    let with_despawn = app
        .world_mut()
        .query_filtered::<(), (With<Sprite>, With<DespawnOnExit<GameState>>)>()
        .iter(app.world())
        .count();
    assert_eq!(with_despawn, 65); // All 5 zones + 60 grid cells have DespawnOnExit
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] Existing tests updated and passing (65 sprites instead of 5)
- [x] Z-layer constants defined and consistent

#### Manual Verification:
- [ ] Grid is visible in the building zone — outlined squares in a 6x10 pattern
- [ ] Grid cells are slightly lighter than the build zone background
- [ ] Grid lines (gaps between cells) are clearly visible
- [ ] No z-fighting between grid cells and zone backgrounds

**Implementation Note**: After this phase, pause for manual confirmation that the grid looks good.

---

## Phase 2: Building Components & Plugin Setup

### Overview
Create the building module with components, helper functions, and plugin registration. No systems yet — just the data model.

### Changes Required:

#### 1. New file: `src/building.rs`

```rust
//! Building placement: grid cursor, hover highlight, and click-to-place buildings.

#![allow(clippy::cast_precision_loss)] // Grid math with small u32 values.
#![allow(clippy::cast_possible_truncation)] // f32→u32 grid coords are in range.
#![allow(clippy::cast_sign_loss)] // Grid coords are non-negative.

use bevy::prelude::*;

use crate::battlefield::{
    BATTLEFIELD_ROWS, BUILD_ZONE_COLS, BUILD_ZONE_START_COL, CELL_SIZE, Z_BUILDING, Z_GRID_CURSOR,
};
use crate::{GameState, InGameState};

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

/// A placed building on the grid.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Building {
    pub building_type: BuildingType,
    /// Local grid column (0–5).
    pub grid_col: u32,
    /// Grid row (0–9).
    pub grid_row: u32,
}

/// Types of buildings the player can place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum BuildingType {
    Barracks,
    Farm,
}

/// Marker: this `BuildSlot` has a building on it.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Occupied;

/// Marker for the grid cursor (hover highlight) entity.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct GridCursor;

/// Tracks which build-zone cell the mouse is currently over.
#[derive(Resource, Default, Debug)]
pub struct HoveredCell(pub Option<(u32, u32)>);

// === Helper Functions ===

/// Convert a world position to build-zone grid coordinates.
///
/// Returns `Some((local_col, row))` if the position is inside the build zone,
/// where `local_col` is 0–5 and `row` is 0–9. Returns `None` otherwise.
#[must_use]
pub fn world_to_build_grid(world_pos: Vec2) -> Option<(u32, u32)> {
    let build_start_x = BUILD_ZONE_START_COL as f32 * CELL_SIZE;
    let build_end_x = (BUILD_ZONE_START_COL + BUILD_ZONE_COLS) as f32 * CELL_SIZE;
    let build_end_y = BATTLEFIELD_ROWS as f32 * CELL_SIZE;

    if world_pos.x < build_start_x
        || world_pos.x >= build_end_x
        || world_pos.y < 0.0
        || world_pos.y >= build_end_y
    {
        return None;
    }

    let col = ((world_pos.x - build_start_x) / CELL_SIZE) as u32;
    let row = (world_pos.y / CELL_SIZE) as u32;
    Some((col, row))
}

/// Get the color for a building type.
#[must_use]
pub const fn building_color(building_type: BuildingType) -> Color {
    match building_type {
        BuildingType::Barracks => BARRACKS_COLOR,
        BuildingType::Farm => FARM_COLOR,
    }
}

// === Plugin ===

/// Plugin for building placement: grid cursor, hover, and click-to-place.
#[derive(Debug)]
pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Building>()
            .register_type::<Occupied>()
            .register_type::<GridCursor>()
            .init_resource::<HoveredCell>();

        app.add_systems(
            OnEnter(GameState::InGame),
            spawn_grid_cursor,
        )
        .add_systems(
            Update,
            (update_grid_cursor, handle_building_placement)
                .chain_ignore_deferred()
                .run_if(in_state(InGameState::Playing)),
        );
    }
}

// Systems defined in Phase 3 and 4 below.
```

#### 2. Register module in `src/lib.rs`

Add `pub mod building;` after `pub mod battlefield;` (line 3):
```rust
pub mod battlefield;
pub mod building;
```

#### 3. Register plugin in `src/main.rs`

Add import and plugin:
```rust
use auto_battle::building::BuildingPlugin;

// In the plugins tuple:
.add_plugins((
    GamePlugin,
    LoadingScreenPlugin,
    MainMenuPlugin,
    InGamePlugin,
    BattlefieldPlugin,
    BuildingPlugin,
))
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (module compiles, no warnings)
- [x] Components have correct derives (Debug, Reflect, etc.)
- [x] Types registered with `register_type::<T>()`

#### Manual Verification:
- [x] N/A (no visual output yet — systems are stubs)

---

## Phase 3: Grid Cursor & Hover Highlight

### Overview
Spawn a semi-transparent highlight entity that follows the mouse cursor over the build-zone grid. Convert screen coordinates to world coordinates to grid coordinates.

### Changes Required:

#### 1. `spawn_grid_cursor` system in `src/building.rs`

```rust
/// Spawns the semi-transparent grid cursor entity. Hidden by default.
fn spawn_grid_cursor(mut commands: Commands) {
    commands.spawn((
        GridCursor,
        Sprite::from_color(GRID_CURSOR_COLOR, Vec2::splat(CELL_SIZE - 2.0)),
        Transform::from_xyz(0.0, 0.0, Z_GRID_CURSOR),
        Visibility::Hidden,
        DespawnOnExit(GameState::InGame),
    ));
}
```

#### 2. `update_grid_cursor` system in `src/building.rs`

```rust
/// Moves the grid cursor to the cell under the mouse. Hides it when off-grid.
fn update_grid_cursor(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut cursor: Single<(&mut Transform, &mut Visibility), With<GridCursor>>,
    mut hovered: ResMut<HoveredCell>,
) {
    let (cursor_transform, cursor_visibility) = &mut *cursor;
    let (camera, camera_global) = *camera;

    // Try to convert screen cursor → world position → grid cell
    let grid_cell = window
        .cursor_position()
        .and_then(|screen_pos| camera.viewport_to_world_2d(camera_global, screen_pos).ok())
        .and_then(world_to_build_grid);

    hovered.0 = grid_cell;

    if let Some((col, row)) = grid_cell {
        // Position cursor sprite at the hovered cell
        let world_x = crate::battlefield::col_to_world_x(BUILD_ZONE_START_COL + col);
        let world_y = crate::battlefield::row_to_world_y(row);
        cursor_transform.translation.x = world_x;
        cursor_transform.translation.y = world_y;
        **cursor_visibility = Visibility::Inherited;
    } else {
        **cursor_visibility = Visibility::Hidden;
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes

#### Manual Verification:
- [ ] Hover over grid cells → semi-transparent white highlight appears on the cell
- [ ] Move cursor off the grid → highlight disappears
- [ ] Pan camera with A/D → highlight still tracks the correct cell
- [ ] Highlight snaps to cell centers (no sub-cell tracking)

**Implementation Note**: Pause here for manual confirmation that hover feels correct.

---

## Phase 4: Building Placement

### Overview
Click to place a Barracks building on an empty grid cell. Validate that the cell is not already occupied.

### Changes Required:

#### 1. `handle_building_placement` system in `src/building.rs`

```rust
use crate::battlefield::BuildSlot;

/// Places a building when the player left-clicks an empty grid cell.
fn handle_building_placement(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredCell>,
    mut slots: Query<(Entity, &BuildSlot), Without<Occupied>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Some((col, row)) = hovered.0 else {
        return;
    };

    // Find the empty BuildSlot matching the hovered cell
    let Some((slot_entity, _)) = slots
        .iter_mut()
        .find(|(_, slot)| slot.col == col && slot.row == row)
    else {
        return; // Cell is occupied (filtered by Without<Occupied>)
    };

    // Mark slot as occupied
    commands.entity(slot_entity).insert(Occupied);

    // Spawn the building entity
    let building_type = BuildingType::Barracks; // Hardcoded for now (Ticket 6 adds selector)
    let world_x = crate::battlefield::col_to_world_x(BUILD_ZONE_START_COL + col);
    let world_y = crate::battlefield::row_to_world_y(row);

    commands.spawn((
        Building {
            building_type,
            grid_col: col,
            grid_row: row,
        },
        Sprite::from_color(building_color(building_type), Vec2::splat(BUILDING_SPRITE_SIZE)),
        Transform::from_xyz(world_x, world_y, Z_BUILDING),
        DespawnOnExit(GameState::InGame),
    ));
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes

#### Manual Verification:
- [ ] Click an empty grid cell → dark blue square appears (Barracks)
- [ ] Click another empty cell → another building appears
- [ ] Click an already-occupied cell → nothing happens (no duplicate)
- [ ] Building is visually on top of the grid cell (higher z-layer)
- [ ] ESC pause → ESC resume → buildings persist
- [ ] Pan camera → click cells at different scroll positions → works correctly
- [ ] Place buildings across multiple rows and columns → all render correctly

**Implementation Note**: Pause here for manual testing before writing tests.

---

## Phase 5: Tests

### Overview
Add unit tests for helper functions and integration tests for the building placement flow. Target 90% coverage.

### Changes Required:

#### 1. Unit tests in `src/building.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // --- world_to_build_grid tests ---

    #[test]
    fn world_to_build_grid_first_cell() {
        // First cell: col 0, row 0 — world position just inside (128.0, 0.0)
        let result = world_to_build_grid(Vec2::new(128.5, 0.5));
        assert_eq!(result, Some((0, 0)));
    }

    #[test]
    fn world_to_build_grid_last_cell() {
        // Last cell: col 5, row 9 — world position inside (511.x, 639.x)
        let result = world_to_build_grid(Vec2::new(500.0, 630.0));
        assert_eq!(result, Some((5, 9)));
    }

    #[test]
    fn world_to_build_grid_center_of_cell() {
        // Center of cell (2, 3) — world position = col_to_world_x(2+2), row_to_world_y(3)
        let x = (BUILD_ZONE_START_COL + 2) as f32 * CELL_SIZE + CELL_SIZE / 2.0;
        let y = 3.0 * CELL_SIZE + CELL_SIZE / 2.0;
        let result = world_to_build_grid(Vec2::new(x, y));
        assert_eq!(result, Some((2, 3)));
    }

    #[test]
    fn world_to_build_grid_outside_left() {
        // Before build zone (x < 128.0)
        assert_eq!(world_to_build_grid(Vec2::new(100.0, 100.0)), None);
    }

    #[test]
    fn world_to_build_grid_outside_right() {
        // After build zone (x >= 512.0)
        assert_eq!(world_to_build_grid(Vec2::new(512.0, 100.0)), None);
    }

    #[test]
    fn world_to_build_grid_outside_top() {
        // Above battlefield (y >= 640.0)
        assert_eq!(world_to_build_grid(Vec2::new(200.0, 640.0)), None);
    }

    #[test]
    fn world_to_build_grid_outside_bottom() {
        // Below battlefield (y < 0.0)
        assert_eq!(world_to_build_grid(Vec2::new(200.0, -1.0)), None);
    }

    #[test]
    fn world_to_build_grid_boundary_left_edge() {
        // Exactly at left boundary (128.0 = first valid x)
        assert_eq!(world_to_build_grid(Vec2::new(128.0, 32.0)), Some((0, 0)));
    }

    #[test]
    fn world_to_build_grid_boundary_right_edge() {
        // Exactly at right boundary (512.0 = first invalid x)
        assert_eq!(world_to_build_grid(Vec2::new(512.0, 32.0)), None);
    }

    // --- building_color tests ---

    #[test]
    fn barracks_color_is_blue() {
        let color = building_color(BuildingType::Barracks);
        assert_eq!(color, BARRACKS_COLOR);
    }

    #[test]
    fn farm_color_is_green() {
        let color = building_color(BuildingType::Farm);
        assert_eq!(color, FARM_COLOR);
    }
}
```

#### 2. Integration tests in `src/building.rs`

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::battlefield::{BattlefieldPlugin, BuildSlot};
    use pretty_assertions::assert_eq;

    /// Helper: app with BattlefieldPlugin + BuildingPlugin, transitioned to InGame.
    fn create_building_test_app() -> App {
        let mut app = crate::testing::create_test_app();
        app.add_plugins(bevy::state::app::StatesPlugin);
        app.add_plugins(bevy::input::InputPlugin);
        app.add_plugins(bevy::window::WindowPlugin::default());
        app.init_state::<crate::GameState>();
        app.add_sub_state::<crate::InGameState>();
        app.add_plugins(BattlefieldPlugin);
        app.add_plugins(BuildingPlugin);
        app.world_mut().spawn(Camera2d);
        app.world_mut()
            .resource_mut::<NextState<crate::GameState>>()
            .set(crate::GameState::InGame);
        app.update(); // Apply transition + OnEnter
        app.update(); // Apply deferred commands
        app
    }

    #[test]
    fn grid_cursor_spawned() {
        let mut app = create_building_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), With<GridCursor>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn grid_cursor_starts_hidden() {
        let mut app = create_building_test_app();
        let mut query = app.world_mut().query_filtered::<&Visibility, With<GridCursor>>();
        let visibility = query.single(app.world()).unwrap();
        assert_eq!(*visibility, Visibility::Hidden);
    }

    #[test]
    fn hovered_cell_resource_initialized() {
        let app = create_building_test_app();
        let hovered = app.world().resource::<HoveredCell>();
        assert!(hovered.0.is_none());
    }

    #[test]
    fn no_buildings_at_start() {
        let mut app = create_building_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), With<Building>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn no_occupied_slots_at_start() {
        let mut app = create_building_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), (With<BuildSlot>, With<Occupied>)>()
            .iter(app.world())
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn grid_cursor_has_despawn_on_exit() {
        let mut app = create_building_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), (With<GridCursor>, With<DespawnOnExit<GameState>>)>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }
}
```

#### 3. Add z-layer constant tests in `src/battlefield/mod.rs`

Add to the existing `tests` module:

```rust
#[test]
fn z_layers_are_ordered() {
    assert!(Z_BACKGROUND < Z_ZONE);
    assert!(Z_ZONE < Z_GRID);
    assert!(Z_GRID < Z_GRID_CURSOR);
    assert!(Z_GRID_CURSOR < Z_BUILDING);
    assert!(Z_BUILDING < Z_UNIT);
    assert!(Z_UNIT < Z_HEALTH_BAR);
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (all tests green, no clippy warnings)
- [x] All new unit tests pass (world_to_build_grid edge cases)
- [x] All new integration tests pass (cursor spawning, initial state)
- [x] Existing battlefield tests updated and passing

#### Manual Verification:
- [ ] Full gameplay test: start game, hover grid, place multiple buildings, pause/resume, return to menu

---

## Testing Strategy

### Unit Tests (in `src/building.rs`):
- `world_to_build_grid`: 9 tests covering center, boundaries, outside all 4 edges
- `building_color`: 2 tests mapping type → color

### Integration Tests (in `src/building.rs`):
- Grid cursor spawned, starts hidden, has DespawnOnExit
- No buildings/occupied slots at start
- HoveredCell resource initialized to None

### Updated Battlefield Tests (in `src/battlefield/mod.rs`):
- Sprite count updated from 5 → 65
- DespawnOnExit count updated from 5 → 65
- New z-layer ordering test

### Coverage Analysis

| Module | Lines (approx) | Test approach | Expected coverage |
|--------|:-:|---|:-:|
| `building.rs` (components + helpers) | ~50 | Unit tests | ~100% |
| `building.rs` (spawn_grid_cursor) | ~10 | Integration tests | ~100% |
| `building.rs` (update_grid_cursor) | ~20 | Integration (initial state) + manual | ~60% |
| `building.rs` (handle_building_placement) | ~25 | Integration (initial state) + manual | ~50% |
| `battlefield/mod.rs` (z constants) | ~10 | Unit test | ~100% |
| `battlefield/renderer.rs` (grid cells) | ~10 | Integration (existing + updated counts) | ~95% |
| **Overall project** | | | **~85-90%** |

**Note**: `update_grid_cursor` and `handle_building_placement` are hard to test via integration tests because they require simulating mouse input and camera projection in a headless app. The helper functions they use (`world_to_build_grid`, `building_color`) are fully tested. Manual testing covers the systems end-to-end. Coverage for these specific systems will improve when/if we add simulated input helpers.

## File Summary

| File | Action |
|------|--------|
| `src/battlefield/mod.rs` | **Edit** — add z-layer constants, add z-layer test |
| `src/battlefield/renderer.rs` | **Edit** — add grid cell sprite to BuildSlots, use z-layer constants, add grid cell color constant |
| `src/building.rs` | **New** — BuildingPlugin, components (Building, BuildingType, Occupied, GridCursor), resource (HoveredCell), helpers, 3 systems, tests |
| `src/lib.rs` | **Edit** — add `pub mod building;` |
| `src/main.rs` | **Edit** — import and register `BuildingPlugin` |

## Implementation Brief for Agents

### Key API Patterns
- `Camera::viewport_to_world_2d(&self, &GlobalTransform, Vec2) -> Result<Vec2>` for cursor conversion
- `window.cursor_position() -> Option<Vec2>` for screen-space cursor
- `ButtonInput<MouseButton>` with `MouseButton::Left` for click detection
- `Single<(&Camera, &GlobalTransform), With<Camera2d>>` for camera query
- `Without<Occupied>` filter on BuildSlot query for placement validation

### Clippy Notes
- `#![allow(clippy::cast_precision_loss)]` at module level for grid math (u32→f32)
- `#![allow(clippy::cast_possible_truncation)]` for f32→u32 grid coords
- `#![allow(clippy::cast_sign_loss)]` for f32→u32 (non-negative grid coords)
- `#[must_use]` on all helper functions
- `chain_ignore_deferred()` not `chain()` — no commands between cursor and placement systems

### What Future Tickets Need
- **`Building` component with `BuildingType`** — Ticket 3 queries Barracks for production, Ticket 6 queries by type
- **`Occupied` marker on BuildSlot** — Ticket 6 adds cost validation before placement
- **`HoveredCell` resource** — Ticket 6's building selector shows info about hovered cell
- **`world_to_build_grid()` helper** — reusable for any cursor→grid conversion
- **Z-layer constants** — all future tickets use these for consistent ordering

## References

- Original ticket: `thoughts/shared/tickets/2026-02-08-0002-grid-building-placement.md`
- Ticket 1 plan: `thoughts/shared/plans/2026-02-08-camera-battlefield-layout.md`
- Research: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.2 Building System)

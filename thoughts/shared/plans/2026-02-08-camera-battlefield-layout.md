# Camera & Battlefield Layout — Implementation Plan

## Overview

Implement the foundational battlefield coordinate system with zone layout, fortress placeholders, and horizontal camera panning. This is Ticket 1, the foundation all subsequent tickets build on.

## Current State Analysis

- Camera: bare `Camera2d` spawned at startup (`src/main.rs:38`), no transform, no panning
- InGame screen: spawns placeholder text only (`src/screens/in_game.rs:23-39`)
- No coordinate system, no sprites, no world-space entities exist
- Window: 1920x1080, `ImagePlugin::default_nearest()` for pixel-perfect rendering
- Bevy 0.18 with `"2d"` feature only

### Key Discoveries:
- `CleanupInGame` component exists for entity lifecycle (`src/components/cleanup.rs:14-15`)
- Generic `cleanup_entities::<CleanupInGame>` runs on `OnExit(GameState::InGame)` (`src/screens/in_game.rs:19`)
- Paused state issue (destroys InGame entities) is known but NOT fixed in this ticket

## Desired End State

After this plan is complete:

- A `battlefield` module exists with coordinate constants for all zones
- Entering InGame spawns the full battlefield layout as colored `Sprite` entities:
  - Blue fortress (2 cols, far left)
  - Building zone background (6 cols, distinct color)
  - Combat zone (72 cols, open area)
  - Red fortress (2 cols, far right)
- Camera starts centered on the player's building zone
- A/D or Left/Right arrow keys pan the camera horizontally
- Camera clamps at battlefield boundaries (can't pan into void)
- All battlefield entities have `CleanupInGame` so they despawn on state exit

### Verification:
- `make check` passes (clippy + tests)
- Running the game and pressing SPACE shows the battlefield layout
- Panning with A/D or arrow keys works and stops at boundaries
- Both fortress rectangles visible at their respective ends

## What We're NOT Doing

- Grid lines / cell outlines (Ticket 2)
- Building placement (Ticket 2)
- Sub-state fix for Paused (noted, deferred to Ticket 2+)
- Zoom controls
- Mouse-based panning
- Any gameplay systems

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

---

## Phase 1: Battlefield Constants & Module Setup

### Overview
Create a `battlefield` module with all coordinate constants and zone definitions. Create a `BattlefieldPlugin` that will be registered from `InGamePlugin`.

### Changes Required:

#### 1. New file: `src/battlefield.rs`

Define all battlefield constants and a plugin struct.

```rust
//! Battlefield layout constants and systems.

use bevy::prelude::*;

use crate::GameState;
use crate::components::CleanupInGame;
use crate::systems::cleanup_entities;

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
```

#### 2. Register module in `src/lib.rs`

Add `pub mod battlefield;` to the module list.

#### 3. New plugin: `BattlefieldPlugin` (in `src/battlefield.rs`)

```rust
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
```

#### 4. Register `BattlefieldPlugin` in `src/screens/in_game.rs`

Add `BattlefieldPlugin` to the InGamePlugin's build method or register it in `main.rs` alongside the other plugins. Since it's game-logic related to InGame, it makes sense to add it to main.rs plugin list.

Update `src/main.rs` to add `BattlefieldPlugin` to the game plugins tuple.

### Success Criteria:

#### Automated Verification:
- [x] `cargo build` compiles successfully
- [x] `make check` passes (clippy + tests)
- [x] All constants are defined and consistent (TOTAL_COLS = 82, etc.)

#### Manual Verification:
- [x] N/A for this phase (no visual output yet)

---

## Phase 2: Spawn Battlefield Entities

### Overview
Implement `spawn_battlefield` to create colored sprite rectangles for each zone. All entities get `CleanupInGame`.

### Changes Required:

#### 1. `spawn_battlefield` system in `src/battlefield.rs`

Spawn one sprite per zone: player fortress, building zone, combat zone, enemy fortress. Also spawn a background sprite behind everything.

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
        Sprite {
            color: BACKGROUND_COLOR,
            custom_size: Some(Vec2::new(BATTLEFIELD_WIDTH + 128.0, BATTLEFIELD_HEIGHT + 128.0)),
            ..default()
        },
        Transform::from_xyz(BATTLEFIELD_WIDTH / 2.0, BATTLEFIELD_HEIGHT / 2.0, -1.0),
        CleanupInGame,
    ));

    // Player fortress (blue, 2 cols wide, full height)
    spawn_zone_sprite(
        &mut commands,
        PLAYER_FORT_START_COL,
        FORTRESS_COLS,
        PLAYER_FORT_COLOR,
        0.0, // z-order
    );

    // Building zone (dark blue-gray, 6 cols wide, full height)
    spawn_zone_sprite(
        &mut commands,
        BUILD_ZONE_START_COL,
        BUILD_ZONE_COLS,
        BUILD_ZONE_COLOR,
        0.0,
    );

    // Combat zone (dark gray, 72 cols wide, full height)
    spawn_zone_sprite(
        &mut commands,
        COMBAT_ZONE_START_COL,
        COMBAT_ZONE_COLS,
        COMBAT_ZONE_COLOR,
        0.0,
    );

    // Enemy fortress (red, 2 cols wide, full height)
    spawn_zone_sprite(
        &mut commands,
        ENEMY_FORT_START_COL,
        FORTRESS_COLS,
        ENEMY_FORT_COLOR,
        0.0,
    );
}

fn spawn_zone_sprite(
    commands: &mut Commands,
    start_col: u32,
    width_cols: u32,
    color: Color,
    z: f32,
) {
    let width = width_cols as f32 * CELL_SIZE;
    let height = BATTLEFIELD_HEIGHT;
    let x = zone_center_x(start_col, width_cols);
    let y = battlefield_center_y();

    commands.spawn((
        Sprite {
            color,
            custom_size: Some(Vec2::new(width, height)),
            ..default()
        },
        Transform::from_xyz(x, y, z),
        CleanupInGame,
    ));
}
```

#### 2. Remove placeholder text from `setup_game` in `src/screens/in_game.rs`

The "Game Running - Press ESC to Pause" text entity should be removed since the battlefield visuals will replace it. Keep the `handle_game_input` system for ESC→Paused transition.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes

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
Modify the camera to start positioned over the player's building zone. Add a horizontal panning system with keyboard controls and boundary clamping.

### Changes Required:

#### 1. Move camera spawn into the battlefield module

Instead of spawning the camera at `Startup` in `main.rs`, spawn it as part of the battlefield setup on `OnEnter(GameState::InGame)`. This way the camera position is tied to the battlefield layout.

**Actually, better approach:** Keep the camera as a global entity spawned at Startup (it persists across states for menus too), but add a system that repositions it when entering InGame. This avoids re-spawning the camera every time.

Add a `setup_camera_for_battlefield` system that runs on `OnEnter(GameState::InGame)`:

```rust
/// Camera panning speed in pixels per second.
const CAMERA_PAN_SPEED: f32 = 500.0;

fn setup_camera_for_battlefield(
    mut camera_query: Query<(&mut Transform, &mut OrthographicProjection), With<Camera2d>>,
) {
    let Ok((mut transform, mut projection)) = camera_query.single_mut() else {
        return;
    };

    // Position camera centered on the building zone (X), centered on battlefield height (Y)
    let build_zone_center_x = zone_center_x(BUILD_ZONE_START_COL, BUILD_ZONE_COLS);
    transform.translation.x = build_zone_center_x;
    transform.translation.y = BATTLEFIELD_HEIGHT / 2.0;

    // Set projection scaling so the full battlefield height fits the viewport
    // With ScalingMode::FixedVertical, the viewport height in world units equals the given value
    projection.scaling_mode = bevy::render::camera::ScalingMode::FixedVertical {
        viewport_height: BATTLEFIELD_HEIGHT,
    };
}
```

Register this system in `BattlefieldPlugin::build`:
```rust
app.add_systems(OnEnter(GameState::InGame), (spawn_battlefield, setup_camera_for_battlefield));
```

#### 2. Camera panning system in `src/battlefield.rs`

```rust
fn camera_pan(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera_query: Query<&mut Transform, With<Camera2d>>,
    windows: Query<&Window>,
) {
    let Ok(mut transform) = camera_query.single_mut() else {
        return;
    };

    let mut direction = 0.0;
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        direction += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        direction -= 1.0;
    }

    if direction != 0.0 {
        transform.translation.x += direction * CAMERA_PAN_SPEED * time.delta_secs();
    }

    // Clamp camera to battlefield bounds
    // The camera shows BATTLEFIELD_HEIGHT vertically (via FixedVertical scaling).
    // The visible width depends on the window aspect ratio.
    let window = windows.single();
    let aspect_ratio = window.width() / window.height();
    let visible_width = BATTLEFIELD_HEIGHT * aspect_ratio;
    let half_visible = visible_width / 2.0;

    let min_x = half_visible; // Can't see past left edge (x=0)
    let max_x = BATTLEFIELD_WIDTH - half_visible; // Can't see past right edge

    transform.translation.x = transform.translation.x.clamp(min_x, max_x);
}
```

#### 3. Update `BattlefieldPlugin` registration

```rust
impl Plugin for BattlefieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
                OnEnter(GameState::InGame),
                (spawn_battlefield, setup_camera_for_battlefield),
            )
            .add_systems(
                Update,
                camera_pan.run_if(in_state(GameState::InGame)),
            );
    }
}
```

#### 4. Clean up `src/main.rs`

The `setup_camera` function stays as-is (spawning `Camera2d` at Startup). The battlefield module repositions it.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (clippy + tests + fmt)
- [x] No warnings

#### Manual Verification:
- [ ] Camera starts centered on the building zone (blue fortress visible to the left)
- [ ] Full battlefield height fits the viewport (no vertical scrolling needed)
- [ ] Press D or Right Arrow → camera pans right smoothly
- [ ] Press A or Left Arrow → camera pans left smoothly
- [ ] Camera stops at left boundary (can't pan past player fortress)
- [ ] Camera stops at right boundary (can't pan past enemy fortress)
- [ ] Pan all the way right to see the red enemy fortress
- [ ] Pan all the way left to see the blue player fortress

**Implementation Note**: After completing this phase, pause for manual confirmation that panning feels good. Speed can be adjusted.

---

## Testing Strategy

### Unit Tests:
- Test `col_to_world_x` and `row_to_world_y` helper functions
- Test that `TOTAL_COLS` equals expected 82
- Test that `BATTLEFIELD_WIDTH` and `BATTLEFIELD_HEIGHT` are consistent with cell size

### Manual Testing Steps:
1. `cargo run` → press SPACE to start game
2. Verify battlefield layout visible with colored zones
3. Pan left/right with A/D or arrow keys
4. Verify camera clamps at both boundaries
5. Press ESC to pause, then ESC to resume — battlefield still there
6. Press Q from pause to return to main menu — battlefield cleaned up

## Performance Considerations

- Only 5 sprites spawned (background + 4 zones) — negligible
- Camera panning uses `Time::delta_secs()` for frame-rate independence
- `FixedVertical` scaling mode handles window resizing gracefully

## File Summary

| File | Action |
|------|--------|
| `src/battlefield.rs` | **New** — constants, plugin, spawn & camera systems |
| `src/lib.rs` | **Edit** — add `pub mod battlefield;` |
| `src/main.rs` | **Edit** — add `BattlefieldPlugin` to plugins |
| `src/screens/in_game.rs` | **Edit** — remove placeholder text from `setup_game` |

## References

- Original ticket: `thoughts/shared/tickets/2026-02-08-0001-camera-battlefield-layout.md`
- Research doc: `thoughts/shared/research/2026-02-04-tano-style-game-research.md` (Section 2.1, Section 7)

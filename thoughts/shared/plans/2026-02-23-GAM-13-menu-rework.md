# GAM-13: Menu Rework Implementation Plan

## Overview

Rework the entire menu and HUD system: add clickable buttons via the foxtrot widget pattern, fix centering bugs, create a persistent bottom bar with gold/cards/reroll/minimap, restyle all screens (main menu, pause, victory/defeat), add elapsed time display, building production indicators, and vertical camera panning.

## Current State Analysis

### What exists:
- **Main menu** (`menus/main_menu.rs`): "Auto Battle" title + "Press SPACE to Start" label, keyboard-only
- **Pause** (`menus/pause.rs`): overlay + "PAUSED" + "Press ESC to Resume | Q to Quit", keyboard-only
- **Victory/Defeat** (`menus/endgame.rs`): overlay + title + "Press Q to Continue", keyboard-only
- **Shop UI** (`economy/shop_ui.rs`): standalone absolute panel at bottom-center with 4 card slots + reroll
- **Gold HUD** (`economy/ui.rs`): absolute text at top-right
- **Camera** (`battlefield/camera.rs`): X-axis panning only (A/D, Left/Right)
- **Theme**: 4 colors in `palette.rs`, 2 widget constructors (`header`, `label`, `overlay`) in `widget.rs`

### Key bugs:
- All menus use `left: Val::Percent(50.0)` which positions the LEFT EDGE at 50%, not centered
- 22 color constants scattered outside `palette.rs`
- 7 font sizes as magic numbers (14, 16, 24, 28, 32, 64, 72)
- No mouse-clickable buttons anywhere
- No elapsed time, no production indicators, no vertical camera pan

### Key Discoveries:
- Foxtrot button pattern uses `SpawnWith` + `ChildSpawner` + `.observe(action)` (`children!` doesn't support `.observe()`)
- `InteractionPalette` component + `Changed<Interaction>` system (foxtrot approach, simpler than 4 observers)
- `Button` component still needed in Bevy 0.18 (auto-adds `Interaction` + `FocusPolicy::Block`)
- `GlobalZIndex(i32)` for overlay z-ordering above gameplay UI
- `AppExit` is a Message in 0.18 — use `MessageWriter<AppExit>` to quit
- `Time<Virtual>` pauses when `time.pause()` is called — already wired in `menus/mod.rs`
- `FixedVertical` scaling shows full battlefield height — Y pan is no-op at default zoom, but code is needed for future zoom

## Desired End State

After this plan is complete:
- **Main menu**: Centered bordered panel with "Auto Battle" title, "Start Battle" + "Exit Game" clickable buttons (matching mockup)
- **In-game**: Full-width bottom bar with Gold (left) | 4 card slots + Reroll (center) | Elapsed time + Minimap placeholder (right)
- **Pause**: Semi-transparent overlay with centered bordered panel, "Auto Battle" title, "Continue" + "Exit Game" buttons (matching mockup)
- **Victory/Defeat**: Styled overlays with buttons ("Restart" + "Exit to Menu")
- **Theme**: All colors centralized, font size tokens, `button()` + `ui_root()` widgets, `interaction.rs`
- **Camera**: Supports both X and Y panning
- **Buildings**: Show production progress bar below health bar

## What We're NOT Doing

- Functional minimap (just a placeholder box)
- Zoom functionality (Y panning prep only)
- Audio/sound effects
- Custom fonts (using Bevy default)
- Animated transitions between screens
- Gamepad/keyboard navigation for buttons (mouse-only for now)

## Verified API Patterns (Bevy 0.18)

These were verified against actual crate source by the research agents:

- `Button` auto-requires `Node`, `FocusPolicy::Block`, `Interaction` — still needed for clickable UI
- `IntoObserverSystem` — NOT in prelude, import via `use bevy::ecs::system::IntoObserverSystem;`
- `SpawnWith` — in prelude, `use bevy::ecs::spawn::SpawnWith;`
- `ChildSpawner` — in prelude, `use bevy::ecs::hierarchy::ChildSpawner;`
- `Pickable::IGNORE` — in prelude, prevents click events from hitting text children
- `Hovered` — NOT in prelude, `use bevy::picking::hover::Hovered;`
- `Pressed` — NOT in prelude, `use bevy::ui::Pressed;`
- `GlobalZIndex(i32)` — in prelude, escapes hierarchy z-ordering
- `BorderColor` — component, auto-added by `Node` (defaults transparent)
- `Node.border: UiRect` — field on Node struct for border widths
- `Node.border_radius: BorderRadius` — field on Node struct for corner rounding
- `MessageWriter<AppExit>` + `.write(AppExit::Success)` — for quitting the game
- `children![]` macro — in prelude, for simple child spawning (no `.observe()` support)
- `Children::spawn(SpawnWith(...))` — for cases needing `.observe()` on child

---

## Phase 1: Theme Foundation

### Overview
Centralize all colors and font sizes, add `interaction.rs` with hover/press feedback, add `widget::button()` and `widget::ui_root()` constructors.

### Changes Required:

#### 1. Expand `theme/palette.rs`
**File**: `src/theme/palette.rs`
**Changes**: Move all 22 scattered colors here, add button colors, add font size tokens.

```rust
//! Color constants and font size tokens for consistent UI theming.

use bevy::prelude::*;

// === Text Colors ===

pub const HEADER_TEXT: Color = Color::WHITE;
pub const BODY_TEXT: Color = Color::srgb(0.7, 0.7, 0.7);
pub const GOLD_TEXT: Color = Color::srgb(1.0, 0.85, 0.0);
pub const BUTTON_TEXT: Color = Color::srgb(0.925, 0.925, 0.925);

// === UI Backgrounds ===

pub const OVERLAY_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);
pub const PANEL_BACKGROUND: Color = Color::srgba(0.1, 0.1, 0.15, 0.95);
pub const PANEL_BORDER: Color = Color::srgba(0.5, 0.5, 0.6, 0.8);

// === Button Colors ===

pub const BUTTON_BACKGROUND: Color = Color::srgb(0.275, 0.4, 0.75);
pub const BUTTON_HOVERED_BACKGROUND: Color = Color::srgb(0.384, 0.6, 0.82);
pub const BUTTON_PRESSED_BACKGROUND: Color = Color::srgb(0.239, 0.286, 0.6);

// === Bottom Bar ===

pub const BOTTOM_BAR_BACKGROUND: Color = Color::srgba(0.1, 0.1, 0.15, 0.9);

// === Shop Card Colors ===

pub const CARD_BACKGROUND: Color = Color::srgb(0.2, 0.2, 0.3);
pub const CARD_SELECTED: Color = Color::srgb(0.3, 0.5, 0.3);
pub const CARD_EMPTY: Color = Color::srgb(0.15, 0.15, 0.15);
pub const CARD_HOVER: Color = Color::srgb(0.3, 0.3, 0.4);
pub const REROLL_BACKGROUND: Color = Color::srgb(0.4, 0.25, 0.1);

// === Battlefield Colors ===

pub const GRID_CELL: Color = Color::srgb(0.3, 0.3, 0.4);
pub const GRID_CURSOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.2);
pub const PLAYER_FORTRESS: Color = Color::srgb(0.2, 0.3, 0.8);
pub const ENEMY_FORTRESS: Color = Color::srgb(0.8, 0.2, 0.2);
pub const BUILD_ZONE: Color = Color::srgb(0.25, 0.25, 0.35);
pub const COMBAT_ZONE: Color = Color::srgb(0.15, 0.15, 0.2);
pub const BACKGROUND: Color = Color::srgb(0.1, 0.1, 0.12);

// === Entity Colors ===

pub const PLAYER_UNIT: Color = Color::srgb(0.2, 0.8, 0.2);
pub const ENEMY_UNIT: Color = Color::srgb(0.8, 0.2, 0.2);
pub const PROJECTILE: Color = Color::srgb(1.0, 1.0, 0.3);
pub const BARRACKS: Color = Color::srgb(0.15, 0.2, 0.6);
pub const FARM: Color = Color::srgb(0.2, 0.6, 0.1);

// === Health/Progress Bar Colors ===

pub const HEALTH_BAR_BG: Color = Color::srgb(0.8, 0.1, 0.1);
pub const HEALTH_BAR_FILL: Color = Color::srgb(0.1, 0.9, 0.1);
pub const PRODUCTION_BAR_BG: Color = Color::srgb(0.2, 0.2, 0.4);
pub const PRODUCTION_BAR_FILL: Color = Color::srgb(0.3, 0.5, 0.9);

// === Font Size Tokens ===

pub const FONT_SIZE_TITLE: f32 = 72.0;
pub const FONT_SIZE_HEADER: f32 = 64.0;
pub const FONT_SIZE_LABEL: f32 = 32.0;
pub const FONT_SIZE_HUD: f32 = 28.0;
pub const FONT_SIZE_PROMPT: f32 = 24.0;
pub const FONT_SIZE_BODY: f32 = 16.0;
pub const FONT_SIZE_SMALL: f32 = 14.0;
```

Then update all 22 scattered color references across the codebase to use `palette::*` constants:
- `battlefield/renderer.rs` — 6 colors
- `building/mod.rs` — 3 colors (1 const + 2 inline in `building_stats()`)
- `units/mod.rs` — 2 colors
- `combat/attack.rs` — 1 color
- `combat/health_bar.rs` — 2 colors
- `economy/shop_ui.rs` — 6 const colors + 2 inline `Color::WHITE`
- `economy/ui.rs` — 1 font size reference
- `menus/main_menu.rs` — 1 font size
- `menus/pause.rs` — 1 font size
- `menus/endgame.rs` — 1 font size

Also update `widget::header()` and `widget::label()` to use `FONT_SIZE_HEADER` and `FONT_SIZE_LABEL` tokens.

#### 2. Create `theme/interaction.rs`
**File**: `src/theme/interaction.rs` (NEW)
**Changes**: Add `InteractionPalette` component and `apply_interaction_palette` system.

```rust
//! Button hover/press visual feedback.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;

/// Defines colors for none/hovered/pressed button states.
/// Add alongside `Button` and `BackgroundColor` on clickable UI elements.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
#[require(Hovered)]
pub struct InteractionPalette {
    pub none: Color,
    pub hovered: Color,
    pub pressed: Color,
}

fn apply_interaction_palette(
    mut palette_query: Query<
        (Has<Pressed>, &Hovered, &InteractionPalette, &mut BackgroundColor),
        Changed<Interaction>,
    >,
) {
    for (pressed, Hovered(hovered), palette, mut background) in &mut palette_query {
        *background = match (pressed, hovered) {
            (true, _) => palette.pressed,
            (false, true) => palette.hovered,
            (false, false) => palette.none,
        }
        .into();
    }
}

pub fn plugin(app: &mut App) {
    app.register_type::<InteractionPalette>();
    app.add_systems(Update, apply_interaction_palette);
}
```

#### 3. Expand `theme/widget.rs`
**File**: `src/theme/widget.rs`
**Changes**: Add `ui_root()`, `button()`, update existing constructors.

```rust
//! Reusable UI widget constructors.

use bevy::ecs::hierarchy::ChildSpawner;
use bevy::ecs::spawn::SpawnWith;
use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;

use super::interaction::InteractionPalette;
use super::palette;

/// Full-screen flex container that centers its children.
/// Use as root for menus and overlays.
pub fn ui_root(name: impl Into<std::borrow::Cow<'static, str>>) -> impl Bundle {
    (
        Name::new(name),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(20.0),
            ..default()
        },
    )
}

/// Large header text (title size, white).
pub fn header(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: palette::FONT_SIZE_HEADER,
            ..default()
        },
        TextColor(palette::HEADER_TEXT),
    )
}

/// Medium label text (label size, gray).
pub fn label(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: palette::FONT_SIZE_LABEL,
            ..default()
        },
        TextColor(palette::BODY_TEXT),
    )
}

/// Full-screen semi-transparent overlay background.
/// Typically used as `BackgroundColor(palette::OVERLAY_BACKGROUND)` on a `ui_root()` now.
/// Kept for backwards compatibility.
pub fn overlay() -> impl Bundle {
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

/// Clickable button with text and an observer-based action.
/// Uses the foxtrot pattern: outer wrapper + inner Button with InteractionPalette.
pub fn button<E, B, M, I>(text: impl Into<String>, action: I) -> impl Bundle
where
    E: EntityEvent,
    B: Bundle,
    I: IntoObserverSystem<E, B, M>,
{
    let text = text.into();
    let action = IntoObserverSystem::into_system(action);
    (
        Name::new("Button"),
        Node::default(),
        Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
            parent
                .spawn((
                    Name::new("Button Inner"),
                    Button,
                    Node {
                        width: Val::Px(300.0),
                        height: Val::Px(60.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(palette::BUTTON_BACKGROUND),
                    BorderColor(palette::PANEL_BORDER),
                    InteractionPalette {
                        none: palette::BUTTON_BACKGROUND,
                        hovered: palette::BUTTON_HOVERED_BACKGROUND,
                        pressed: palette::BUTTON_PRESSED_BACKGROUND,
                    },
                    children![(
                        Text(text),
                        TextFont::from_font_size(palette::FONT_SIZE_LABEL),
                        TextColor(palette::BUTTON_TEXT),
                        Pickable::IGNORE,
                    )],
                ))
                .observe(action);
        })),
    )
}
```

#### 4. Update `theme/mod.rs`
**File**: `src/theme/mod.rs`
**Changes**: Add `interaction` module, wire up its plugin.

```rust
//! Shared UI theme: color palette, interaction feedback, and reusable widget constructors.

pub mod interaction;
pub mod palette;
pub mod widget;

pub fn plugin(app: &mut bevy::prelude::App) {
    app.add_plugins(interaction::plugin);
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes (clippy + type checking)
- [x] `make test` passes (existing tests still work after color constant renames)
- [x] `make build` succeeds

#### Manual Verification:
- [ ] No visible regressions (colors unchanged despite being moved to palette)

**Implementation Note**: After completing this phase, pause for manual verification before proceeding.

---

## Phase 2: Screen Rework (Main Menu + Loading)

### Overview
Rebuild main menu with bordered panel and clickable buttons. Fix loading screen. All screens use new `ui_root()` + `button()` widgets.

### Changes Required:

#### 1. Rewrite `menus/main_menu.rs`
**File**: `src/menus/main_menu.rs`
**Changes**: Replace keyboard-only menu with bordered panel + clickable buttons.

```rust
//! Main menu UI: bordered panel with title and clickable buttons.

use bevy::prelude::*;

use super::Menu;
use crate::screens::GameState;
use crate::theme::{palette, widget};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Main), spawn_main_menu);
}

fn spawn_main_menu(mut commands: Commands) {
    commands.spawn((
        widget::ui_root("Main Menu Screen"),
        DespawnOnExit(Menu::Main),
        children![
            // Bordered panel
            (
                Name::new("Main Menu Panel"),
                Node {
                    width: Val::Px(500.0),
                    min_height: Val::Px(400.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(Val::Px(40.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(palette::PANEL_BACKGROUND),
                BorderColor(palette::PANEL_BORDER),
                children![
                    // Title
                    (
                        Text::new("Auto Battle"),
                        TextFont::from_font_size(palette::FONT_SIZE_TITLE),
                        TextColor(palette::HEADER_TEXT),
                    ),
                    // Start button
                    widget::button("Start Battle", |_: On<Pointer<Click>>,
                        mut next_game: ResMut<NextState<GameState>>,
                        mut next_menu: ResMut<NextState<Menu>>| {
                        next_game.set(GameState::InGame);
                        next_menu.set(Menu::None);
                    }),
                    // Exit button
                    widget::button("Exit Game", |_: On<Pointer<Click>>,
                        mut exit: MessageWriter<AppExit>| {
                        exit.write(AppExit::Success);
                    }),
                ],
            ),
        ],
    ));
}
```

No more `handle_main_menu_input` system — buttons handle everything via observers. ESC handling is not needed on main menu (nowhere to go back to).

Update tests: replace keyboard input test with button-spawn verification test.

#### 2. Update `screens/main_menu.rs` (no changes needed)
The `open_main_menu` system just sets `Menu::Main` — still correct.

#### 3. Keep `screens/loading.rs` as-is
Loading screen is kept as a skeleton for future asset loading. No changes needed.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes
- [x] `make build` succeeds

#### Manual Verification:
- [ ] Main menu shows centered bordered panel with title and two buttons
- [ ] "Start Battle" button has hover/press visual feedback and starts the game
- [ ] "Exit Game" button closes the application
- [ ] Panel layout matches mockup (title at top, buttons centered, spacing between)

**Implementation Note**: Pause here for manual verification.

---

## Phase 3: In-Game Bottom Bar

### Overview
Replace the standalone shop panel and top-right gold display with a unified full-width bottom bar: Gold (left) | 4 cards + Reroll (center) | Elapsed time + Minimap placeholder (right).

### Changes Required:

#### 1. Add `GameStartTime` resource
**File**: `src/gameplay/mod.rs`
**Changes**: Add resource for tracking when gameplay started (for elapsed time display).

```rust
/// Virtual time when the current game started.
/// Used to compute elapsed game time for the HUD.
#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct GameStartTime(pub f32);
```

Register in gameplay plugin: `app.register_type::<GameStartTime>().init_resource::<GameStartTime>();`

#### 2. Create `gameplay/hud/` module (NEW directory)
**File**: `src/gameplay/hud/mod.rs` (NEW)
**Purpose**: Owns the entire bottom bar layout, elapsed time, minimap placeholder.

```rust
//! In-game HUD: bottom bar with gold, cards, reroll, elapsed time, minimap.

mod bottom_bar;
mod elapsed_time;

use bevy::prelude::*;

use crate::screens::GameState;
use crate::{GameSet, gameplay_running};

pub fn plugin(app: &mut App) {
    app.add_plugins((bottom_bar::plugin, elapsed_time::plugin));
}
```

#### 3. Create `gameplay/hud/bottom_bar.rs` (NEW)
**File**: `src/gameplay/hud/bottom_bar.rs`
**Purpose**: Spawns the bottom bar with 3 sections.

The bottom bar is a `PositionType::Absolute` flex row pinned to the screen bottom. Three child sections with `flex_grow: 1.0` for left/center/right distribution:

- **Left section**: Gold display text
- **Center section**: 4 card slots + reroll button (migrated from `shop_ui.rs`)
- **Right section**: Elapsed time display + Minimap placeholder box

The bar uses `DespawnOnExit(GameState::InGame)`.

Key implementation detail: the shop card slots and reroll button components (`CardSlot`, `RerollButton`, etc.) stay in `economy/shop_ui.rs` since they own the shop interaction logic. The bottom bar just provides the layout container. Shop UI spawns its card slots as children of the center section via a marker component (`BottomBarCenter`).

Alternatively (simpler): merge the shop panel spawning into `bottom_bar.rs` and keep `shop_ui.rs` for the interaction/update systems only. This avoids cross-module parent-child wiring.

**Recommended approach**: Bottom bar owns ALL spawning (gold display + cards + reroll + elapsed time + minimap). Shop interaction systems (`handle_card_click`, `handle_reroll_click`, `update_card_visuals`, `update_card_text`, `update_reroll_text`) stay in `shop_ui.rs`. Gold update system stays in `economy/ui.rs`. This keeps spawning centralized and interaction logic modular.

#### 4. Refactor `economy/shop_ui.rs`
**File**: `src/gameplay/economy/shop_ui.rs`
**Changes**:
- Remove `spawn_shop_panel` (moved to `hud/bottom_bar.rs`)
- Keep all interaction/update systems
- Remove scattered color constants (use `palette::*`)
- Remove font size magic numbers (use `palette::FONT_SIZE_*`)
- Export component types (`CardSlot`, `CardNameText`, etc.) as `pub(crate)` for use by bottom bar

#### 5. Refactor `economy/ui.rs`
**File**: `src/gameplay/economy/ui.rs`
**Changes**:
- Remove `spawn_gold_hud` (gold display spawned by bottom bar)
- Keep `update_gold_display` system
- Update font size to use `palette::FONT_SIZE_HUD`

#### 6. Set `GameStartTime` on enter
**File**: Add system to `gameplay/mod.rs` or `hud/mod.rs`
**Changes**: On `OnEnter(GameState::InGame)`, store the current virtual time:

```rust
fn record_game_start_time(time: Res<Time<Virtual>>, mut start: ResMut<GameStartTime>) {
    start.0 = time.elapsed_secs();
}
```

#### 7. Create `gameplay/hud/elapsed_time.rs` (NEW)
**File**: `src/gameplay/hud/elapsed_time.rs`
**Purpose**: Update elapsed time display text.

```rust
fn update_elapsed_time(
    time: Res<Time<Virtual>>,
    start: Res<GameStartTime>,
    mut query: Single<&mut Text, With<ElapsedTimeDisplay>>,
) {
    let elapsed = time.elapsed_secs() - start.0;
    let total_secs = elapsed as u32;
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    **query = Text::new(format!("{minutes:02}:{seconds:02}"));
}
```

Runs in `GameSet::Ui` with `gameplay_running`.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes
- [x] `make build` succeeds

#### Manual Verification:
- [ ] Bottom bar spans full screen width, pinned to bottom
- [ ] Gold display on left, 4 card slots + reroll in center, elapsed time + minimap placeholder on right
- [ ] Card selection, hover, and reroll still work correctly
- [ ] Elapsed time counts up in MM:SS format
- [ ] Timer pauses when pause menu is open
- [ ] Minimap is a visible placeholder rectangle

**Implementation Note**: Pause here for manual verification.

---

## Phase 4: Pause + Endgame Overlays

### Overview
Rework pause, victory, and defeat menus with bordered panels, clickable buttons, and proper overlay z-ordering.

### Changes Required:

#### 1. Rewrite `menus/pause.rs`
**File**: `src/menus/pause.rs`
**Changes**: Replace keyboard-only pause with bordered panel + buttons.

```rust
fn spawn_pause_menu(mut commands: Commands) {
    commands.spawn((
        widget::ui_root("Pause Menu"),
        BackgroundColor(palette::OVERLAY_BACKGROUND),
        GlobalZIndex(1), // Above bottom bar
        DespawnOnExit(Menu::Pause),
        children![
            (
                Name::new("Pause Panel"),
                Node {
                    width: Val::Px(500.0),
                    min_height: Val::Px(400.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::all(Val::Px(40.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(palette::PANEL_BACKGROUND),
                BorderColor(palette::PANEL_BORDER),
                children![
                    (
                        Text::new("Auto Battle"),
                        TextFont::from_font_size(palette::FONT_SIZE_TITLE),
                        TextColor(palette::HEADER_TEXT),
                    ),
                    widget::button("Continue", |_: On<Pointer<Click>>,
                        mut next_menu: ResMut<NextState<Menu>>| {
                        next_menu.set(Menu::None);
                    }),
                    widget::button("Exit Game", |_: On<Pointer<Click>>,
                        mut next_game: ResMut<NextState<GameState>>,
                    | {
                        next_game.set(GameState::MainMenu);
                    }),
                ],
            ),
        ],
    ));
}
```

Keep `open_pause_menu` in `screens/in_game.rs` (ESC key still opens pause). Remove `handle_pause_input` keyboard system.

#### 2. Rewrite `menus/endgame.rs`
**File**: `src/menus/endgame.rs`
**Changes**: Replace keyboard prompts with styled panels and buttons.

Victory screen:
- Green-tinted panel border or header color for positive feel
- "VICTORY!" header
- "Restart" button (→ new InGame) + "Exit to Menu" button (→ MainMenu)

Defeat screen:
- Red-tinted panel border or header color for somber feel
- "DEFEAT" header
- "Restart" button + "Exit to Menu" button

```rust
fn spawn_endgame_overlay(commands: &mut Commands, title: &str, title_color: Color, menu: Menu) {
    commands.spawn((
        widget::ui_root("Endgame Screen"),
        BackgroundColor(palette::OVERLAY_BACKGROUND),
        GlobalZIndex(1),
        DespawnOnExit(menu),
        children![
            (
                Name::new("Endgame Panel"),
                Node {
                    width: Val::Px(500.0),
                    min_height: Val::Px(300.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceEvenly,
                    padding: UiRect::all(Val::Px(40.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(palette::PANEL_BACKGROUND),
                BorderColor(palette::PANEL_BORDER),
                children![
                    (
                        Text::new(title),
                        TextFont::from_font_size(palette::FONT_SIZE_HEADER),
                        TextColor(title_color),
                    ),
                    widget::button("Restart", |_: On<Pointer<Click>>,
                        mut next_game: ResMut<NextState<GameState>>,
                        mut next_menu: ResMut<NextState<Menu>>| {
                        // Restart: exit to trigger cleanup, then re-enter InGame
                        next_game.set(GameState::InGame);
                        next_menu.set(Menu::None);
                    }),
                    widget::button("Exit to Menu", |_: On<Pointer<Click>>,
                        mut next_game: ResMut<NextState<GameState>>| {
                        next_game.set(GameState::MainMenu);
                    }),
                ],
            ),
        ],
    ));
}
```

Victory uses green header text, defeat uses red header text.

Remove `handle_endgame_input` keyboard system.

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes
- [x] `make build` succeeds

#### Manual Verification:
- [ ] Pause overlay appears over gameplay with semi-transparent background
- [ ] Pause panel has "Auto Battle" title, "Continue" and "Exit Game" buttons
- [ ] "Continue" resumes gameplay, "Exit Game" returns to main menu
- [ ] Bottom bar visible underneath pause overlay
- [ ] Victory screen has green-tinted title, "Restart" + "Exit to Menu" buttons
- [ ] Defeat screen has red-tinted title, same buttons
- [ ] "Restart" starts a new game directly
- [ ] "Exit to Menu" returns to main menu

**Implementation Note**: Pause here for manual verification.

---

## Phase 5: Camera + Production Indicator

### Overview
Add vertical camera panning (W/S, Up/Down) and production progress bars on buildings.

### Changes Required:

#### 1. Add vertical camera panning
**File**: `src/gameplay/battlefield/camera.rs`
**Changes**: Add Y-axis input and clamping alongside existing X panning.

```rust
pub(super) fn camera_pan(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera: Single<&mut Transform, With<Camera2d>>,
    windows: Single<&Window>,
) {
    // X-axis panning (existing)
    let mut x_direction = 0.0;
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        x_direction += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        x_direction -= 1.0;
    }
    camera.translation.x += x_direction * CAMERA_PAN_SPEED * time.delta_secs();

    // Y-axis panning (new)
    let mut y_direction = 0.0;
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        y_direction += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        y_direction -= 1.0;
    }
    camera.translation.y += y_direction * CAMERA_PAN_SPEED * time.delta_secs();

    // X clamping (existing)
    let aspect_ratio = windows.width() / windows.height();
    let visible_width = BATTLEFIELD_HEIGHT * aspect_ratio;
    let half_visible_x = visible_width / 2.0;
    let min_x = half_visible_x;
    let max_x = BATTLEFIELD_WIDTH - half_visible_x;
    camera.translation.x = camera.translation.x.clamp(min_x, max_x);

    // Y clamping (new)
    // FixedVertical shows full height at default zoom, so min==max (no movement).
    // With future zoom, this would allow vertical panning.
    let half_visible_y = BATTLEFIELD_HEIGHT / 2.0;
    let min_y = half_visible_y;
    let max_y = BATTLEFIELD_HEIGHT - half_visible_y;
    camera.translation.y = camera.translation.y.clamp(min_y, max_y);
}
```

Note: At default zoom with `FixedVertical`, `min_y == max_y == 320.0`, so Y panning has no visible effect. The code is ready for a future zoom feature. This is intentional per the ticket: "Add vertical camera panning alongside existing horizontal panning."

#### 2. Add production progress bar
**File**: `src/gameplay/building/production.rs` (extend)
**Changes**: Add production bar components + observer + update system.

```rust
// === Production Bar Components ===

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct ProductionBarBackground;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct ProductionBarFill;

/// Configuration for production bar sizing.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct ProductionBarConfig {
    pub width: f32,
    pub height: f32,
    pub y_offset: f32,
}
```

Observer on `Add<ProductionTimer>` spawns bar children (same pattern as health bar):

```rust
fn spawn_production_bars(
    add: On<Add, ProductionTimer>,
    configs: Query<&ProductionBarConfig>,
    mut commands: Commands,
) {
    let Ok(config) = configs.get(add.entity) else { return };
    commands.entity(add.entity).with_children(|parent| {
        parent.spawn((
            Name::new("Production Bar BG"),
            Sprite::from_color(palette::PRODUCTION_BAR_BG, Vec2::new(config.width, config.height)),
            Transform::from_xyz(0.0, config.y_offset, 1.0),
            ProductionBarBackground,
        ));
        parent.spawn((
            Name::new("Production Bar Fill"),
            Sprite::from_color(palette::PRODUCTION_BAR_FILL, Vec2::new(config.width, config.height)),
            Transform::from_xyz(0.0, config.y_offset, 1.1),
            ProductionBarFill,
        ));
    });
}
```

Update system reads `ProductionTimer.0.fraction()` and scales the fill:

```rust
fn update_production_bars(
    timer_query: Query<(&ProductionTimer, &Children, &ProductionBarConfig)>,
    mut bar_query: Query<&mut Transform, With<ProductionBarFill>>,
) {
    for (timer, children, config) in &timer_query {
        let ratio = timer.0.fraction();
        for child in children.iter() {
            if let Ok(mut transform) = bar_query.get_mut(child) {
                transform.scale.x = ratio;
                transform.translation.x = config.width.mul_add(-(1.0 - ratio), 0.0) / 2.0;
            }
        }
    }
}
```

Add `ProductionBarConfig` to building spawning (in `building/placement.rs` or `building/mod.rs`) alongside `HealthBarConfig`:

```rust
ProductionBarConfig {
    width: 30.0,   // Same as building health bar width
    height: 2.0,
    y_offset: -24.0, // Below the building sprite, below health bar
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes
- [x] `make build` succeeds

#### Manual Verification:
- [ ] W/S and Up/Down arrow keys attempt vertical panning (no visible effect at default zoom — expected)
- [ ] A/D and Left/Right still work for horizontal panning
- [ ] Buildings show a blue progress bar that fills left-to-right as production timer advances
- [ ] Progress bar resets when unit is spawned
- [ ] Progress bar disappears when building is destroyed

**Implementation Note**: Pause here for manual verification.

---

## Phase 6: Tests

### Overview
Update all existing tests and add new tests for button interactions, timer display, production bars, and overlay spawning.

### Changes Required:

#### 1. Update existing menu tests
- **`menus/main_menu.rs` tests**: Remove `space_starts_game` keyboard test. Add test for `spawn_main_menu` entity count (ui_root + panel + title + 2 buttons).
- **`menus/pause.rs` tests**: Remove `escape_unpauses` and `q_quits_to_main_menu` keyboard tests. Add `spawn_pause_menu` entity count test.
- **`menus/endgame.rs` tests**: Remove `handle_endgame_input_returns_to_menu_on_q`. Update spawn tests for new entity counts (panel + title + 2 buttons).

#### 2. Update shop_ui tests
- Tests that reference `spawn_shop_panel` need updating since spawning moves to `hud/bottom_bar.rs`.
- Card click and reroll click tests still work (they test interaction systems in isolation).

#### 3. New theme tests
- **`interaction.rs`**: Test that `apply_interaction_palette` changes `BackgroundColor` when `Interaction` changes.
- **`widget.rs`**: Test that `button()` spawns expected entity hierarchy (outer node + inner Button + text child).

#### 4. New HUD tests
- **`bottom_bar.rs`**: Test that bottom bar spawns with 3 sections.
- **`elapsed_time.rs`**: Test that elapsed time text updates correctly with `GameStartTime` offset.

#### 5. New production bar tests
- Test that production bar spawns as children when `ProductionTimer` is added.
- Test that production bar fill scales with timer fraction.

#### 6. Camera pan tests
- Add test for Y-axis input reading (mirror existing X-axis test pattern).

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes with all new and updated tests (208 total, +12 new)
- [x] Coverage maintains or increases toward 90% target

#### Manual Verification:
- [ ] Full end-to-end playthrough: main menu → start → play → pause → resume → win/lose → restart/exit

---

## Testing Strategy

### Unit Tests:
- `InteractionPalette` color changes on `Changed<Interaction>`
- `format_elapsed_time` helper for MM:SS formatting
- Production bar fraction scaling
- Font size tokens are positive
- All palette colors are distinct (no accidental duplicates)

### Integration Tests:
- Main menu spawns expected entity hierarchy
- Pause overlay spawns with `GlobalZIndex`
- Bottom bar has 3 sections with expected children
- Production bar observer fires on `Add<ProductionTimer>`
- Gold display updates when `Gold` resource changes
- Elapsed time updates each frame

### Manual Testing Steps:
1. Launch game → main menu centered with bordered panel
2. Hover "Start Battle" → button highlights
3. Click "Start Battle" → game starts with bottom bar
4. Verify bottom bar: Gold left, 4 cards center, reroll center-right, time + minimap right
5. Select a card → card highlights green
6. Place building → production bar appears and fills
7. Press ESC → pause overlay with bordered panel
8. Click "Continue" → resume gameplay
9. Click "Exit Game" on pause → return to main menu
10. Win/lose → endgame overlay with Restart + Exit buttons
11. Click "Restart" → new game starts directly

## Performance Considerations

- `InteractionPalette` system uses `Changed<Interaction>` filter — only runs when interaction state changes, not every frame
- Production bar update uses `Query` iteration — only processes entities with `ProductionTimer`, minimal overhead
- Elapsed time update is a single `Single<>` query — O(1) per frame
- `GlobalZIndex` only used on overlay roots (2-3 entities max)

## Migration Notes

- Shop UI spawning migrates from `economy/shop_ui.rs` to `gameplay/hud/bottom_bar.rs`
- Gold HUD spawning migrates from `economy/ui.rs` to `gameplay/hud/bottom_bar.rs`
- Update systems stay in their original modules (no behavior change)
- All keyboard-only menu input handlers are removed (replaced by button observers)
- ESC to open pause is kept (in `screens/in_game.rs`)

## References

- Linear ticket: [GAM-13](https://linear.app/tayhu-games/issue/GAM-13/menu-rework)
- Research doc: `thoughts/shared/research/2026-02-04-tano-style-game-research.md`
- Architecture: `ARCHITECTURE.md` (button pattern, widget constructors, observer patterns)
- Foxtrot button pattern: verified against foxtrot source (SpawnWith + ChildSpawner + .observe())
- Foxtrot interaction pattern: verified against foxtrot source (Changed<Interaction> + InteractionPalette)

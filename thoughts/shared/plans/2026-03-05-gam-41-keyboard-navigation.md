# Keyboard Navigation for UI Buttons — Implementation Plan

## Overview

Add full keyboard navigation to all menu buttons: arrow keys cycle focus, Enter/Space confirms, visual focus indicator, wrap-around, auto-focus on menu open, ESC back navigation. Uses Bevy 0.18's built-in `bevy_input_focus` crate (`InputFocus`, `TabNavigationPlugin`, `TabIndex`, `TabGroup`).

## Current State Analysis

- **Button widget** (`theme/widget.rs:56-97`): Generic over event type. Buttons use `Pointer<Click>` observers. Inner `Button` node has `InteractionPalette` for hover/press colors.
- **InteractionPalette** (`theme/interaction.rs:12-16`): Tracks `none`/`hovered`/`pressed` colors. Applied via `Changed<Interaction>` query.
- **Menus**: 4 menus spawn buttons via `widget::button()`:
  - Main menu (`menus/main_menu.rs`): 2 buttons (Start Battle, Exit Game)
  - Pause menu (`menus/pause.rs`): 2 buttons (Continue, Exit Game)
  - Victory overlay (`menus/endgame.rs`): 1 button (Exit to Menu)
  - Defeat overlay (`menus/endgame.rs`): 1 button (Exit to Menu)
- **ESC handling**: Only `screens/in_game.rs:21` — opens pause menu. No ESC-to-close in menus themselves.

### Key Discoveries:
- `InputDispatchPlugin` + `TabNavigationPlugin` are NOT in `DefaultPlugins` — must add manually
- `commands.entity(e).trigger(EventConstructor)` is the Bevy 0.18 idiom for firing entity events
- `Outline::new(Val::Px(w), Val::Px(offset), color)` doesn't affect layout — ideal for focus ring
- `AutoFocus` component fires immediately on spawn via hook — sets `InputFocus` at spawn time
- `TabNavigation` is a `SystemParam` with `.navigate(&focus, NavAction::Next/Previous)`

## Desired End State

All menu screens support full keyboard navigation:
- Up/Down arrow keys cycle focus between buttons (with wrap-around)
- Enter/Space activates the focused button
- A white outline visually indicates the focused button
- First button auto-focuses when any menu opens
- ESC closes the current menu (pause → resume, endgame → main menu)
- Mouse interaction still works exactly as before

### Verification:
- `make check` passes (no warnings)
- `make test` passes (all existing + new tests)
- Manual: open each menu, navigate with arrows, confirm with Enter, verify focus ring, verify ESC

## What We're NOT Doing

- Gamepad/controller support (separate ticket)
- Tab key navigation (built-in TabNavigationPlugin handles it for free, but we don't need to test/document it)
- Focus for non-menu UI (shop cards, HUD) — only menu buttons
- Sound effects on focus change
- Animation on focus change (just instant outline toggle)

## Implementation Approach

Three phases: (1) core infrastructure in `theme/`, (2) integrate into all menus, (3) tests.

The key architectural change is introducing a custom `Activate` entity event. Currently buttons observe `Pointer<Click>` directly. We add a bridge: `Pointer<Click>` → fires `Activate` on the same entity. The keyboard confirm system also fires `Activate`. Button action closures observe `Activate` instead of `Pointer<Click>`.

## Verified API Patterns (Bevy 0.18)

These were verified against actual crate source:

- **Custom `EntityEvent`**: `#[derive(Event, EntityEvent, Clone, Debug, Reflect)] struct Activate(Entity);` — tuple struct with Entity field auto-detected as target
- **Trigger on entity**: `commands.entity(e).trigger(Activate);` — tuple struct constructor implements `FnOnce(Entity) -> Activate`
- **`TabIndex`**: `TabIndex(i32)` component, `i32 >= 0` = tabbable. Import: `bevy::input_focus::tab_navigation::TabIndex`
- **`TabGroup`**: `TabGroup::new(order)` component on parent container. Import: `bevy::input_focus::tab_navigation::TabGroup`
- **`AutoFocus`**: Marker component, fires hook on spawn to set `InputFocus`. Import: `bevy::input_focus::AutoFocus`
- **`InputFocus`**: Resource `InputFocus(pub Option<Entity>)`. Import: `bevy::input_focus::InputFocus`
- **`TabNavigation`**: SystemParam. `nav.navigate(&focus, NavAction::Next)` returns `Result<Entity, TabNavigationError>`
- **`Outline`**: `Outline::new(Val::Px(2.), Val::Px(2.), Color::WHITE)` — no layout impact
- **Plugin registration**: `InputDispatchPlugin` (from `bevy::input_focus`) + `TabNavigationPlugin` (from `bevy::input_focus::tab_navigation`)
- **`NavAction`**: `NavAction::Next`, `NavAction::Previous`. Import: `bevy::input_focus::tab_navigation::NavAction`

---

## Phase 1: Core Infrastructure

### Overview
Add the `Activate` event, register focus plugins, refactor the button widget, and add the three new systems (keyboard confirm, arrow navigation, focus visuals).

### Changes Required:

#### 1. Register focus plugins
**File**: `src/theme/mod.rs`
**Changes**: Add `InputDispatchPlugin` and `TabNavigationPlugin` to the theme plugin.

```rust
use bevy::input_focus::InputDispatchPlugin;
use bevy::input_focus::tab_navigation::TabNavigationPlugin;

pub fn plugin(app: &mut App) {
    app.add_plugins((
        InputDispatchPlugin,
        TabNavigationPlugin,
        palette::plugin,
        interaction::plugin,
        widget::plugin,
    ));
}
```

Note: `widget::plugin` is new — needed to register the `Activate` event type and the keyboard/focus systems.

#### 2. Define `Activate` event and keyboard systems
**File**: `src/theme/widget.rs`
**Changes**: Add the `Activate` event, a bridge observer, and refactor `button()`. Add a new `plugin` function with keyboard confirm, arrow nav, and focus visual systems.

```rust
use bevy::input_focus::InputFocus;
use bevy::input_focus::tab_navigation::{NavAction, TabNavigation};

/// Custom entity event fired when a button is activated (click or keyboard Enter/Space).
#[derive(Event, EntityEvent, Clone, Debug, Reflect)]
pub struct Activate(pub Entity);

/// Plugin for widget systems (keyboard navigation, focus visuals).
pub fn plugin(app: &mut App) {
    app.register_type::<Activate>();
    app.add_systems(Update, (keyboard_confirm_focused, arrow_key_navigation, update_focus_outline));
}
```

**Refactored `button()` function:**

```rust
/// Clickable button with text and an observer-based action.
/// The action observes `Activate`, which fires on both mouse click and keyboard Enter/Space.
pub fn button<B, M, I>(text: impl Into<String>, action: I) -> impl Bundle
where
    B: Bundle,
    I: IntoObserverSystem<Activate, B, M>,
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
                    BorderColor::all(palette::PANEL_BORDER),
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
                // Bridge: Pointer<Click> → Activate
                .observe(
                    |click: On<Pointer<Click>>, mut commands: Commands| {
                        commands.entity(click.entity).trigger(Activate);
                    },
                )
                // User-provided action
                .observe(action);
        })),
    )
}
```

**New systems:**

```rust
/// Fire `Activate` on the focused button when Enter or Space is pressed.
fn keyboard_confirm_focused(
    input: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    if input.just_pressed(KeyCode::Enter) || input.just_pressed(KeyCode::Space) {
        if let Some(entity) = focus.0 {
            commands.entity(entity).trigger(Activate);
        }
    }
}

/// Arrow Up/Down mapped to TabNavigation Previous/Next.
fn arrow_key_navigation(
    input: Res<ButtonInput<KeyCode>>,
    nav: TabNavigation,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    let action = if input.just_pressed(KeyCode::ArrowUp) {
        Some(NavAction::Previous)
    } else if input.just_pressed(KeyCode::ArrowDown) {
        Some(NavAction::Next)
    } else {
        None
    };

    if let Some(action) = action {
        if let Ok(next) = nav.navigate(&focus, action) {
            focus.0 = Some(next);
            focus_visible.0 = true;
        }
    }
}

/// Show/hide outline on the focused button entity.
fn update_focus_outline(
    focus: Res<InputFocus>,
    mut buttons: Query<(Entity, &mut Outline), With<Button>>,
) {
    for (entity, mut outline) in &mut buttons {
        if focus.0 == Some(entity) {
            outline.width = Val::Px(2.0);
            outline.offset = Val::Px(2.0);
            outline.color = Color::WHITE;
        } else {
            outline.color = Color::NONE;
        }
    }
}
```

**Important**: Add `Outline::default()` to the button spawn bundle so every button has the component (avoids insert/remove table moves per Bevy docs recommendation).

Add to the button inner spawn:
```rust
Outline::default(), // Focus ring — color set to NONE by default, toggled by update_focus_outline
```

#### 3. Add focus palette color constant
**File**: `src/theme/palette.rs`
**Changes**: Add a focus outline color constant.

```rust
pub const BUTTON_FOCUS_OUTLINE: Color = Color::WHITE;
```

(Used by `update_focus_outline` instead of hardcoded `Color::WHITE`.)

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes — no compiler errors or warnings
- [ ] `make test` passes — existing tests still pass (button closure signatures changed)

#### Manual Verification:
- [ ] N/A for this phase alone — menus don't have TabGroup/TabIndex yet

**Implementation Note**: After completing this phase and all automated verification passes, proceed to Phase 2 immediately (no manual gate — infrastructure only).

---

## Phase 2: Menu Integration

### Overview
Add `TabGroup`, `TabIndex`, and `AutoFocus` to all menu spawning functions. Add ESC-to-close for pause and endgame menus. Update button action closure signatures from `On<Pointer<Click>>` to `On<Activate>`.

### Changes Required:

#### 1. Main Menu
**File**: `src/menus/main_menu.rs`
**Changes**: Add `TabGroup` to panel, `TabIndex` + `AutoFocus` to buttons, update closure signatures.

```rust
use bevy::input_focus::AutoFocus;
use bevy::input_focus::tab_navigation::{TabGroup, TabIndex};
use crate::theme::widget::{self, Activate};

fn spawn_main_menu(mut commands: Commands) {
    commands.spawn((
        widget::ui_root("Main Menu Screen"),
        DespawnOnExit(Menu::Main),
        children![
            (
                Name::new("Main Menu Panel"),
                Node { /* ... same ... */ },
                BackgroundColor(palette::PANEL_BACKGROUND),
                BorderColor::all(palette::PANEL_BORDER),
                TabGroup::new(0),  // NEW
                children![
                    // Title (unchanged)
                    (
                        Text::new("Auto Battle"),
                        TextFont::from_font_size(palette::FONT_SIZE_TITLE),
                        TextColor(palette::HEADER_TEXT),
                    ),
                    // Start button — TabIndex(0) + AutoFocus
                    widget::button_with_nav(
                        "Start Battle",
                        0,
                        true, // auto_focus
                        |_: On<Activate>,
                         mut next_game: ResMut<NextState<GameState>>,
                         mut next_menu: ResMut<NextState<Menu>>| {
                            next_game.set(GameState::InGame);
                            next_menu.set(Menu::None);
                        },
                    ),
                    // Exit button — TabIndex(1)
                    widget::button_with_nav(
                        "Exit Game",
                        1,
                        false,
                        |_: On<Activate>, mut exit: MessageWriter<AppExit>| {
                            exit.write(AppExit::Success);
                        },
                    ),
                ],
            ),
        ],
    ));
}
```

**Alternative approach — simpler**: Instead of creating `button_with_nav()`, keep `widget::button()` unchanged and add `TabIndex`/`AutoFocus` as sibling components on the **inner Button entity** by having each menu manually add them. But since the inner Button is spawned inside `SpawnWith`, the menu code can't easily add components to it.

**Chosen approach**: Add a new `widget::button_with_nav()` variant that accepts `tab_index: i32` and `auto_focus: bool` parameters. This keeps the widget module as the single place that knows about button internals.

```rust
/// Button with keyboard navigation support (TabIndex + optional AutoFocus).
pub fn button_with_nav<B, M, I>(
    text: impl Into<String>,
    tab_index: i32,
    auto_focus: bool,
    action: I,
) -> impl Bundle
where
    B: Bundle,
    I: IntoObserverSystem<Activate, B, M>,
{
    // Same as button() but adds TabIndex(tab_index) and optionally AutoFocus
    // to the inner Button entity
}
```

Actually, even simpler: since ALL menu buttons now need TabIndex, **replace** `button()` with the nav-aware version. Non-menu buttons (shop cards) don't use `widget::button()`. So we modify the existing `button()` signature to include `tab_index` and `auto_focus`:

```rust
pub fn button<B, M, I>(
    text: impl Into<String>,
    tab_index: i32,
    auto_focus: bool,
    action: I,
) -> impl Bundle
```

This avoids having two button functions. All call sites get updated.

#### 2. Pause Menu
**File**: `src/menus/pause.rs`
**Changes**: Add `TabGroup` to panel, `TabIndex`/`AutoFocus` to buttons, update closure signatures, add ESC-to-close system.

```rust
pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Pause), spawn_pause_menu);
    app.add_systems(Update, close_pause_on_escape.run_if(in_state(Menu::Pause)));
}

fn close_pause_on_escape(
    input: Res<ButtonInput<KeyCode>>,
    mut next_menu: ResMut<NextState<Menu>>,
) {
    if input.just_pressed(KeyCode::Escape) {
        next_menu.set(Menu::None);
    }
}
```

Buttons: "Continue" gets `TabIndex(0)` + `AutoFocus`, "Exit Game" gets `TabIndex(1)`.

#### 3. Endgame Menus (Victory/Defeat)
**File**: `src/menus/endgame.rs`
**Changes**: Add `TabGroup` to panel, `TabIndex`/`AutoFocus` to the single button, add ESC-to-main-menu systems.

```rust
pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Victory), spawn_victory_screen);
    app.add_systems(OnEnter(Menu::Defeat), spawn_defeat_screen);
    app.add_systems(
        Update,
        close_endgame_on_escape.run_if(in_state(Menu::Victory).or(in_state(Menu::Defeat))),
    );
}

fn close_endgame_on_escape(
    input: Res<ButtonInput<KeyCode>>,
    mut next_game: ResMut<NextState<GameState>>,
) {
    if input.just_pressed(KeyCode::Escape) {
        next_game.set(GameState::MainMenu);
    }
}
```

Single button: "Exit to Menu" gets `TabIndex(0)` + `AutoFocus`.

#### 4. Update `On<Pointer<Click>>` → `On<Activate>` in all closures

All button action closures change their first parameter from `_: On<Pointer<Click>>` to `_: On<Activate>`.

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes

#### Manual Verification:
- [ ] Main menu: arrows cycle Start/Exit, Enter starts game, focus ring visible
- [ ] Pause menu: arrows cycle Continue/Exit, Enter works, ESC resumes game
- [ ] Victory overlay: single button focused, Enter exits to menu, ESC exits to menu
- [ ] Defeat overlay: same as victory
- [ ] Focus wraps around (last button → first, first → last)
- [ ] Mouse clicks still work on all buttons
- [ ] Focus ring appears when using keyboard, disappears on mouse click (built-in `InputFocusVisible` behavior)

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to Phase 3.

---

## Phase 3: Tests

### Overview
Add integration tests for keyboard navigation, focus cycling, Enter confirmation, and ESC handling.

### Changes Required:

#### 1. Widget/interaction tests
**File**: `src/theme/widget.rs` (test module)
**Changes**: Test that `Activate` event fires from keyboard confirm system.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_enter_triggers_activate_on_focused_button() {
        // Setup: MinimalPlugins + StatesPlugin + InputDispatchPlugin
        // Spawn a button with TabIndex + action that sets a flag resource
        // Set InputFocus to the button entity
        // Press Enter
        // Assert flag resource was set
    }

    #[test]
    fn arrow_down_cycles_focus_to_next_button() {
        // Setup: spawn 2 buttons with TabIndex(0), TabIndex(1) in a TabGroup
        // Set focus to first button
        // Press ArrowDown
        // Assert InputFocus is now second button
    }

    #[test]
    fn arrow_up_wraps_from_first_to_last() {
        // Setup: spawn 2 buttons in TabGroup
        // Set focus to first button
        // Press ArrowUp
        // Assert InputFocus wrapped to second (last) button
    }

    #[test]
    fn focus_outline_shown_on_focused_button() {
        // Spawn 2 buttons with Outline::default()
        // Set InputFocus to first
        // Run update_focus_outline
        // Assert first has white outline, second has Color::NONE
    }
}
```

#### 2. Menu-specific tests
**File**: `src/menus/pause.rs` (test module)
**Changes**: Test ESC closes pause menu.

```rust
#[test]
fn escape_closes_pause_menu() {
    // Setup with Menu::Pause active
    // Press Escape
    // Assert NextState<Menu> is Menu::None
}
```

**File**: `src/menus/endgame.rs` (test module)
**Changes**: Test ESC exits to main menu from victory/defeat.

```rust
#[test]
fn escape_exits_victory_to_main_menu() {
    // Setup with Menu::Victory active
    // Press Escape
    // Assert NextState<GameState> is MainMenu
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — all new tests pass
- [ ] Coverage maintained or increased toward 90% target

#### Manual Verification:
- [ ] Full end-to-end keyboard navigation test through all menus

---

## Testing Strategy

### Unit Tests:
- `Activate` event construction and entity target
- Focus outline system (correct entity gets outline)
- Arrow key navigation (up/down cycling, wrap-around)

### Integration Tests:
- Keyboard Enter fires `Activate` on focused button
- ESC closes pause menu (sets `NextState<Menu>::None`)
- ESC exits endgame to main menu
- `AutoFocus` sets focus on menu open

### Manual Testing Steps:
1. Launch game → main menu has "Start Battle" focused with outline
2. Press Down → "Exit Game" focused
3. Press Down → wraps to "Start Battle"
4. Press Up → wraps to "Exit Game"
5. Press Enter on "Start Battle" → game starts
6. Press ESC → pause menu, "Continue" focused
7. Press Down → "Exit Game" focused
8. Press Enter on "Continue" → game resumes
9. Click a button with mouse → focus ring disappears, button activates
10. Win/lose → endgame overlay, button focused, Enter exits, ESC exits

## Performance Considerations

- `update_focus_outline` runs every frame but only iterates `Button` entities (typically 1-3 in menus). Negligible cost.
- `arrow_key_navigation` and `keyboard_confirm_focused` both early-return when no key pressed. No cost when idle.
- `Outline::default()` on every button adds minimal memory overhead.

## References

- Linear ticket: [GAM-41](https://linear.app/tayhu-games/issue/GAM-41/keyboard-navigation-for-ui-buttons)
- Bevy example: `bevy-0.18.0/examples/ui/tab_navigation.rs`
- Bevy input_focus crate: `bevy_input_focus-0.18.0/src/`
- Current widget: `src/theme/widget.rs:56-97`
- Current interaction: `src/theme/interaction.rs`

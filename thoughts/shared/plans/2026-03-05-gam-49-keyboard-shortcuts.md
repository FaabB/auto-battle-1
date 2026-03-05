# Keyboard Shortcuts for Shop (GAM-49) Implementation Plan

## Overview

Add keyboard shortcuts so players can select building cards (1/2/3/4) and reroll (R) without clicking, complementing the existing mouse-based shop UI.

## Current State Analysis

- `shop_ui.rs` has `handle_card_click` (mouse card selection) and `handle_reroll_click` (mouse reroll) systems in `GameSet::Input` with `gameplay_running`
- `Shop` resource tracks `cards: [Option<BuildingType>; 4]` and `selected: Option<usize>`
- Card selection logic: toggle if same slot clicked, otherwise select new slot; ignore empty slots
- Reroll logic: check gold >= cost, deduct gold, call `shop.reroll()`
- Bottom bar spawns card slot entities with `CardSlot(i)` marker and `Button` component
- Keyboard input already used for ESC (pause toggle) in `screens/in_game.rs`

### Key Discoveries:
- `handle_card_click` reads `Interaction` on `CardSlot` entities — keyboard system won't use `Interaction`, it reads `ButtonInput<KeyCode>` directly
- `handle_reroll_click` reads `Interaction` on `RerollButton` — same pattern
- Both systems already guard with `gameplay_running` (InGame + Menu::None)
- `shop_ui.rs:136-156` registers systems — new system slots in alongside existing ones

## Desired End State

Pressing 1/2/3/4 selects the corresponding card slot (toggle behavior, same as clicking). Pressing R triggers reroll (same as clicking the reroll button). These work only during active gameplay (not while paused/in menus).

### How to verify:
- Run the game, enter InGame, press 1-4 to select cards, press R to reroll
- Verify selection highlight updates correctly
- Verify reroll deducts gold and regenerates cards
- Verify keys do nothing when paused

## What We're NOT Doing

- Key hint labels on the UI cards (can add later)
- Configurable key bindings
- Keyboard-based building placement (already mouse-based)

## Implementation Approach

Single new system `handle_shop_keyboard` in `shop_ui.rs` that reads `ButtonInput<KeyCode>` and writes to `Shop` and `Gold` — exact same logic as the click handlers but triggered by key presses. Add alongside existing systems in `GameSet::Input`.

## Phase 1: Add Keyboard Input System

### Overview
Add the keyboard handler system and register it in the plugin.

### Changes Required:

#### 1. New system in `shop_ui.rs`
**File**: `src/gameplay/economy/shop_ui.rs`
**Changes**: Add `handle_shop_keyboard` system

```rust
/// Handle keyboard shortcuts for card selection (1-4) and reroll (R).
fn handle_shop_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut shop: ResMut<Shop>,
    mut gold: ResMut<super::Gold>,
) {
    // Card selection: Digit1-4 map to slots 0-3
    const CARD_KEYS: [KeyCode; 4] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ];

    for (slot_index, &key) in CARD_KEYS.iter().enumerate() {
        if keyboard.just_pressed(key) {
            if shop.cards[slot_index].is_some() {
                if shop.selected == Some(slot_index) {
                    shop.selected = None;
                } else {
                    shop.selected = Some(slot_index);
                }
            }
            return; // Only process one key per frame
        }
    }

    // Reroll: R key
    if keyboard.just_pressed(KeyCode::KeyR) {
        let cost = shop.reroll_cost();
        if gold.0 >= cost {
            gold.0 -= cost;
            shop.reroll();
        }
    }
}
```

#### 2. Register the system
**File**: `src/gameplay/economy/shop_ui.rs`
**Changes**: Add `handle_shop_keyboard` to the existing `GameSet::Input` system group

```rust
app.add_systems(
    Update,
    (handle_card_click, handle_reroll_click, handle_shop_keyboard)
        .in_set(GameSet::Input)
        .run_if(gameplay_running),
);
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (no lint/type errors)
- [ ] `make test` passes (all existing + new tests)
- [ ] `make build` passes

#### Manual Verification:
- [ ] Press 1-4 during gameplay to select/deselect cards
- [ ] Press R during gameplay to reroll
- [ ] Reroll blocked when insufficient gold
- [ ] Keys do nothing when game is paused
- [ ] Mouse click selection still works alongside keyboard

---

## Phase 2: Tests

### Overview
Add unit tests for the keyboard handler, following the same pattern as existing `handle_card_click` and `handle_reroll_click` tests.

### Changes Required:

#### 1. Tests in `shop_ui.rs`
**File**: `src/gameplay/economy/shop_ui.rs`
**Changes**: Add test functions in the existing `mod tests` block

```rust
fn create_keyboard_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<Shop>();
    app.init_resource::<Gold>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_systems(Update, handle_shop_keyboard);
    app
}

#[test]
fn keyboard_digit1_selects_first_card() {
    let mut app = create_keyboard_test_app();
    let mut shop = app.world_mut().resource_mut::<Shop>();
    shop.cards[0] = Some(BuildingType::Barracks);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit1);
    app.update();

    let shop = app.world().resource::<Shop>();
    assert_eq!(shop.selected, Some(0));
}

#[test]
fn keyboard_digit_toggles_selection() {
    let mut app = create_keyboard_test_app();
    let mut shop = app.world_mut().resource_mut::<Shop>();
    shop.cards[2] = Some(BuildingType::Farm);
    shop.selected = Some(2);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit3);
    app.update();

    let shop = app.world().resource::<Shop>();
    assert_eq!(shop.selected, None);
}

#[test]
fn keyboard_digit_empty_slot_ignored() {
    let mut app = create_keyboard_test_app();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit1);
    app.update();

    let shop = app.world().resource::<Shop>();
    assert_eq!(shop.selected, None);
}

#[test]
fn keyboard_r_rerolls() {
    let mut app = create_keyboard_test_app();
    app.world_mut().resource_mut::<Shop>().generate_cards();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();

    let shop = app.world().resource::<Shop>();
    for (i, card) in shop.cards.iter().enumerate() {
        assert!(card.is_some(), "Card slot {i} should be filled after reroll");
    }
}

#[test]
fn keyboard_r_deducts_gold() {
    let mut app = create_keyboard_test_app();
    let mut shop = app.world_mut().resource_mut::<Shop>();
    shop.generate_cards();
    shop.placed_since_last_reroll = false;
    shop.reroll(); // consecutive_no_build_rerolls = 1, cost = 5

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();

    let gold = app.world().resource::<Gold>();
    assert_eq!(gold.0, STARTING_GOLD - 5);
}

#[test]
fn keyboard_r_blocked_insufficient_gold() {
    let mut app = create_keyboard_test_app();
    let mut shop = app.world_mut().resource_mut::<Shop>();
    shop.placed_since_last_reroll = false;
    shop.consecutive_no_build_rerolls = 2; // cost = 10
    let old_cards = shop.cards;

    app.world_mut().resource_mut::<Gold>().0 = 5;

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();

    let shop = app.world().resource::<Shop>();
    let gold = app.world().resource::<Gold>();
    assert_eq!(shop.cards, old_cards);
    assert_eq!(gold.0, 5);
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make test` passes — all 6 new tests green
- [ ] `make check` passes

---

## Testing Strategy

### Unit Tests:
- Keyboard digit selects correct card slot
- Keyboard digit toggles already-selected card
- Keyboard digit on empty slot does nothing
- Keyboard R triggers reroll
- Keyboard R deducts gold correctly
- Keyboard R blocked when insufficient gold

### Manual Testing Steps:
1. Start a game, verify cards appear in bottom bar
2. Press 1 — first card highlights (selected color)
3. Press 1 again — card deselects
4. Press 2 — second card selects (first deselects)
5. Press R — cards refresh, gold changes if cost > 0
6. Open pause menu (ESC), press 1/R — nothing happens
7. Close pause, verify keys work again

## References

- Linear ticket: [GAM-49](https://linear.app/tayhu-games/issue/GAM-49/keyboard-shortcuts)
- Shop logic: `src/gameplay/economy/shop.rs`
- Shop UI (target file): `src/gameplay/economy/shop_ui.rs`
- Bottom bar spawning: `src/gameplay/hud/bottom_bar.rs`

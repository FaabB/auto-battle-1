# Keyboard Shortcuts for Shop (GAM-49) Implementation Plan

## Overview

Add keyboard shortcuts so players can select building cards (1/2/3/4) and reroll (R) without clicking, complementing the existing mouse-based shop UI. Extract shared logic to avoid duplication between mouse and keyboard handlers.

## Current State Analysis

- `shop_ui.rs` has `handle_card_click` (mouse card selection) and `handle_reroll_click` (mouse reroll) systems in `GameSet::Input` with `gameplay_running`
- `Shop` resource tracks `cards: [Option<BuildingType>; 4]` and `selected: Option<usize>`
- Card toggle logic is **inlined** in `handle_card_click` (`shop_ui.rs:44-51`) — no `Shop` method
- Reroll gold check + deduction is **inlined** in `handle_reroll_click` (`shop_ui.rs:64-68`) — `Shop::reroll()` doesn't touch `Gold`
- The codebase's dual-input pattern is the `Activate` observer in `widget.rs`, but shop cards use raw `Interaction` polling (not `widget::button()`)
- No `Digit1-4` or `KeyR` used anywhere in the codebase yet

### Key Discoveries:
- `handle_card_click` and `handle_reroll_click` both inline domain logic that will be duplicated if a keyboard system copies it
- `Shop::reroll_cost()` and `Shop::reroll()` exist but the gold guard sits in the system
- `Shop::selected` is a `pub` field written directly — no selection method exists
- Testing pattern: `create_base_test_app_no_input()` + `init_input_resources()` + manual `ButtonInput<KeyCode>.press()` (from `in_game.rs` tests)

## Desired End State

Pressing 1/2/3/4 selects the corresponding card slot (toggle behavior, same as clicking). Pressing R triggers reroll (same as clicking the reroll button). These work only during active gameplay (not while paused/in menus). No logic duplication between mouse and keyboard paths.

### How to verify:
- Run the game, enter InGame, press 1-4 to select cards, press R to reroll
- Verify selection highlight updates correctly
- Verify reroll deducts gold and regenerates cards
- Verify keys do nothing when paused

## What We're NOT Doing

- Migrating shop cards to the `Activate` observer pattern (larger refactor, not needed for this ticket)
- Key hint labels on the UI cards (can add later)
- Configurable key bindings
- Keyboard-based building placement (already mouse-based)

## Implementation Approach

1. Extract shared logic into `Shop` methods (`toggle_select`, `try_reroll`) so both mouse and keyboard systems call the same code
2. Refactor existing mouse handlers to use the new methods
3. Add a new `handle_shop_keyboard` system that also calls the shared methods
4. Tests for the keyboard system + tests for the new Shop methods

## Phase 1: Extract Shared Logic + Refactor Mouse Handlers

### Overview
Move card toggle and reroll-with-gold-check logic into `Shop` methods. Refactor existing click handlers to call them. No behavior change — pure refactor.

### Changes Required:

#### 1. Add `toggle_select` method to `Shop`
**File**: `src/gameplay/economy/shop.rs`
**Changes**: Add method after `selected_building()`

```rust
/// Toggle selection of a card slot. If the slot is empty, does nothing.
/// If already selected, deselects. Otherwise, selects it.
pub fn toggle_select(&mut self, slot: usize) {
    if self.cards.get(slot).is_some_and(|c| c.is_some()) {
        if self.selected == Some(slot) {
            self.selected = None;
        } else {
            self.selected = Some(slot);
        }
    }
}
```

#### 2. Add `try_reroll` method to `Shop`
**File**: `src/gameplay/economy/shop.rs`
**Changes**: Add method after `reroll()`

```rust
/// Attempt a reroll: check gold, deduct cost, and reroll cards.
/// Returns `true` if the reroll was performed, `false` if insufficient gold.
pub fn try_reroll(&mut self, gold: &mut u32) -> bool {
    let cost = self.reroll_cost();
    if *gold >= cost {
        *gold -= cost;
        self.reroll();
        true
    } else {
        false
    }
}
```

Note: takes `&mut u32` rather than `&mut Gold` to keep `Shop` decoupled from the `Gold` resource type.

#### 3. Refactor `handle_card_click` to use `toggle_select`
**File**: `src/gameplay/economy/shop_ui.rs`
**Changes**: Replace inlined toggle logic

```rust
fn handle_card_click(
    cards: Query<(&Interaction, &CardSlot), Changed<Interaction>>,
    mut shop: ResMut<Shop>,
) {
    for (interaction, slot) in &cards {
        if *interaction == Interaction::Pressed {
            shop.toggle_select(slot.0);
        }
    }
}
```

#### 4. Refactor `handle_reroll_click` to use `try_reroll`
**File**: `src/gameplay/economy/shop_ui.rs`
**Changes**: Replace inlined gold check + reroll

```rust
fn handle_reroll_click(
    reroll_btn: Query<&Interaction, (Changed<Interaction>, With<RerollButton>)>,
    mut shop: ResMut<Shop>,
    mut gold: ResMut<Gold>,
) {
    for interaction in &reroll_btn {
        if *interaction == Interaction::Pressed {
            shop.try_reroll(&mut gold.0);
        }
    }
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make check` passes
- [x] `make test` passes — all existing tests still green (behavior unchanged)
- [x] `make build` passes

#### Manual Verification:
- [ ] Mouse click card selection still works
- [ ] Mouse click reroll still works
- [ ] Gold deduction unchanged

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to Phase 2.

---

## Phase 2: Add Keyboard Input System

### Overview
Add `handle_shop_keyboard` system that calls the same `toggle_select` and `try_reroll` methods.

### Changes Required:

#### 1. New system in `shop_ui.rs`
**File**: `src/gameplay/economy/shop_ui.rs`
**Changes**: Add `handle_shop_keyboard` system

```rust
/// Handle keyboard shortcuts for card selection (1-4) and reroll (R).
fn handle_shop_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut shop: ResMut<Shop>,
    mut gold: ResMut<Gold>,
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
            shop.toggle_select(slot_index);
            return; // Only process one key per frame
        }
    }

    // Reroll: R key
    if keyboard.just_pressed(KeyCode::KeyR) {
        shop.try_reroll(&mut gold.0);
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
- [x] `make check` passes
- [x] `make test` passes
- [x] `make build` passes

#### Manual Verification:
- [ ] Press 1-4 during gameplay to select/deselect cards
- [ ] Press R during gameplay to reroll
- [ ] Reroll blocked when insufficient gold
- [ ] Keys do nothing when game is paused
- [ ] Mouse click selection still works alongside keyboard

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to Phase 3.

---

## Phase 3: Tests

### Overview
Add unit tests for the new `Shop` methods and the keyboard handler system.

### Changes Required:

#### 1. Unit tests for `toggle_select` and `try_reroll` in `shop.rs`
**File**: `src/gameplay/economy/shop.rs`
**Changes**: Add tests in the existing `mod tests` block

```rust
#[test]
fn toggle_select_selects_card() {
    let mut shop = Shop::default();
    shop.cards[1] = Some(BuildingType::Farm);
    shop.toggle_select(1);
    assert_eq!(shop.selected, Some(1));
}

#[test]
fn toggle_select_deselects_card() {
    let mut shop = Shop::default();
    shop.cards[2] = Some(BuildingType::Barracks);
    shop.selected = Some(2);
    shop.toggle_select(2);
    assert_eq!(shop.selected, None);
}

#[test]
fn toggle_select_switches_card() {
    let mut shop = Shop::default();
    shop.cards[0] = Some(BuildingType::Farm);
    shop.cards[1] = Some(BuildingType::Barracks);
    shop.selected = Some(0);
    shop.toggle_select(1);
    assert_eq!(shop.selected, Some(1));
}

#[test]
fn toggle_select_empty_slot_ignored() {
    let mut shop = Shop::default();
    shop.toggle_select(0);
    assert_eq!(shop.selected, None);
}

#[test]
fn try_reroll_deducts_gold_and_rerolls() {
    let mut shop = Shop::default();
    shop.generate_cards();
    shop.placed_since_last_reroll = false;
    shop.reroll(); // consecutive = 1, next cost = 5
    let mut gold = 200u32;

    let result = shop.try_reroll(&mut gold);

    assert!(result);
    assert_eq!(gold, 195);
    for (i, card) in shop.cards.iter().enumerate() {
        assert!(card.is_some(), "Card slot {i} should be filled");
    }
}

#[test]
fn try_reroll_blocked_insufficient_gold() {
    let mut shop = Shop::default();
    shop.placed_since_last_reroll = false;
    shop.consecutive_no_build_rerolls = 2; // cost = 10
    let old_cards = shop.cards;
    let mut gold = 5u32;

    let result = shop.try_reroll(&mut gold);

    assert!(!result);
    assert_eq!(gold, 5);
    assert_eq!(shop.cards, old_cards);
}

#[test]
fn try_reroll_free_after_placement() {
    let mut shop = Shop::default();
    shop.generate_cards();
    shop.placed_since_last_reroll = true;
    let mut gold = 200u32;

    let result = shop.try_reroll(&mut gold);

    assert!(result);
    assert_eq!(gold, 200); // Free reroll
}
```

#### 2. System-level tests for `handle_shop_keyboard` in `shop_ui.rs`
**File**: `src/gameplay/economy/shop_ui.rs`
**Changes**: Add tests in the existing `mod tests` block

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
    app.world_mut().resource_mut::<Shop>().cards[0] = Some(BuildingType::Barracks);

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit1);
    app.update();

    assert_eq!(app.world().resource::<Shop>().selected, Some(0));
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

    assert_eq!(app.world().resource::<Shop>().selected, None);
}

#[test]
fn keyboard_digit_empty_slot_ignored() {
    let mut app = create_keyboard_test_app();

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Digit1);
    app.update();

    assert_eq!(app.world().resource::<Shop>().selected, None);
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
    shop.reroll(); // consecutive = 1, cost = 5

    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::KeyR);
    app.update();

    assert_eq!(app.world().resource::<Gold>().0, STARTING_GOLD - 5);
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

    assert_eq!(app.world().resource::<Shop>().cards, old_cards);
    assert_eq!(app.world().resource::<Gold>().0, 5);
}
```

### Success Criteria:

#### Automated Verification:
- [x] `make test` passes — all 13 new tests green (7 shop.rs + 6 shop_ui.rs)
- [x] `make check` passes

---

## Testing Strategy

### Unit Tests (shop.rs):
- `toggle_select` — selects, deselects, switches, ignores empty
- `try_reroll` — deducts gold, blocks on insufficient gold, free after placement

### System Tests (shop_ui.rs):
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
- Dual-input pattern: `src/theme/widget.rs` (`Activate` observer — not used here but documents the codebase convention)

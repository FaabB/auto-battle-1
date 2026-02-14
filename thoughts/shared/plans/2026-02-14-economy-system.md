# Economy System (GAM-6) Implementation Plan

## Overview

Implement the economy system: Gold resource, building costs, Farm income, kill rewards, gold HUD, and shop UI with reroll. This replaces the hardcoded Barracks placement with a card-based shop where players draw random buildings and select which to place.

## Current State Analysis

- **No economy exists** — no Gold resource, no building costs, no kill rewards
- **Building placement hardcoded** to `BuildingType::Barracks` (`building/placement.rs:87`)
- **`ProductionTimer`** is Barracks-specific — Farm needs its own `IncomeTimer` component (idiomatic ECS composition)
- **Death system** (`combat/mod.rs:154`) is generic — despawns entities with HP <= 0, no events or rewards
- **No interactive UI buttons** — all UI is keyboard-based. Shop cards will be the first use of Bevy's `Interaction` component
- **GameSet ordering**: Input → Production → Ai → Movement → Combat → Death → Ui

### Key Discoveries:
- `Button` is a marker component requiring `Node`, `FocusPolicy::Block`, `Interaction` (`bevy_ui/widget/button.rs`)
- `Interaction` enum: `Pressed`, `Hovered`, `None` — auto-updated by `ui_focus_system` (included in `DefaultPlugins`)
- `check_death` queries all `Health` entities generically — kill rewards need a separate system that runs before it in `GameSet::Death`
- Building placement system reads `HoveredCell` resource + mouse click — clean hook point for gold cost checking

## Desired End State

After this plan is complete:
- Game starts with 200 gold displayed in a HUD
- Bottom panel shows 4 random building cards (drawn from Barracks/Farm pool)
- Click card to select → click grid to place (costs gold, removes card from shop)
- Reroll button refreshes all cards (free after building, escalating cost otherwise: 5 → 10 → 20 → 40)
- Can't place when insufficient gold, can't reroll when insufficient gold
- Farms generate 3 gold/sec via `IncomeTimer`
- Killing enemies awards 5 gold each

### Verification:
- `make check` passes (clippy + compile)
- `make test` passes with comprehensive integration tests
- Manual: gold ticks up from farms, gold deducted on placement, shop UI is interactive

## What We're NOT Doing

- Enemy type variety or stat scaling (GAM-7+)
- Fortress health/damage (GAM-8)
- Victory/defeat conditions (GAM-9)
- Sound effects or animations for gold changes
- Building sell/refund mechanic
- Sophisticated card pool weighting

## Implementation Approach

Two phases: (1) Economy backend + Farm income, (2) Shop UI. Phase 1 establishes the Gold resource, adds an `IncomeTimer` component for Farms (separate from `ProductionTimer` — idiomatic ECS composition), adds cost checking, kill rewards, and a gold HUD. Phase 2 adds the full shop UI with card selection and reroll.

## Verified API Patterns (Bevy 0.18)

These were verified against actual crate source:

- `Button` — marker component, `#[require(Node, FocusPolicy::Block, Interaction)]`. Spawning `Button` auto-adds these.
- `Interaction` — enum: `Pressed`, `Hovered`, `None`. Updated by `ui_focus_system` (part of `UiPlugin` in `DefaultPlugins`).
- `Changed<Interaction>` — valid query filter for detecting state changes.
- `Text::new(text)` + `TextFont { font_size, ..default() }` + `TextColor(color)` — verified in existing widget.rs.
- `BackgroundColor(Color)` — verified in existing widget.rs overlay.
- `Node { width, height, position_type, justify_content, align_items, .. }` — flexbox layout, verified in existing code.
- In tests with `MinimalPlugins`, `ui_focus_system` doesn't run — tests must manually set `Interaction` values or test systems in isolation.

---

## Phase 1: Economy Core + Farm Income

### Overview
Establish the Gold resource, add `IncomeTimer` component for Farms (separate from existing `ProductionTimer` — idiomatic ECS composition), add gold cost checking to placement, implement farm income and kill rewards, and display gold in a HUD.

### Changes Required:

#### 1. New module: `src/gameplay/economy/mod.rs`

**File**: `src/gameplay/economy/mod.rs`
**Changes**: Create new file — Gold resource, constants, cost helper, plugin compositor.

```rust
//! Economy: gold resource, building costs, income, and shop.

pub mod income;
pub mod shop;
mod ui;

use bevy::prelude::*;

use crate::gameplay::building::BuildingType;
use crate::screens::GameState;

// === Constants ===

/// Starting gold when entering InGame.
pub const STARTING_GOLD: u32 = 200;

/// Cost to place a Barracks.
pub const BARRACKS_COST: u32 = 100;

/// Cost to place a Farm.
pub const FARM_COST: u32 = 50;

/// Gold awarded per enemy kill.
pub const KILL_REWARD: u32 = 5;

/// Gold generated per Farm per tick.
pub const FARM_INCOME_PER_TICK: u32 = 3;

/// Farm income tick interval in seconds.
pub const FARM_INCOME_INTERVAL: f32 = 1.0;

// === Resources ===

/// The player's current gold.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct Gold(pub u32);

impl Default for Gold {
    fn default() -> Self {
        Self(STARTING_GOLD)
    }
}

// === Helper Functions ===

/// Get the gold cost for a building type.
#[must_use]
pub const fn building_cost(building_type: BuildingType) -> u32 {
    match building_type {
        BuildingType::Barracks => BARRACKS_COST,
        BuildingType::Farm => FARM_COST,
    }
}

// === Systems ===

fn reset_gold(mut gold: ResMut<Gold>) {
    gold.0 = STARTING_GOLD;
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Gold>()
        .init_resource::<Gold>();

    app.add_systems(OnEnter(GameState::InGame), reset_gold);

    // Sub-plugins
    income::plugin(app);
    shop::plugin(app);
    ui::plugin(app);
}
```

#### 2. New module: `src/gameplay/economy/income.rs`

**File**: `src/gameplay/economy/income.rs`
**Changes**: Create new file — kill reward system.

```rust
//! Income systems: kill rewards.

use bevy::prelude::*;

use super::Gold;
use crate::gameplay::units::{Health, Team};
use crate::screens::GameState;

/// Awards gold for each enemy that is about to die (Health <= 0).
/// Runs in `GameSet::Death` BEFORE `check_death` so entities still exist.
fn award_kill_gold(mut gold: ResMut<Gold>, query: Query<(&Health, &Team)>) {
    for (health, team) in &query {
        if health.current <= 0.0 && *team == Team::Enemy {
            gold.0 += super::KILL_REWARD;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        award_kill_gold
            .in_set(crate::GameSet::Death)
            .before(crate::gameplay::combat::check_death)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );
}
```

**Note**: `check_death` must be made `pub(crate)` so `income.rs` can reference it in `.before()`. Currently it's `fn check_death` (private). Change visibility to `pub(crate) fn check_death` in `combat/mod.rs`.

#### 3. New module: `src/gameplay/economy/ui.rs`

**File**: `src/gameplay/economy/ui.rs`
**Changes**: Create new file — gold HUD display.

```rust
//! Gold HUD display.

use bevy::prelude::*;

use super::Gold;
use crate::screens::GameState;

/// Marker for the gold display text entity.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
struct GoldDisplay;

fn spawn_gold_hud(mut commands: Commands) {
    commands.spawn((
        Text::new(format!("Gold: {}", super::STARTING_GOLD)),
        TextFont {
            font_size: 28.0,
            ..default()
        },
        TextColor(crate::theme::palette::GOLD_TEXT),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        },
        GoldDisplay,
        DespawnOnExit(GameState::InGame),
    ));
}

fn update_gold_display(gold: Res<Gold>, mut query: Single<&mut Text, With<GoldDisplay>>) {
    if gold.is_changed() {
        **query = Text::new(format!("Gold: {}", gold.0));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.register_type::<GoldDisplay>();

    app.add_systems(OnEnter(GameState::InGame), spawn_gold_hud);
    app.add_systems(
        Update,
        update_gold_display
            .in_set(crate::GameSet::Ui)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );
}
```

#### 4. Stub: `src/gameplay/economy/shop.rs`

**File**: `src/gameplay/economy/shop.rs`
**Changes**: Create stub — will be filled in Phase 2.

```rust
//! Shop: card selection, reroll, and building purchase.
//! Full implementation in Phase 2.

use bevy::prelude::*;

pub(super) fn plugin(_app: &mut App) {
    // Phase 2: Shop resource, card generation, selection, reroll
}
```

#### 5. Add `IncomeTimer` component + farm income system

Idiomatic ECS: separate components for separate behaviors. `ProductionTimer` stays unchanged for Barracks. `IncomeTimer` is a new component for Farms, with its own focused system.

**File**: `src/gameplay/economy/income.rs`
**Changes**: Add `IncomeTimer` component and `tick_farm_income` system alongside the existing `award_kill_gold`.

```rust
/// Timer for passive gold income (e.g., Farms).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct IncomeTimer(pub Timer);

/// Ticks income timers and adds gold when they fire.
/// Runs in `GameSet::Production`.
fn tick_farm_income(
    time: Res<Time>,
    mut farms: Query<&mut IncomeTimer>,
    mut gold: ResMut<Gold>,
) {
    for mut timer in &mut farms {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            gold.0 += super::FARM_INCOME_PER_TICK;
        }
    }
}
```

Register in the income plugin function:
```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<IncomeTimer>();

    app.add_systems(
        Update,
        tick_farm_income
            .in_set(crate::GameSet::Production)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );

    app.add_systems(
        Update,
        award_kill_gold
            .in_set(crate::GameSet::Death)
            .before(crate::gameplay::combat::check_death)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );
}
```

**File**: `src/gameplay/building/placement.rs`
**Changes**: Add `IncomeTimer` to Farm buildings when placed.

- Import `IncomeTimer` from economy module
- Keep existing `ProductionTimer` logic for Barracks (no change)
- Add `IncomeTimer` for Farm buildings:

```rust
// Barracks get ProductionTimer (unchanged)
if building_type == BuildingType::Barracks {
    entity_commands.insert(ProductionTimer(Timer::from_seconds(
        BARRACKS_PRODUCTION_INTERVAL,
        TimerMode::Repeating,
    )));
}

// Farms get IncomeTimer
if building_type == BuildingType::Farm {
    entity_commands.insert(crate::gameplay::economy::income::IncomeTimer(
        Timer::from_seconds(
            crate::gameplay::economy::FARM_INCOME_INTERVAL,
            TimerMode::Repeating,
        ),
    ));
}
```

**No changes needed** to `production.rs` or `ProductionTimer` — they stay exactly as they are.

#### 6. Add gold cost to building placement

**File**: `src/gameplay/building/placement.rs`
**Changes**: Check gold before placement, deduct on success.

Add `gold: ResMut<Gold>` parameter to `handle_building_placement`:
```rust
pub(super) fn handle_building_placement(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredCell>,
    grid_index: Res<GridIndex>,
    occupied: Query<(), With<Occupied>>,
    mut gold: ResMut<crate::gameplay::economy::Gold>,
) {
    // ... existing click + hover + occupied checks ...

    let building_type = BuildingType::Barracks; // Still hardcoded (Phase 2 replaces)
    let cost = crate::gameplay::economy::building_cost(building_type);

    // Check gold
    if gold.0 < cost {
        return;
    }

    // Deduct gold
    gold.0 -= cost;

    // ... rest of placement logic ...
}
```

#### 7. Update `gameplay/mod.rs` compositor

**File**: `src/gameplay/mod.rs`
**Changes**: Add economy module declaration and plugin registration.

```rust
pub(crate) mod economy;

// In plugin():
app.add_plugins((
    battlefield::plugin,
    building::plugin,
    combat::plugin,
    economy::plugin,
    units::plugin,
));
```

#### 8. Make `check_death` pub(crate)

**File**: `src/gameplay/combat/mod.rs`
**Changes**: Change visibility from `fn check_death` to `pub(crate) fn check_death` so `economy::income` can reference it in system ordering (`.before()`).

#### 9. Add gold text color to palette

**File**: `src/theme/palette.rs`
**Changes**: Add gold HUD color constant.

```rust
/// Gold/currency display text color (yellow-gold).
pub const GOLD_TEXT: Color = Color::srgb(1.0, 0.85, 0.0);
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes (clippy + compile)
- [ ] `make test` passes — all existing tests pass (no changes to `ProductionTimer`)
- [ ] New tests pass:
  - Gold resource initialized to 200 on enter InGame
  - Gold reset to 200 on re-enter InGame
  - `building_cost()` returns correct values
  - Building placement deducts gold
  - Building placement blocked when insufficient gold (0 gold → no building)
  - Farm `IncomeTimer` ticks and adds gold
  - Kill reward awards gold for enemy deaths
  - Kill reward does NOT award gold for player deaths
  - Gold HUD entity spawned on enter InGame
  - `award_kill_gold` runs before `check_death` (entity still exists when queried)

#### Manual Verification:
- [ ] Gold HUD shows "Gold: 200" at game start
- [ ] Placing a building (still hardcoded Barracks) deducts 100 gold from display
- [ ] Cannot place when gold < 100
- [ ] Killing enemies visibly increases gold
- [ ] Gold resets to 200 when re-entering InGame after quitting

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation before proceeding to Phase 2.

---

## Phase 2: Shop UI

### Overview
Add the shop system: 4 random building cards, click-to-select, placement removes card, and reroll with escalating costs. This replaces the hardcoded Barracks in `handle_building_placement` with the selected card from the shop.

### Changes Required:

#### 1. Shop resource and logic: `src/gameplay/economy/shop.rs`

**File**: `src/gameplay/economy/shop.rs`
**Changes**: Replace stub with full implementation.

```rust
//! Shop: card selection, reroll, and building purchase.

use bevy::prelude::*;

use crate::gameplay::building::BuildingType;
use crate::screens::GameState;

// === Constants ===

/// Number of card slots in the shop.
pub const HAND_SIZE: usize = 4;

/// Base reroll cost (before doubling).
const REROLL_BASE_COST: u32 = 5;

/// Maximum reroll cost (cap).
const MAX_REROLL_COST: u32 = 40;

/// Available building types in the card pool.
const BUILDING_POOL: [BuildingType; 2] = [BuildingType::Barracks, BuildingType::Farm];

// === Resources ===

/// The player's current shop offering of building cards.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct Shop {
    /// The 4 card slots. `None` = empty (already placed or not yet drawn).
    pub cards: [Option<BuildingType>; HAND_SIZE],
    /// Which slot is currently selected (0–3), or None.
    pub selected: Option<usize>,
    /// Number of consecutive rerolls without placing a building.
    pub consecutive_no_build_rerolls: u32,
    /// Whether the player placed a building since the last reroll.
    pub placed_since_last_reroll: bool,
}

impl Default for Shop {
    fn default() -> Self {
        Self {
            cards: [None; HAND_SIZE],
            selected: None,
            consecutive_no_build_rerolls: 0,
            placed_since_last_reroll: false,
        }
    }
}

impl Shop {
    /// Generate new random cards for all slots.
    pub fn generate_cards(&mut self) {
        use rand::Rng;
        let mut rng = rand::rng();
        for card in &mut self.cards {
            let idx = rng.random_range(0..BUILDING_POOL.len());
            *card = Some(BUILDING_POOL[idx]);
        }
        self.selected = None;
    }

    /// Get the currently selected building type, if any.
    #[must_use]
    pub fn selected_building(&self) -> Option<BuildingType> {
        self.selected
            .and_then(|idx| self.cards.get(idx).copied().flatten())
    }

    /// Remove the selected card after placement.
    pub fn remove_selected(&mut self) {
        if let Some(idx) = self.selected {
            self.cards[idx] = None;
            self.selected = None;
            self.placed_since_last_reroll = true;
            self.consecutive_no_build_rerolls = 0;
        }
    }

    /// Get the current reroll cost.
    /// Free after placing a building, otherwise 5 * 2^(n-1) capped at 40.
    #[must_use]
    pub fn reroll_cost(&self) -> u32 {
        if self.placed_since_last_reroll {
            0
        } else if self.consecutive_no_build_rerolls == 0 {
            0
        } else {
            (REROLL_BASE_COST << (self.consecutive_no_build_rerolls - 1)).min(MAX_REROLL_COST)
        }
    }

    /// Perform a reroll: pay cost, regenerate cards, update state.
    pub fn reroll(&mut self) {
        if !self.placed_since_last_reroll {
            self.consecutive_no_build_rerolls += 1;
        }
        self.placed_since_last_reroll = false;
        self.generate_cards();
    }
}

// === Systems ===

fn initialize_shop(mut shop: ResMut<Shop>) {
    shop.generate_cards();
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Shop>()
        .init_resource::<Shop>();

    app.add_systems(OnEnter(GameState::InGame), initialize_shop);
}
```

**Note on randomness**: Bevy doesn't bundle `rand`. We need to add `rand` as a dependency. Check `Cargo.toml`:

```toml
[dependencies]
rand = "0.9"
```

#### 2. Shop UI: `src/gameplay/economy/shop_ui.rs`

**File**: `src/gameplay/economy/shop_ui.rs`
**Changes**: Create new file — bottom panel UI with card buttons and reroll button.

**UI Layout** (bottom of screen):
```
┌──────────────────────────────────────────────────────┐
│  [Card 0]  [Card 1]  [Card 2]  [Card 3]  [Reroll]  │
│  Barracks  Farm      Barracks  Farm       FREE/5g   │
│  100g      50g       100g      50g                   │
└──────────────────────────────────────────────────────┘
```

Key components:
- `CardSlot(usize)` — marker on each card button entity, stores slot index
- `RerollButton` — marker on the reroll button entity
- `ShopPanel` — marker on the root panel entity

```rust
//! Shop UI: bottom panel with card buttons and reroll button.

use bevy::prelude::*;

use super::shop::{HAND_SIZE, Shop};
use super::Gold;
use crate::gameplay::building::BuildingType;
use crate::screens::GameState;

// === Constants ===

const CARD_WIDTH: f32 = 120.0;
const CARD_HEIGHT: f32 = 80.0;
const CARD_GAP: f32 = 10.0;
const PANEL_PADDING: f32 = 12.0;
const PANEL_BG_COLOR: Color = Color::srgba(0.1, 0.1, 0.15, 0.9);
const CARD_BG_COLOR: Color = Color::srgb(0.2, 0.2, 0.3);
const CARD_SELECTED_COLOR: Color = Color::srgb(0.3, 0.5, 0.3);
const CARD_EMPTY_COLOR: Color = Color::srgb(0.15, 0.15, 0.15);
const CARD_HOVER_COLOR: Color = Color::srgb(0.3, 0.3, 0.4);
const REROLL_BG_COLOR: Color = Color::srgb(0.4, 0.25, 0.1);
const CARD_TEXT_SIZE: f32 = 16.0;
const COST_TEXT_SIZE: f32 = 14.0;

// === Components ===

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
struct ShopPanel;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
struct CardSlot(usize);

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
struct CardNameText(usize);

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
struct CardCostText(usize);

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
struct RerollButton;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
struct RerollCostText;

// === Systems ===

fn spawn_shop_panel(mut commands: Commands) {
    // Root panel — fixed at bottom center
    commands
        .spawn((
            ShopPanel,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                left: Val::Percent(50.0),
                // Translate left by 50% of own width to center
                // (Bevy doesn't have transform: translateX(-50%), so we use a wrapper)
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(CARD_GAP),
                padding: UiRect::all(Val::Px(PANEL_PADDING)),
                ..default()
            },
            BackgroundColor(PANEL_BG_COLOR),
            DespawnOnExit(GameState::InGame),
        ))
        .with_children(|parent| {
            // 4 card slots
            for i in 0..HAND_SIZE {
                parent
                    .spawn((
                        CardSlot(i),
                        Button,
                        Node {
                            width: Val::Px(CARD_WIDTH),
                            height: Val::Px(CARD_HEIGHT),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(4.0),
                            ..default()
                        },
                        BackgroundColor(CARD_BG_COLOR),
                    ))
                    .with_children(|card| {
                        // Building name
                        card.spawn((
                            CardNameText(i),
                            Text::new("—"),
                            TextFont {
                                font_size: CARD_TEXT_SIZE,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                        // Cost
                        card.spawn((
                            CardCostText(i),
                            Text::new(""),
                            TextFont {
                                font_size: COST_TEXT_SIZE,
                                ..default()
                            },
                            TextColor(crate::theme::palette::GOLD_TEXT),
                        ));
                    });
            }

            // Reroll button
            parent
                .spawn((
                    RerollButton,
                    Button,
                    Node {
                        width: Val::Px(90.0),
                        height: Val::Px(CARD_HEIGHT),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(REROLL_BG_COLOR),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        RerollCostText,
                        Text::new("Reroll\nFREE"),
                        TextFont {
                            font_size: COST_TEXT_SIZE,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TextLayout::new_with_justify(JustifyText::Center),
                    ));
                });
        });
}

/// Handle card button clicks — select the clicked card.
fn handle_card_click(
    cards: Query<(&Interaction, &CardSlot), Changed<Interaction>>,
    mut shop: ResMut<Shop>,
) {
    for (interaction, slot) in &cards {
        if *interaction == Interaction::Pressed {
            // Only select if the slot has a card
            if shop.cards[slot.0].is_some() {
                // Toggle selection: click same card deselects
                if shop.selected == Some(slot.0) {
                    shop.selected = None;
                } else {
                    shop.selected = Some(slot.0);
                }
            }
        }
    }
}

/// Handle reroll button click.
fn handle_reroll_click(
    reroll_btn: Query<&Interaction, (Changed<Interaction>, With<RerollButton>)>,
    mut shop: ResMut<Shop>,
    mut gold: ResMut<Gold>,
) {
    for interaction in &reroll_btn {
        if *interaction == Interaction::Pressed {
            let cost = shop.reroll_cost();
            if gold.0 >= cost {
                gold.0 -= cost;
                shop.reroll();
            }
        }
    }
}

/// Update card button visuals based on Shop state.
fn update_card_visuals(
    shop: Res<Shop>,
    mut cards: Query<(&CardSlot, &Interaction, &mut BackgroundColor)>,
) {
    for (slot, interaction, mut bg) in &mut cards {
        let is_selected = shop.selected == Some(slot.0);
        let has_card = shop.cards[slot.0].is_some();

        *bg = if !has_card {
            BackgroundColor(CARD_EMPTY_COLOR)
        } else if is_selected {
            BackgroundColor(CARD_SELECTED_COLOR)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(CARD_HOVER_COLOR)
        } else {
            BackgroundColor(CARD_BG_COLOR)
        };
    }
}

/// Update card text content when shop changes.
fn update_card_text(
    shop: Res<Shop>,
    mut name_query: Query<(&CardNameText, &mut Text)>,
    mut cost_query: Query<(&CardCostText, &mut Text), Without<CardNameText>>,
) {
    if !shop.is_changed() {
        return;
    }

    for (name_text, mut text) in &mut name_query {
        let slot = name_text.0;
        *text = Text::new(match shop.cards[slot] {
            Some(BuildingType::Barracks) => "Barracks",
            Some(BuildingType::Farm) => "Farm",
            None => "—",
        });
    }

    for (cost_text, mut text) in &mut cost_query {
        let slot = cost_text.0;
        *text = Text::new(match shop.cards[slot] {
            Some(bt) => format!("{}g", super::building_cost(bt)),
            None => String::new(),
        });
    }
}

/// Update reroll button text with current cost.
fn update_reroll_text(shop: Res<Shop>, mut query: Query<&mut Text, With<RerollCostText>>) {
    if !shop.is_changed() {
        return;
    }

    for mut text in &mut query {
        let cost = shop.reroll_cost();
        *text = if cost == 0 {
            Text::new("Reroll\nFREE")
        } else {
            Text::new(format!("Reroll\n{cost}g"))
        };
    }
}

pub(super) fn plugin(app: &mut App) {
    app.register_type::<ShopPanel>()
        .register_type::<CardSlot>()
        .register_type::<CardNameText>()
        .register_type::<CardCostText>()
        .register_type::<RerollButton>()
        .register_type::<RerollCostText>();

    app.add_systems(OnEnter(GameState::InGame), spawn_shop_panel);

    app.add_systems(
        Update,
        (handle_card_click, handle_reroll_click)
            .in_set(crate::GameSet::Input)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );

    app.add_systems(
        Update,
        (update_card_visuals, update_card_text, update_reroll_text)
            .in_set(crate::GameSet::Ui)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );
}
```

#### 3. Update `economy/mod.rs` to include `shop_ui`

**File**: `src/gameplay/economy/mod.rs`
**Changes**: Add `mod shop_ui;` and call `shop_ui::plugin(app);` in the plugin function.

```rust
mod shop_ui;

// In plugin():
shop_ui::plugin(app);
```

#### 4. Replace hardcoded Barracks with shop selection

**File**: `src/gameplay/building/placement.rs`
**Changes**: Use `Shop.selected_building()` instead of hardcoded `BuildingType::Barracks`.

```rust
pub(super) fn handle_building_placement(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredCell>,
    grid_index: Res<GridIndex>,
    occupied: Query<(), With<Occupied>>,
    mut gold: ResMut<crate::gameplay::economy::Gold>,
    mut shop: ResMut<crate::gameplay::economy::shop::Shop>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Some((col, row)) = hovered.0 else {
        return;
    };

    let Some(slot_entity) = grid_index.get(col, row) else {
        return;
    };

    if occupied.contains(slot_entity) {
        return;
    }

    // Get selected building from shop
    let Some(building_type) = shop.selected_building() else {
        return; // No card selected
    };

    // Check gold
    let cost = crate::gameplay::economy::building_cost(building_type);
    if gold.0 < cost {
        return;
    }

    // Deduct gold and remove card from shop
    gold.0 -= cost;
    shop.remove_selected();

    // Mark slot as occupied
    commands.entity(slot_entity).insert(Occupied);

    // Spawn the building entity
    let world_x = col_to_world_x(BUILD_ZONE_START_COL + col);
    let world_y = row_to_world_y(row);

    let mut entity_commands = commands.spawn((
        Building {
            building_type,
            grid_col: col,
            grid_row: row,
        },
        Team::Player,
        Target,
        Sprite::from_color(
            building_color(building_type),
            Vec2::splat(BUILDING_SPRITE_SIZE),
        ),
        Transform::from_xyz(world_x, world_y, Z_BUILDING),
        DespawnOnExit(GameState::InGame),
    ));

    // Each building type gets its own timer component (idiomatic ECS composition)
    match building_type {
        BuildingType::Barracks => {
            entity_commands.insert(ProductionTimer(Timer::from_seconds(
                BARRACKS_PRODUCTION_INTERVAL,
                TimerMode::Repeating,
            )));
        }
        BuildingType::Farm => {
            entity_commands.insert(crate::gameplay::economy::income::IncomeTimer(
                Timer::from_seconds(
                    crate::gameplay::economy::FARM_INCOME_INTERVAL,
                    TimerMode::Repeating,
                ),
            ));
        }
    }
}
```

### Success Criteria:

#### Automated Verification:
- [ ] `make check` passes
- [ ] `make test` passes — all tests including:
  - `Shop::generate_cards()` fills all 4 slots
  - `Shop::selected_building()` returns correct type
  - `Shop::remove_selected()` clears slot and resets state
  - `Shop::reroll_cost()` returns 0 after building, escalates otherwise
  - `Shop::reroll()` regenerates cards and updates cost state
  - Card click selects card (integration test with mocked Interaction)
  - Reroll click regenerates cards and deducts gold
  - Reroll blocked when insufficient gold
  - Placement uses selected card type, not hardcoded Barracks
  - Card removed from shop after placement
  - Empty slot can't be selected
  - Reroll cost resets after placing a building

#### Manual Verification:
- [ ] Bottom panel shows 4 cards with building names and costs
- [ ] Clicking a card highlights it (selected state)
- [ ] Clicking the grid places the selected building (correct type and color)
- [ ] Placed card disappears from shop (slot becomes empty/dark)
- [ ] Reroll button refreshes all 4 cards
- [ ] Reroll shows "FREE" after placing a building, then escalating cost
- [ ] Can't place when no card selected
- [ ] Can't place when insufficient gold
- [ ] Can't reroll when insufficient gold
- [ ] Farms placed from shop generate income

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation.

---

## Testing Strategy

### Unit Tests:
- `building_cost()` returns correct values for each building type
- `Gold::default()` starts at STARTING_GOLD
- `Shop::generate_cards()` fills all slots with `Some(BuildingType)`
- `Shop::selected_building()` returns None when no selection, correct type when selected
- `Shop::remove_selected()` clears card, sets placed flag, resets escalation
- `Shop::reroll_cost()` cost escalation: 0 after build, 5/10/20/40 consecutive
- `Shop::reroll()` regenerates and increments no-build counter
- Constants validation (costs > 0, intervals > 0, etc.)

### Integration Tests:
- Gold resource initialized on enter InGame
- Gold deducted on building placement
- Placement blocked with insufficient gold (gold stays at 0)
- Farm `IncomeTimer` adds gold per tick
- Kill reward awards gold for enemy death
- Kill reward ignores player deaths
- Card click sets Shop.selected
- Reroll regenerates cards and deducts gold
- Placement removes selected card from shop
- Gold HUD entity spawned and displays correct value

### Test Helpers:
- `create_economy_test_app()` — base app + economy plugin + states, transitioned to InGame
- Reuse `create_placement_test_app()` pattern but add Gold and Shop resources

## Performance Considerations

- `update_gold_display` uses `gold.is_changed()` to skip unnecessary text updates
- `update_card_text` and `update_reroll_text` use `shop.is_changed()` guard
- Card visual updates happen every frame (due to hover tracking) but are cheap (just BackgroundColor)
- Farm income and kill rewards are O(n) over buildings/dying entities — negligible for prototype scale

## References

- Linear ticket: GAM-6
- Dependent ticket: GAM-7 (Enemy Stream) — needs Gold + kill rewards to exist
- Building placement: `src/gameplay/building/placement.rs`
- Death system: `src/gameplay/combat/mod.rs:154`
- Theme widgets: `src/theme/widget.rs`
- Production system: `src/gameplay/building/production.rs`

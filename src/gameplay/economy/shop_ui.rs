//! Shop UI: bottom panel with card buttons and reroll button.

use bevy::prelude::*;

use super::Gold;
use super::shop::{HAND_SIZE, Shop};
use crate::gameplay::building::BuildingType;
use crate::screens::GameState;
use crate::{GameSet, gameplay_running};

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
            Name::new("Shop Panel"),
            ShopPanel,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(10.0),
                left: Val::Percent(50.0),
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
                        Name::new(format!("Card Slot {i}")),
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
                            Name::new(format!("Card {i} Name")),
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
                            Name::new(format!("Card {i} Cost")),
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
                    Name::new("Reroll Button"),
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
                        Name::new("Reroll Text"),
                        RerollCostText,
                        Text::new("Reroll\nFREE"),
                        TextFont {
                            font_size: COST_TEXT_SIZE,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TextLayout::new_with_justify(Justify::Center),
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
        *text = Text::new(
            shop.cards[slot]
                .map_or_else(String::new, |bt| format!("{}g", super::building_cost(bt))),
        );
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

// === Plugin ===

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
            .in_set(GameSet::Input)
            .run_if(gameplay_running),
    );

    app.add_systems(
        Update,
        (update_card_visuals, update_card_text, update_reroll_text)
            .in_set(GameSet::Ui)
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::economy::shop::Shop;
    use pretty_assertions::assert_eq;

    // In MinimalPlugins, ui_focus_system doesn't run, so Interaction
    // doesn't auto-update. We test systems in isolation by manually
    // setting Interaction values.

    fn create_card_click_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Shop>();
        app.add_systems(Update, handle_card_click);
        app
    }

    fn create_reroll_click_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<Shop>();
        app.init_resource::<Gold>();
        app.add_systems(Update, handle_reroll_click);
        app
    }

    #[test]
    fn card_click_selects_card() {
        let mut app = create_card_click_test_app();

        // Pre-fill shop cards
        let mut shop = app.world_mut().resource_mut::<Shop>();
        shop.cards = [
            Some(BuildingType::Barracks),
            Some(BuildingType::Farm),
            Some(BuildingType::Barracks),
            Some(BuildingType::Farm),
        ];

        // Spawn a card button with Pressed interaction
        app.world_mut().spawn((CardSlot(1), Interaction::Pressed));
        app.update();

        let shop = app.world().resource::<Shop>();
        assert_eq!(shop.selected, Some(1));
    }

    #[test]
    fn card_click_toggles_selection() {
        let mut app = create_card_click_test_app();

        let mut shop = app.world_mut().resource_mut::<Shop>();
        shop.cards[2] = Some(BuildingType::Barracks);
        shop.selected = Some(2); // Already selected

        // Click the same card
        app.world_mut().spawn((CardSlot(2), Interaction::Pressed));
        app.update();

        let shop = app.world().resource::<Shop>();
        assert_eq!(shop.selected, None); // Deselected
    }

    #[test]
    fn card_click_empty_slot_ignored() {
        let mut app = create_card_click_test_app();

        // Shop cards are all None by default

        app.world_mut().spawn((CardSlot(0), Interaction::Pressed));
        app.update();

        let shop = app.world().resource::<Shop>();
        assert_eq!(shop.selected, None);
    }

    #[test]
    fn reroll_click_regenerates_cards_and_deducts_gold() {
        let mut app = create_reroll_click_test_app();

        // Generate initial cards so the shop has content
        app.world_mut().resource_mut::<Shop>().generate_cards();

        // Do one no-build reroll to make next reroll cost 5
        app.world_mut().resource_mut::<Shop>().reroll();
        let initial_gold = app.world().resource::<Gold>().0;

        // Click reroll button
        app.world_mut().spawn((RerollButton, Interaction::Pressed));
        app.update();

        let shop = app.world().resource::<Shop>();
        let gold = app.world().resource::<Gold>();

        // Cards regenerated (all slots filled)
        for (i, card) in shop.cards.iter().enumerate() {
            assert!(
                card.is_some(),
                "Card slot {i} should be filled after reroll"
            );
        }

        // Gold deducted (5g for second no-build reroll)
        assert_eq!(gold.0, initial_gold - 5);
    }

    #[test]
    fn reroll_click_blocked_when_insufficient_gold() {
        let mut app = create_reroll_click_test_app();

        // Set up shop: 2 no-build rerolls so cost = 10
        let mut shop = app.world_mut().resource_mut::<Shop>();
        shop.placed_since_last_reroll = false;
        shop.consecutive_no_build_rerolls = 2;
        let old_cards = shop.cards;

        // Set gold below reroll cost
        app.world_mut().resource_mut::<Gold>().0 = 5; // Cost is 10

        app.world_mut().spawn((RerollButton, Interaction::Pressed));
        app.update();

        let shop = app.world().resource::<Shop>();
        let gold = app.world().resource::<Gold>();

        // Cards unchanged, gold unchanged
        assert_eq!(shop.cards, old_cards);
        assert_eq!(gold.0, 5);
    }

    #[test]
    fn no_placement_without_card_selected() {
        // This tests the placement system path, but we verify via
        // the shop's selected_building returning None.
        let shop = Shop::default();
        assert!(shop.selected_building().is_none());
    }
}

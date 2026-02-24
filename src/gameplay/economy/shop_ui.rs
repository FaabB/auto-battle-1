//! Shop UI: interaction and visual update systems for card slots and reroll button.
//!
//! Spawning is handled by `gameplay/hud/bottom_bar.rs`.

use bevy::prelude::*;

use super::Gold;
use super::shop::Shop;
use crate::theme::palette;
use crate::{GameSet, gameplay_running};

// === Components ===

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct CardSlot(pub usize);

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct CardNameText(pub usize);

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct CardCostText(pub usize);

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct RerollButton;

#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct RerollCostText;

// === Systems ===

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
            BackgroundColor(palette::CARD_EMPTY)
        } else if is_selected {
            BackgroundColor(palette::CARD_SELECTED)
        } else if *interaction == Interaction::Hovered {
            BackgroundColor(palette::CARD_HOVER)
        } else {
            BackgroundColor(palette::CARD_BACKGROUND)
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
        *text = Text::new(shop.cards[slot].map_or("—", |bt| bt.display_name()));
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
    app.register_type::<CardSlot>()
        .register_type::<CardNameText>()
        .register_type::<CardCostText>()
        .register_type::<RerollButton>()
        .register_type::<RerollCostText>();

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
    use crate::gameplay::building::BuildingType;
    use crate::gameplay::economy::shop::Shop;
    use pretty_assertions::assert_eq;

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

        let mut shop = app.world_mut().resource_mut::<Shop>();
        shop.cards = [
            Some(BuildingType::Barracks),
            Some(BuildingType::Farm),
            Some(BuildingType::Barracks),
            Some(BuildingType::Farm),
        ];

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
        shop.selected = Some(2);

        app.world_mut().spawn((CardSlot(2), Interaction::Pressed));
        app.update();

        let shop = app.world().resource::<Shop>();
        assert_eq!(shop.selected, None);
    }

    #[test]
    fn card_click_empty_slot_ignored() {
        let mut app = create_card_click_test_app();

        app.world_mut().spawn((CardSlot(0), Interaction::Pressed));
        app.update();

        let shop = app.world().resource::<Shop>();
        assert_eq!(shop.selected, None);
    }

    #[test]
    fn reroll_click_regenerates_cards_and_deducts_gold() {
        let mut app = create_reroll_click_test_app();

        app.world_mut().resource_mut::<Shop>().generate_cards();
        app.world_mut().resource_mut::<Shop>().reroll();
        let initial_gold = app.world().resource::<Gold>().0;

        app.world_mut().spawn((RerollButton, Interaction::Pressed));
        app.update();

        let shop = app.world().resource::<Shop>();
        let gold = app.world().resource::<Gold>();

        for (i, card) in shop.cards.iter().enumerate() {
            assert!(
                card.is_some(),
                "Card slot {i} should be filled after reroll"
            );
        }

        assert_eq!(gold.0, initial_gold - 5);
    }

    #[test]
    fn reroll_click_blocked_when_insufficient_gold() {
        let mut app = create_reroll_click_test_app();

        let mut shop = app.world_mut().resource_mut::<Shop>();
        shop.placed_since_last_reroll = false;
        shop.consecutive_no_build_rerolls = 2;
        let old_cards = shop.cards;

        app.world_mut().resource_mut::<Gold>().0 = 5;

        app.world_mut().spawn((RerollButton, Interaction::Pressed));
        app.update();

        let shop = app.world().resource::<Shop>();
        let gold = app.world().resource::<Gold>();

        assert_eq!(shop.cards, old_cards);
        assert_eq!(gold.0, 5);
    }

    #[test]
    fn no_placement_without_card_selected() {
        let shop = Shop::default();
        assert!(shop.selected_building().is_none());
    }
}

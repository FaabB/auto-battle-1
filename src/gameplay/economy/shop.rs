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


// === Resources ===

/// The player's current shop offering of building cards.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct Shop {
    /// The 4 card slots. `None` = empty (already placed or not yet drawn).
    pub cards: [Option<BuildingType>; HAND_SIZE],
    /// Which slot is currently selected (0-3), or `None`.
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
        let pool = BuildingType::ALL;
        for card in &mut self.cards {
            let idx = rng.random_range(0..pool.len());
            *card = Some(pool[idx]);
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
    pub const fn remove_selected(&mut self) {
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
        if self.placed_since_last_reroll || self.consecutive_no_build_rerolls == 0 {
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
    *shop = Shop::default();
    shop.generate_cards();
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Shop>().init_resource::<Shop>();

    app.add_systems(OnEnter(GameState::InGame), initialize_shop);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn generate_cards_fills_all_slots() {
        let mut shop = Shop::default();
        shop.generate_cards();

        for (i, card) in shop.cards.iter().enumerate() {
            assert!(card.is_some(), "Card slot {i} should be filled");
        }
    }

    #[test]
    fn generate_cards_only_uses_pool() {
        let mut shop = Shop::default();
        shop.generate_cards();

        for card in &shop.cards {
            let bt = card.unwrap();
            assert!(
                BuildingType::ALL.contains(&bt),
                "Card should be in BuildingType::ALL, got {bt:?}"
            );
        }
    }

    #[test]
    fn generate_cards_clears_selection() {
        let mut shop = Shop::default();
        shop.selected = Some(2);
        shop.generate_cards();
        assert_eq!(shop.selected, None);
    }

    #[test]
    fn selected_building_returns_none_when_no_selection() {
        let mut shop = Shop::default();
        shop.generate_cards();
        assert!(shop.selected_building().is_none());
    }

    #[test]
    fn selected_building_returns_correct_type() {
        let mut shop = Shop::default();
        shop.cards = [
            Some(BuildingType::Farm),
            Some(BuildingType::Barracks),
            None,
            Some(BuildingType::Farm),
        ];
        shop.selected = Some(1);
        assert_eq!(shop.selected_building(), Some(BuildingType::Barracks));
    }

    #[test]
    fn selected_building_returns_none_for_empty_slot() {
        let mut shop = Shop::default();
        shop.cards = [None, None, None, None];
        shop.selected = Some(0);
        assert!(shop.selected_building().is_none());
    }

    #[test]
    fn remove_selected_clears_card_and_selection() {
        let mut shop = Shop::default();
        shop.generate_cards();
        shop.selected = Some(1);
        shop.remove_selected();

        assert!(shop.cards[1].is_none());
        assert_eq!(shop.selected, None);
    }

    #[test]
    fn remove_selected_sets_placed_flag() {
        let mut shop = Shop::default();
        shop.generate_cards();
        shop.selected = Some(0);
        shop.remove_selected();

        assert!(shop.placed_since_last_reroll);
        assert_eq!(shop.consecutive_no_build_rerolls, 0);
    }

    #[test]
    fn reroll_cost_free_initially() {
        let shop = Shop::default();
        assert_eq!(shop.reroll_cost(), 0);
    }

    #[test]
    fn reroll_cost_free_after_placing() {
        let mut shop = Shop::default();
        shop.placed_since_last_reroll = true;
        assert_eq!(shop.reroll_cost(), 0);
    }

    #[test]
    fn reroll_cost_escalates() {
        let mut shop = Shop::default();
        shop.placed_since_last_reroll = false;

        // First no-build reroll: 5
        shop.consecutive_no_build_rerolls = 1;
        assert_eq!(shop.reroll_cost(), 5);

        // Second: 10
        shop.consecutive_no_build_rerolls = 2;
        assert_eq!(shop.reroll_cost(), 10);

        // Third: 20
        shop.consecutive_no_build_rerolls = 3;
        assert_eq!(shop.reroll_cost(), 20);

        // Fourth: 40 (cap)
        shop.consecutive_no_build_rerolls = 4;
        assert_eq!(shop.reroll_cost(), 40);

        // Fifth: still 40 (cap)
        shop.consecutive_no_build_rerolls = 5;
        assert_eq!(shop.reroll_cost(), 40);
    }

    #[test]
    fn reroll_increments_no_build_counter() {
        let mut shop = Shop::default();
        shop.placed_since_last_reroll = false;
        shop.reroll();

        assert_eq!(shop.consecutive_no_build_rerolls, 1);
    }

    #[test]
    fn reroll_does_not_increment_after_placing() {
        let mut shop = Shop::default();
        shop.placed_since_last_reroll = true;
        shop.reroll();

        assert_eq!(shop.consecutive_no_build_rerolls, 0);
    }

    #[test]
    fn reroll_clears_placed_flag() {
        let mut shop = Shop::default();
        shop.placed_since_last_reroll = true;
        shop.reroll();

        assert!(!shop.placed_since_last_reroll);
    }

    #[test]
    fn reroll_regenerates_cards() {
        let mut shop = Shop::default();
        // Start with empty cards
        assert!(shop.cards.iter().all(Option::is_none));

        shop.reroll();

        // All cards should be filled
        for (i, card) in shop.cards.iter().enumerate() {
            assert!(
                card.is_some(),
                "Card slot {i} should be filled after reroll"
            );
        }
    }

    #[test]
    fn reroll_cost_resets_after_placement() {
        let mut shop = Shop::default();
        shop.placed_since_last_reroll = false;

        // Reroll twice without placing
        shop.reroll();
        shop.reroll();
        assert_eq!(shop.reroll_cost(), 10); // 5 * 2^1

        // Place a building
        shop.cards[0] = Some(BuildingType::Barracks);
        shop.selected = Some(0);
        shop.remove_selected();

        // Cost should be free after placing
        assert_eq!(shop.reroll_cost(), 0);
    }
}

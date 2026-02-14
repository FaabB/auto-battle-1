//! Economy: gold resource, building costs, income, and shop.

pub mod income;
pub mod shop;
mod shop_ui;
mod ui;

use bevy::prelude::*;

use crate::gameplay::building::BuildingType;
use crate::screens::GameState;

// === Constants ===

/// Starting gold when entering `InGame`.
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
    app.register_type::<Gold>().init_resource::<Gold>();

    app.add_systems(OnEnter(GameState::InGame), reset_gold);

    // Sub-plugins
    income::plugin(app);
    shop::plugin(app);
    shop_ui::plugin(app);
    ui::plugin(app);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn gold_default_is_starting_gold() {
        let gold = Gold::default();
        assert_eq!(gold.0, STARTING_GOLD);
    }

    #[test]
    fn building_cost_barracks() {
        assert_eq!(building_cost(BuildingType::Barracks), BARRACKS_COST);
    }

    #[test]
    fn building_cost_farm() {
        assert_eq!(building_cost(BuildingType::Farm), FARM_COST);
    }

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn constants_are_valid() {
        assert!(STARTING_GOLD > 0);
        assert!(BARRACKS_COST > 0);
        assert!(FARM_COST > 0);
        assert!(KILL_REWARD > 0);
        assert!(FARM_INCOME_PER_TICK > 0);
        assert!(FARM_INCOME_INTERVAL > 0.0);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::testing::{assert_entity_count, transition_to_ingame};
    use pretty_assertions::assert_eq;

    #[test]
    fn gold_initialized_on_enter_ingame() {
        let mut app = crate::testing::create_base_test_app();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();
        app.add_plugins(crate::gameplay::plugin);
        transition_to_ingame(&mut app);

        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, STARTING_GOLD);
    }

    #[test]
    fn gold_reset_on_reenter_ingame() {
        let mut app = crate::testing::create_base_test_app();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();
        app.add_plugins(crate::gameplay::plugin);
        transition_to_ingame(&mut app);

        // Modify gold
        app.world_mut().resource_mut::<Gold>().0 = 50;
        assert_eq!(app.world().resource::<Gold>().0, 50);

        // Transition away and back
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::MainMenu);
        app.update();
        app.update();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::InGame);
        app.update();
        app.update();

        let gold = app.world().resource::<Gold>();
        assert_eq!(gold.0, STARTING_GOLD);
    }

    #[test]
    fn gold_hud_spawned_on_enter_ingame() {
        let mut app = crate::testing::create_base_test_app();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();
        app.add_plugins(crate::gameplay::plugin);
        transition_to_ingame(&mut app);

        // The gold HUD is a Text entity with DespawnOnExit<GameState>
        assert_entity_count::<(With<Text>, With<DespawnOnExit<GameState>>)>(&mut app, 1);
    }
}

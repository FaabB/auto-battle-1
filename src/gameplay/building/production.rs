//! Building production: timer ticking and unit spawning.

use bevy::prelude::*;

use super::ProductionTimer;
use crate::Z_UNIT;
use crate::gameplay::battlefield::CELL_SIZE;
use crate::gameplay::combat::{
    AttackTimer, HealthBarConfig, UNIT_HEALTH_BAR_HEIGHT, UNIT_HEALTH_BAR_WIDTH,
    UNIT_HEALTH_BAR_Y_OFFSET,
};
use crate::gameplay::units::{
    CombatStats, CurrentTarget, Movement, SOLDIER_ATTACK_RANGE, SOLDIER_ATTACK_SPEED,
    SOLDIER_DAMAGE, SOLDIER_HEALTH, SOLDIER_MOVE_SPEED, Unit, UnitAssets,
};
use crate::gameplay::{Health, Target, Team};
use crate::screens::GameState;

/// Ticks production timers on all buildings and spawns units when timers fire.
pub(super) fn tick_production_and_spawn_units(
    time: Res<Time>,
    mut buildings: Query<(&super::Building, &mut ProductionTimer, &Transform)>,
    unit_assets: Res<UnitAssets>,
    mut commands: Commands,
) {
    for (_building, mut timer, transform) in &mut buildings {
        timer.0.tick(time.delta());

        if timer.0.just_finished() {
            // Spawn unit one cell to the right of the building (toward combat zone)
            let spawn_x = transform.translation.x + CELL_SIZE;
            let spawn_y = transform.translation.y;

            commands.spawn((
                Name::new("Player Soldier"),
                Unit,
                Team::Player,
                Target,
                CurrentTarget(None),
                Health::new(SOLDIER_HEALTH),
                HealthBarConfig {
                    width: UNIT_HEALTH_BAR_WIDTH,
                    height: UNIT_HEALTH_BAR_HEIGHT,
                    y_offset: UNIT_HEALTH_BAR_Y_OFFSET,
                },
                CombatStats {
                    damage: SOLDIER_DAMAGE,
                    attack_speed: SOLDIER_ATTACK_SPEED,
                    range: SOLDIER_ATTACK_RANGE,
                },
                Movement {
                    speed: SOLDIER_MOVE_SPEED,
                },
                AttackTimer(Timer::from_seconds(
                    1.0 / SOLDIER_ATTACK_SPEED,
                    TimerMode::Repeating,
                )),
                Mesh2d(unit_assets.mesh.clone()),
                MeshMaterial2d(unit_assets.player_material.clone()),
                Transform::from_xyz(spawn_x, spawn_y, Z_UNIT),
                DespawnOnExit(GameState::InGame),
            ));
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::gameplay::building::{Building, BuildingType, HoveredCell, ProductionTimer};
    use crate::menus::Menu;
    use crate::testing::{assert_entity_count, transition_to_ingame};
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    /// Helper: app with battlefield + units + full building plugin, no `InputPlugin`.
    /// Used for production system tests where we manually spawn buildings.
    fn create_production_test_app() -> App {
        let mut app = crate::testing::create_base_test_app_no_input();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();
        // Building placement requires Gold and Shop resources.
        app.init_resource::<crate::gameplay::economy::Gold>();
        app.init_resource::<crate::gameplay::economy::shop::Shop>();

        app.configure_sets(
            Update,
            (crate::GameSet::Input, crate::GameSet::Production).chain(),
        );

        app.add_plugins(crate::gameplay::battlefield::plugin);
        app.add_plugins(crate::gameplay::units::plugin);
        app.add_plugins(crate::gameplay::building::plugin);
        transition_to_ingame(&mut app);
        app
    }

    /// Create a timer that will fire on the next `tick()` with any positive delta.
    /// Sets elapsed to just under the duration so the system's own tick triggers it.
    fn nearly_elapsed_timer() -> Timer {
        let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
        timer.set_elapsed(Duration::from_nanos(999_000));
        timer
    }

    #[test]
    fn barracks_gets_production_timer() {
        use crate::gameplay::building::BuildingType;
        use crate::gameplay::economy::shop::Shop;

        // Use isolated placement setup (without update_grid_cursor which clears HoveredCell).
        let mut app = crate::testing::create_base_test_app_no_input();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>();
        app.add_plugins(crate::gameplay::battlefield::plugin);
        app.register_type::<Building>()
            .register_type::<super::super::Occupied>()
            .register_type::<ProductionTimer>()
            .init_resource::<HoveredCell>()
            .init_resource::<crate::gameplay::economy::Gold>()
            .init_resource::<Shop>();
        app.add_systems(
            Update,
            super::super::placement::handle_building_placement
                .run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
        );
        transition_to_ingame(&mut app);

        // Pre-select a Barracks card in the shop.
        let mut shop = app.world_mut().resource_mut::<Shop>();
        shop.cards[0] = Some(BuildingType::Barracks);
        shop.selected = Some(0);

        // Place a barracks via HoveredCell + mouse click.
        app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        // Verify building has ProductionTimer.
        assert_entity_count::<(With<Building>, With<ProductionTimer>)>(&mut app, 1);
    }

    #[test]
    fn production_timer_spawns_unit() {
        let mut app = create_production_test_app();

        app.world_mut().spawn((
            Building {
                building_type: BuildingType::Barracks,
                grid_col: 2,
                grid_row: 3,
            },
            ProductionTimer(nearly_elapsed_timer()),
            Transform::from_xyz(320.0, 160.0, crate::Z_BUILDING),
            DespawnOnExit(GameState::InGame),
        ));
        app.update();

        assert_entity_count::<With<Unit>>(&mut app, 1);
    }

    #[test]
    fn spawned_unit_has_correct_components() {
        let mut app = create_production_test_app();

        app.world_mut().spawn((
            Building {
                building_type: BuildingType::Barracks,
                grid_col: 2,
                grid_row: 3,
            },
            ProductionTimer(nearly_elapsed_timer()),
            Transform::from_xyz(320.0, 160.0, crate::Z_BUILDING),
            DespawnOnExit(GameState::InGame),
        ));
        app.update();

        assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<Health>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<crate::gameplay::combat::HealthBarConfig>)>(
            &mut app, 1,
        );
        assert_entity_count::<(With<Unit>, With<CombatStats>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<Movement>)>(&mut app, 1);
        assert_entity_count::<(With<Unit>, With<DespawnOnExit<GameState>>)>(&mut app, 1);
    }

    #[test]
    fn spawned_unit_is_player_team() {
        let mut app = create_production_test_app();

        app.world_mut().spawn((
            Building {
                building_type: BuildingType::Barracks,
                grid_col: 0,
                grid_row: 0,
            },
            ProductionTimer(nearly_elapsed_timer()),
            Transform::from_xyz(200.0, 100.0, crate::Z_BUILDING),
            DespawnOnExit(GameState::InGame),
        ));
        app.update();

        let mut query = app.world_mut().query_filtered::<&Team, With<Unit>>();
        let team = query.single(app.world()).unwrap();
        assert_eq!(*team, Team::Player);
    }

    #[test]
    fn unit_spawns_to_right_of_building() {
        let mut app = create_production_test_app();

        let building_x = 320.0;
        let building_y = 160.0;

        app.world_mut().spawn((
            Building {
                building_type: BuildingType::Barracks,
                grid_col: 2,
                grid_row: 3,
            },
            ProductionTimer(nearly_elapsed_timer()),
            Transform::from_xyz(building_x, building_y, crate::Z_BUILDING),
            DespawnOnExit(GameState::InGame),
        ));
        app.update();

        let mut query = app.world_mut().query_filtered::<&Transform, With<Unit>>();
        let transform = query.single(app.world()).unwrap();
        assert_eq!(transform.translation.x, building_x + CELL_SIZE);
        assert_eq!(transform.translation.y, building_y);
    }

    #[test]
    fn no_units_without_buildings() {
        let mut app = create_production_test_app();
        app.update();
        assert_entity_count::<With<Unit>>(&mut app, 0);
    }
}

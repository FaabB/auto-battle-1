//! Building production: timer ticking and unit spawning.

use bevy::prelude::*;
use vleue_navigator::prelude::*;

use super::ProductionTimer;
use crate::Z_UNIT;
use crate::gameplay::building::building_stats;
use crate::gameplay::units::{UnitAssets, random_navigable_spawn, spawn_unit};

/// Radius from building center where spawned units appear.
/// Clears the 40px building sprite + 6px unit radius with margin.
const BUILDING_SPAWN_RADIUS: f32 = 40.0;

/// Ticks production timers on all buildings and spawns units when timers fire.
pub(super) fn tick_production_and_spawn_units(
    time: Res<Time>,
    mut buildings: Query<(&super::Building, &mut ProductionTimer, &Transform)>,
    unit_assets: Res<UnitAssets>,
    navmeshes: Option<Res<Assets<NavMesh>>>,
    navmesh_query: Option<Single<(&ManagedNavMesh, &NavMeshStatus)>>,
    mut commands: Commands,
) {
    // Extract navmesh if available and built
    let navmesh = navmesh_query.and_then(|inner| {
        let (managed, status) = *inner;
        let meshes = navmeshes.as_ref()?;
        (*status == NavMeshStatus::Built).then(|| meshes.get(managed))?
    });

    for (building, mut timer, transform) in &mut buildings {
        timer.0.tick(time.delta());

        if timer.0.just_finished() {
            let stats = building_stats(building.building_type);
            if let Some(unit_type) = stats.produced_unit {
                let center = transform.translation.xy();
                let spawn_xy = random_navigable_spawn(center, BUILDING_SPAWN_RADIUS, navmesh);

                spawn_unit(
                    &mut commands,
                    unit_type,
                    crate::gameplay::Team::Player,
                    spawn_xy.extend(Z_UNIT),
                    &unit_assets,
                );
            }
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::gameplay::building::{Building, BuildingType, HoveredCell, ProductionTimer};
    use crate::gameplay::units::Unit;
    use crate::gameplay::{CombatStats, Health, Movement, Team};
    use crate::menus::Menu;
    use crate::screens::GameState;
    use crate::testing::{assert_entity_count, transition_to_ingame};
    use pretty_assertions::assert_eq;

    /// Helper: app with battlefield + units + full building plugin, no `InputPlugin`.
    /// Used for production system tests where we manually spawn buildings.
    fn create_production_test_app() -> App {
        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        crate::testing::init_asset_resources(&mut app);
        crate::testing::init_economy_resources(&mut app);

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
    fn nearly_elapsed_timer() -> Timer {
        let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
        crate::testing::nearly_expire_timer(&mut timer);
        timer
    }

    #[test]
    fn barracks_gets_production_timer() {
        use crate::gameplay::building::BuildingType;
        use crate::gameplay::economy::shop::Shop;

        // Use isolated placement setup (without update_grid_cursor which clears HoveredCell).
        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        app.add_plugins(crate::gameplay::battlefield::plugin);
        app.register_type::<Building>()
            .register_type::<super::super::Occupied>()
            .register_type::<ProductionTimer>()
            .init_resource::<HoveredCell>();
        crate::testing::init_economy_resources(&mut app);
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
    fn unit_spawns_near_building() {
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
        let dx = transform.translation.x - building_x;
        let dy = transform.translation.y - building_y;
        let dist = dx.hypot(dy);
        assert!(
            (dist - BUILDING_SPAWN_RADIUS).abs() < 0.01,
            "Expected unit at distance {BUILDING_SPAWN_RADIUS} from building, got {dist}"
        );
    }

    #[test]
    fn no_units_without_buildings() {
        let mut app = create_production_test_app();
        app.update();
        assert_entity_count::<With<Unit>>(&mut app, 0);
    }
}

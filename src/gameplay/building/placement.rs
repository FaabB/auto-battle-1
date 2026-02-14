//! Building placement systems: grid cursor spawning, hover tracking, click-to-place.

use bevy::prelude::*;

use super::{
    BUILDING_SPRITE_SIZE, Building, BuildingType, CELL_SIZE, GRID_CURSOR_COLOR, GridCursor,
    HoveredCell, Occupied, ProductionTimer, building_color, world_to_build_grid,
};
use crate::gameplay::battlefield::{
    BUILD_ZONE_START_COL, GridIndex, col_to_world_x, row_to_world_y,
};
use crate::gameplay::units::{BARRACKS_PRODUCTION_INTERVAL, Target, Team};
use crate::screens::GameState;
use crate::{Z_BUILDING, Z_GRID_CURSOR};

/// Spawns the semi-transparent grid cursor entity. Hidden by default.
pub(super) fn spawn_grid_cursor(mut commands: Commands) {
    commands.spawn((
        GridCursor,
        Sprite::from_color(GRID_CURSOR_COLOR, Vec2::splat(CELL_SIZE - 2.0)),
        Transform::from_xyz(0.0, 0.0, Z_GRID_CURSOR),
        Visibility::Hidden,
        DespawnOnExit(GameState::InGame),
    ));
}

/// Moves the grid cursor to the cell under the mouse. Hides it when off-grid.
pub(super) fn update_grid_cursor(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera2d>>,
    mut cursor: Single<(&mut Transform, &mut Visibility), With<GridCursor>>,
    mut hovered: ResMut<HoveredCell>,
) {
    let (cursor_transform, cursor_visibility) = &mut *cursor;
    let (camera, camera_global) = *camera;

    // Try to convert screen cursor → world position → grid cell
    let grid_cell = window
        .cursor_position()
        .and_then(|screen_pos| camera.viewport_to_world_2d(camera_global, screen_pos).ok())
        .and_then(world_to_build_grid);

    hovered.0 = grid_cell;

    if let Some((col, row)) = grid_cell {
        // Position cursor sprite at the hovered cell
        let world_x = col_to_world_x(BUILD_ZONE_START_COL + col);
        let world_y = row_to_world_y(row);
        cursor_transform.translation.x = world_x;
        cursor_transform.translation.y = world_y;
        **cursor_visibility = Visibility::Inherited;
    } else {
        **cursor_visibility = Visibility::Hidden;
    }
}

/// Places a building when the player left-clicks an empty grid cell.
pub(super) fn handle_building_placement(
    mut commands: Commands,
    mouse: Res<ButtonInput<MouseButton>>,
    hovered: Res<HoveredCell>,
    grid_index: Res<GridIndex>,
    occupied: Query<(), With<Occupied>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Some((col, row)) = hovered.0 else {
        return;
    };

    // O(1) lookup via GridIndex
    let Some(slot_entity) = grid_index.get(col, row) else {
        return;
    };

    // Skip if already occupied
    if occupied.contains(slot_entity) {
        return;
    }

    // Mark slot as occupied
    commands.entity(slot_entity).insert(Occupied);

    // Spawn the building entity
    let building_type = BuildingType::Barracks; // Hardcoded for now (Ticket 6 adds selector)
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

    // Only Barracks get a production timer
    if building_type == BuildingType::Barracks {
        entity_commands.insert(ProductionTimer(Timer::from_seconds(
            BARRACKS_PRODUCTION_INTERVAL,
            TimerMode::Repeating,
        )));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::gameplay::battlefield::BuildSlot;
    use crate::menus::Menu;
    use crate::testing::assert_entity_count;
    use pretty_assertions::assert_eq;

    /// Helper: app with battlefield + building + units plugins, transitioned to `InGame`.
    fn create_building_test_app() -> App {
        let mut app = crate::testing::create_base_test_app();
        // Units plugin needs asset infrastructure for UnitAssets (mesh + material).
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();
        app.add_plugins(crate::gameplay::battlefield::plugin);
        app.add_plugins(crate::gameplay::units::plugin);
        app.add_plugins(super::super::plugin);
        crate::testing::transition_to_ingame(&mut app);
        app
    }

    #[test]
    fn grid_cursor_spawned() {
        let mut app = create_building_test_app();
        assert_entity_count::<With<GridCursor>>(&mut app, 1);
    }

    #[test]
    fn grid_cursor_starts_hidden() {
        let mut app = create_building_test_app();
        let mut query = app
            .world_mut()
            .query_filtered::<&Visibility, With<GridCursor>>();
        let visibility = query.single(app.world()).unwrap();
        assert_eq!(*visibility, Visibility::Hidden);
    }

    #[test]
    fn hovered_cell_resource_initialized() {
        let app = create_building_test_app();
        let hovered = app.world().resource::<HoveredCell>();
        assert!(hovered.0.is_none());
    }

    #[test]
    fn no_buildings_at_start() {
        let mut app = create_building_test_app();
        assert_entity_count::<With<Building>>(&mut app, 0);
    }

    #[test]
    fn no_occupied_slots_at_start() {
        let mut app = create_building_test_app();
        assert_entity_count::<(With<BuildSlot>, With<Occupied>)>(&mut app, 0);
    }

    #[test]
    fn grid_cursor_has_despawn_on_exit() {
        let mut app = create_building_test_app();
        assert_entity_count::<(With<GridCursor>, With<DespawnOnExit<GameState>>)>(&mut app, 1);
    }

    /// Helper: app with only `handle_building_placement` (no `update_grid_cursor`).
    /// Skips `InputPlugin` so `just_pressed` isn't cleared in `PreUpdate`,
    /// allowing tests to call `press()` and have it visible in `Update`.
    fn create_placement_test_app() -> App {
        let mut app = crate::testing::create_base_test_app_no_input();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>();
        app.add_plugins(crate::gameplay::battlefield::plugin);
        app.register_type::<Building>()
            .register_type::<Occupied>()
            .init_resource::<HoveredCell>();
        app.add_systems(
            Update,
            handle_building_placement.run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
        );
        crate::testing::transition_to_ingame(&mut app);
        app
    }

    #[test]
    fn clicking_empty_cell_places_building() {
        let mut app = create_placement_test_app();

        app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        assert_entity_count::<With<Building>>(&mut app, 1);
        assert_entity_count::<(With<BuildSlot>, With<Occupied>)>(&mut app, 1);
    }

    #[test]
    fn placed_building_has_correct_data() {
        let mut app = create_placement_test_app();

        app.world_mut().resource_mut::<HoveredCell>().0 = Some((1, 4));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let mut query = app.world_mut().query::<&Building>();
        let building = query.single(app.world()).unwrap();
        assert_eq!(building.building_type, BuildingType::Barracks);
        assert_eq!(building.grid_col, 1);
        assert_eq!(building.grid_row, 4);
    }

    #[test]
    fn clicking_occupied_cell_does_not_place_duplicate() {
        let mut app = create_placement_test_app();

        // Place first building at (3, 5)
        app.world_mut().resource_mut::<HoveredCell>().0 = Some((3, 5));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        // Try to place again at the same cell
        app.world_mut().resource_mut::<HoveredCell>().0 = Some((3, 5));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        assert_entity_count::<With<Building>>(&mut app, 1); // Still just one
    }

    #[test]
    fn clicking_with_no_hovered_cell_does_nothing() {
        let mut app = create_placement_test_app();

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        assert_entity_count::<With<Building>>(&mut app, 0);
    }

    #[test]
    fn building_has_despawn_on_exit() {
        let mut app = create_placement_test_app();

        app.world_mut().resource_mut::<HoveredCell>().0 = Some((0, 0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        assert_entity_count::<(With<Building>, With<DespawnOnExit<GameState>>)>(&mut app, 1);
    }
}

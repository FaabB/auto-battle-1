//! Building placement systems: grid cursor spawning, hover tracking, click-to-place.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{
    BUILDING_HEALTH_BAR_HEIGHT, BUILDING_HEALTH_BAR_WIDTH, BUILDING_HEALTH_BAR_Y_OFFSET,
    BUILDING_SPRITE_SIZE, Building, CELL_SIZE, GridCursor, HoveredCell, Occupied, ProductionTimer,
    building_color, building_hp, building_stats, world_to_build_grid,
};
use crate::gameplay::battlefield::{
    BUILD_ZONE_START_COL, GridIndex, col_to_world_x, row_to_world_y,
};
use crate::gameplay::combat::HealthBarConfig;
use crate::gameplay::{Health, Target, Team};

use crate::screens::GameState;
use crate::third_party::{NavObstacle, solid_entity_layers};
use crate::{Z_BUILDING, Z_GRID_CURSOR};

/// Spawns the semi-transparent grid cursor entity. Hidden by default.
pub(super) fn spawn_grid_cursor(mut commands: Commands) {
    commands.spawn((
        Name::new("Grid Cursor"),
        GridCursor,
        Sprite::from_color(
            crate::theme::palette::GRID_CURSOR,
            Vec2::splat(CELL_SIZE - 2.0),
        ),
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

    // Try to convert screen cursor → world position → grid cell.
    // Ignore cursor positions over the bottom bar area.
    let bar_threshold = window.height() - crate::gameplay::hud::bottom_bar::BOTTOM_BAR_HEIGHT;
    let grid_cell = window
        .cursor_position()
        .filter(|pos| pos.y < bar_threshold)
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
    mut gold: ResMut<crate::gameplay::economy::Gold>,
    mut shop: ResMut<crate::gameplay::economy::shop::Shop>,
    ui_buttons: Query<&Interaction, With<Button>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    // Skip if mouse is over any UI button (prevents click-through from shop panel)
    if ui_buttons.iter().any(|i| *i != Interaction::None) {
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

    // Get selected building from shop
    let Some(building_type) = shop.selected_building() else {
        return; // No card selected
    };

    // Check gold
    let stats = building_stats(building_type);
    if gold.0 < stats.cost {
        return;
    }

    // Deduct gold and remove card from shop
    gold.0 -= stats.cost;
    shop.remove_selected();

    // Mark slot as occupied
    commands.entity(slot_entity).insert(Occupied);

    // Spawn the building entity
    let world_x = col_to_world_x(BUILD_ZONE_START_COL + col);
    let world_y = row_to_world_y(row);

    let mut entity_commands = commands.spawn((
        Name::new(format!("{building_type:?}")),
        Building {
            building_type,
            grid_col: col,
            grid_row: row,
        },
        Team::Player,
        Target,
        Health::new(building_hp(building_type)),
        HealthBarConfig {
            width: BUILDING_HEALTH_BAR_WIDTH,
            height: BUILDING_HEALTH_BAR_HEIGHT,
            y_offset: BUILDING_HEALTH_BAR_Y_OFFSET,
        },
        Sprite::from_color(
            building_color(building_type),
            Vec2::splat(BUILDING_SPRITE_SIZE),
        ),
        Transform::from_xyz(world_x, world_y, Z_BUILDING),
        DespawnOnExit(GameState::InGame),
        NavObstacle,
        // Physics
        RigidBody::Static,
        Collider::rectangle(BUILDING_SPRITE_SIZE, BUILDING_SPRITE_SIZE),
        solid_entity_layers(),
    ));

    // Data-driven timer insertion — no per-type match needed
    if let Some(interval) = stats.production_interval {
        entity_commands.insert((
            super::production::ProductionBarConfig {
                width: BUILDING_HEALTH_BAR_WIDTH,
                height: BUILDING_HEALTH_BAR_HEIGHT,
                y_offset: -BUILDING_HEALTH_BAR_Y_OFFSET,
            },
            ProductionTimer(Timer::from_seconds(interval, TimerMode::Repeating)),
        ));
    }
    if let Some(interval) = stats.income_interval {
        entity_commands.insert(crate::gameplay::economy::income::IncomeTimer(
            Timer::from_seconds(interval, TimerMode::Repeating),
        ));
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::gameplay::battlefield::BuildSlot;
    use crate::gameplay::building::BuildingType;
    use crate::menus::Menu;
    use crate::testing::assert_entity_count;
    use pretty_assertions::assert_eq;

    /// Helper: app with battlefield + building + units plugins, transitioned to `InGame`.
    fn create_building_test_app() -> App {
        let mut app = crate::testing::create_base_test_app();
        crate::testing::init_asset_resources(&mut app);
        crate::testing::init_economy_resources(&mut app);
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
    /// Pre-selects a Barracks card in the shop so placement tests work by default.
    fn create_placement_test_app() -> App {
        use crate::gameplay::economy::shop::Shop;

        let mut app = crate::testing::create_base_test_app_no_input();
        crate::testing::init_input_resources(&mut app);
        app.add_plugins(crate::gameplay::battlefield::plugin);
        app.register_type::<Building>()
            .register_type::<Occupied>()
            .init_resource::<HoveredCell>();
        crate::testing::init_economy_resources(&mut app);
        app.add_systems(
            Update,
            handle_building_placement.run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
        );
        crate::testing::transition_to_ingame(&mut app);

        // Pre-select a Barracks card so existing placement tests work.
        let mut shop = app.world_mut().resource_mut::<Shop>();
        shop.cards[0] = Some(BuildingType::Barracks);
        shop.selected = Some(0);

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
        use crate::gameplay::economy::shop::Shop;

        let mut app = create_placement_test_app();

        // Place first building at (3, 5)
        app.world_mut().resource_mut::<HoveredCell>().0 = Some((3, 5));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        // Re-select a card (first placement consumed the selection)
        let mut shop = app.world_mut().resource_mut::<Shop>();
        shop.cards[1] = Some(BuildingType::Barracks);
        shop.selected = Some(1);

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
    fn clicking_ui_button_does_not_place_building() {
        let mut app = create_placement_test_app();

        // Simulate a UI button being pressed (prevents click-through)
        app.world_mut().spawn((Button, Interaction::Pressed));

        app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
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

    // === Gold Cost Tests ===

    #[test]
    fn placement_deducts_gold() {
        let mut app = create_placement_test_app();

        app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let gold = app.world().resource::<crate::gameplay::economy::Gold>();
        assert_eq!(
            gold.0,
            crate::gameplay::economy::STARTING_GOLD
                - crate::gameplay::building::building_stats(BuildingType::Barracks).cost
        );
        assert_entity_count::<With<Building>>(&mut app, 1);
    }

    #[test]
    fn placement_blocked_when_insufficient_gold() {
        let mut app = create_placement_test_app();

        // Set gold to 0
        app.world_mut()
            .resource_mut::<crate::gameplay::economy::Gold>()
            .0 = 0;

        app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let gold = app.world().resource::<crate::gameplay::economy::Gold>();
        assert_eq!(gold.0, 0);
        assert_entity_count::<With<Building>>(&mut app, 0);
    }

    #[test]
    fn placement_blocked_when_gold_below_cost() {
        let mut app = create_placement_test_app();

        // Set gold to just below Barracks cost
        app.world_mut()
            .resource_mut::<crate::gameplay::economy::Gold>()
            .0 = crate::gameplay::building::building_stats(BuildingType::Barracks).cost - 1;

        app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let gold = app.world().resource::<crate::gameplay::economy::Gold>();
        assert_eq!(
            gold.0,
            crate::gameplay::building::building_stats(BuildingType::Barracks).cost - 1
        );
        assert_entity_count::<With<Building>>(&mut app, 0);
    }

    // === Building Health Tests (GAM-21) ===

    #[test]
    fn placed_building_has_health() {
        use crate::gameplay::Health;

        let mut app = create_placement_test_app();
        let expected_hp = crate::gameplay::building::building_stats(BuildingType::Barracks).hp;

        app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let mut query = app.world_mut().query_filtered::<&Health, With<Building>>();
        let health = query.single(app.world()).unwrap();
        assert_eq!(health.current, expected_hp);
        assert_eq!(health.max, expected_hp);
    }

    #[test]
    fn placed_building_has_health_bar_config() {
        use crate::gameplay::combat::HealthBarConfig;

        let mut app = create_placement_test_app();

        app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        assert_entity_count::<(With<Building>, With<HealthBarConfig>)>(&mut app, 1);
    }
}

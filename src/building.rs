//! Building placement: grid cursor, hover highlight, and click-to-place buildings.

use bevy::prelude::*;

use crate::battlefield::{
    BATTLEFIELD_HEIGHT, BUILD_ZONE_START_COL, BattlefieldSetup, CELL_SIZE, GridIndex,
};
use crate::{GameState, InGameState, Z_BUILDING, Z_GRID_CURSOR};

// === Constants ===

/// Color for the grid cursor hover highlight.
const GRID_CURSOR_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.2);

/// Barracks building color (dark blue).
const BARRACKS_COLOR: Color = Color::srgb(0.15, 0.2, 0.6);
/// Farm building color (green).
const FARM_COLOR: Color = Color::srgb(0.2, 0.6, 0.1);

/// Building sprite size (slightly smaller than cell to show grid outline).
const BUILDING_SPRITE_SIZE: f32 = CELL_SIZE - 4.0;

// === Components ===

/// A placed building on the grid.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Building {
    pub building_type: BuildingType,
    /// Local grid column (0–5).
    pub grid_col: u16,
    /// Grid row (0–9).
    pub grid_row: u16,
}

/// Types of buildings the player can place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum BuildingType {
    Barracks,
    Farm,
}

/// Marker: this `BuildSlot` has a building on it.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Occupied;

/// Marker for the grid cursor (hover highlight) entity.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct GridCursor;

/// Tracks which build-zone cell the mouse is currently over.
#[derive(Resource, Default, Debug, Reflect)]
#[reflect(Resource)]
pub struct HoveredCell(pub Option<(u16, u16)>);

// === Helper Functions ===

/// Convert a world position to build-zone grid coordinates.
///
/// Returns `Some((local_col, row))` if the position is inside the build zone,
/// where `local_col` is 0–5 and `row` is 0–9. Returns `None` otherwise.
#[must_use]
pub fn world_to_build_grid(world_pos: Vec2) -> Option<(u16, u16)> {
    use crate::battlefield::{BUILD_ZONE_END_X, BUILD_ZONE_START_X};

    if world_pos.x < BUILD_ZONE_START_X
        || world_pos.x >= BUILD_ZONE_END_X
        || world_pos.y < 0.0
        || world_pos.y >= BATTLEFIELD_HEIGHT
    {
        return None;
    }

    // Safety: bounds check above guarantees non-negative values within grid range.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let col = ((world_pos.x - BUILD_ZONE_START_X) / CELL_SIZE) as u16;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]
    let row = (world_pos.y / CELL_SIZE) as u16;
    Some((col, row))
}

/// Get the color for a building type.
#[must_use]
pub const fn building_color(building_type: BuildingType) -> Color {
    match building_type {
        BuildingType::Barracks => BARRACKS_COLOR,
        BuildingType::Farm => FARM_COLOR,
    }
}

// === Systems ===

/// Spawns the semi-transparent grid cursor entity. Hidden by default.
fn spawn_grid_cursor(mut commands: Commands) {
    commands.spawn((
        GridCursor,
        Sprite::from_color(GRID_CURSOR_COLOR, Vec2::splat(CELL_SIZE - 2.0)),
        Transform::from_xyz(0.0, 0.0, Z_GRID_CURSOR),
        Visibility::Hidden,
        DespawnOnExit(GameState::InGame),
    ));
}

/// Moves the grid cursor to the cell under the mouse. Hides it when off-grid.
fn update_grid_cursor(
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
        let world_x = crate::battlefield::col_to_world_x(BUILD_ZONE_START_COL + col);
        let world_y = crate::battlefield::row_to_world_y(row);
        cursor_transform.translation.x = world_x;
        cursor_transform.translation.y = world_y;
        **cursor_visibility = Visibility::Inherited;
    } else {
        **cursor_visibility = Visibility::Hidden;
    }
}

/// Places a building when the player left-clicks an empty grid cell.
fn handle_building_placement(
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
    let world_x = crate::battlefield::col_to_world_x(BUILD_ZONE_START_COL + col);
    let world_y = crate::battlefield::row_to_world_y(row);

    commands.spawn((
        Building {
            building_type,
            grid_col: col,
            grid_row: row,
        },
        Sprite::from_color(
            building_color(building_type),
            Vec2::splat(BUILDING_SPRITE_SIZE),
        ),
        Transform::from_xyz(world_x, world_y, Z_BUILDING),
        DespawnOnExit(GameState::InGame),
    ));
}

// === Plugin ===

/// Plugin for building placement: grid cursor, hover, and click-to-place.
#[derive(Debug)]
pub struct BuildingPlugin;

impl Plugin for BuildingPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Building>()
            .register_type::<BuildingType>()
            .register_type::<Occupied>()
            .register_type::<GridCursor>()
            .register_type::<HoveredCell>()
            .init_resource::<HoveredCell>();

        app.add_systems(
            OnEnter(GameState::InGame),
            spawn_grid_cursor.after(BattlefieldSetup),
        )
            .add_systems(
                Update,
                (update_grid_cursor, handle_building_placement)
                    .chain_ignore_deferred()
                    .run_if(in_state(InGameState::Playing)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    // --- world_to_build_grid tests ---

    #[test]
    fn world_to_build_grid_first_cell() {
        // First cell: col 0, row 0 — world position just inside (128.0, 0.0)
        let result = world_to_build_grid(Vec2::new(128.5, 0.5));
        assert_eq!(result, Some((0, 0)));
    }

    #[test]
    fn world_to_build_grid_last_cell() {
        // Last cell: col 5, row 9 — world position inside (511.x, 639.x)
        let result = world_to_build_grid(Vec2::new(500.0, 630.0));
        assert_eq!(result, Some((5, 9)));
    }

    #[test]
    fn world_to_build_grid_center_of_cell() {
        // Center of cell (2, 3) — use the same helpers the production code uses
        let x = crate::battlefield::col_to_world_x(BUILD_ZONE_START_COL + 2);
        let y = crate::battlefield::row_to_world_y(3);
        let result = world_to_build_grid(Vec2::new(x, y));
        assert_eq!(result, Some((2, 3)));
    }

    #[test]
    fn world_to_build_grid_outside_left() {
        // Before build zone (x < 128.0)
        assert_eq!(world_to_build_grid(Vec2::new(100.0, 100.0)), None);
    }

    #[test]
    fn world_to_build_grid_outside_right() {
        // After build zone (x >= 512.0)
        assert_eq!(world_to_build_grid(Vec2::new(512.0, 100.0)), None);
    }

    #[test]
    fn world_to_build_grid_outside_top() {
        // Above battlefield (y >= 640.0)
        assert_eq!(world_to_build_grid(Vec2::new(200.0, 640.0)), None);
    }

    #[test]
    fn world_to_build_grid_outside_bottom() {
        // Below battlefield (y < 0.0)
        assert_eq!(world_to_build_grid(Vec2::new(200.0, -1.0)), None);
    }

    #[test]
    fn world_to_build_grid_boundary_left_edge() {
        // Exactly at left boundary (128.0 = first valid x)
        assert_eq!(world_to_build_grid(Vec2::new(128.0, 32.0)), Some((0, 0)));
    }

    #[test]
    fn world_to_build_grid_boundary_right_edge() {
        // Exactly at right boundary (512.0 = first invalid x)
        assert_eq!(world_to_build_grid(Vec2::new(512.0, 32.0)), None);
    }

    // --- building_color tests ---

    #[test]
    fn barracks_color_is_blue() {
        let color = building_color(BuildingType::Barracks);
        assert_eq!(color, BARRACKS_COLOR);
    }

    #[test]
    fn farm_color_is_green() {
        let color = building_color(BuildingType::Farm);
        assert_eq!(color, FARM_COLOR);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::battlefield::{BattlefieldPlugin, BuildSlot};
    use pretty_assertions::assert_eq;

    /// Helper: app with `BattlefieldPlugin` + `BuildingPlugin`, transitioned to `InGame`.
    fn create_building_test_app() -> App {
        let mut app = crate::testing::create_base_test_app();
        app.add_plugins(BattlefieldPlugin);
        app.add_plugins(BuildingPlugin);
        crate::testing::transition_to_ingame(&mut app);
        app
    }

    #[test]
    fn grid_cursor_spawned() {
        let mut app = create_building_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), With<GridCursor>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
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
        let count = app
            .world_mut()
            .query_filtered::<(), With<Building>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn no_occupied_slots_at_start() {
        let mut app = create_building_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), (With<BuildSlot>, With<Occupied>)>()
            .iter(app.world())
            .count();
        assert_eq!(count, 0);
    }

    #[test]
    fn grid_cursor_has_despawn_on_exit() {
        let mut app = create_building_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), (With<GridCursor>, With<DespawnOnExit<GameState>>)>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    /// Helper: app with only `handle_building_placement` (no `update_grid_cursor`).
    /// Skips `InputPlugin` so `just_pressed` isn't cleared in `PreUpdate`,
    /// allowing tests to call `press()` and have it visible in `Update`.
    fn create_placement_test_app() -> App {
        let mut app = crate::testing::create_base_test_app_no_input();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>();
        app.add_plugins(BattlefieldPlugin);
        app.register_type::<Building>()
            .register_type::<Occupied>()
            .init_resource::<HoveredCell>();
        app.add_systems(
            Update,
            handle_building_placement.run_if(in_state(InGameState::Playing)),
        );
        crate::testing::transition_to_ingame(&mut app);
        app
    }

    #[test]
    fn clicking_empty_cell_places_building() {
        let mut app = create_placement_test_app();

        // Simulate hovering cell (2, 3) and pressing left click
        app.world_mut().resource_mut::<HoveredCell>().0 = Some((2, 3));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let building_count = app
            .world_mut()
            .query_filtered::<(), With<Building>>()
            .iter(app.world())
            .count();
        assert_eq!(building_count, 1);

        let occupied_count = app
            .world_mut()
            .query_filtered::<(), (With<BuildSlot>, With<Occupied>)>()
            .iter(app.world())
            .count();
        assert_eq!(occupied_count, 1);
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

        let building_count = app
            .world_mut()
            .query_filtered::<(), With<Building>>()
            .iter(app.world())
            .count();
        assert_eq!(building_count, 1); // Still just one
    }

    #[test]
    fn clicking_with_no_hovered_cell_does_nothing() {
        let mut app = create_placement_test_app();

        // HoveredCell is None (cursor off grid), click
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let building_count = app
            .world_mut()
            .query_filtered::<(), With<Building>>()
            .iter(app.world())
            .count();
        assert_eq!(building_count, 0);
    }

    #[test]
    fn building_has_despawn_on_exit() {
        let mut app = create_placement_test_app();

        app.world_mut().resource_mut::<HoveredCell>().0 = Some((0, 0));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.update();

        let count = app
            .world_mut()
            .query_filtered::<(), (With<Building>, With<DespawnOnExit<GameState>>)>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }
}

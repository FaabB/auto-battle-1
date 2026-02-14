//! Building placement: grid cursor, hover highlight, and click-to-place buildings.

mod placement;

use bevy::prelude::*;

use crate::gameplay::battlefield::{BATTLEFIELD_HEIGHT, BattlefieldSetup, CELL_SIZE};
use crate::menus::Menu;
use crate::screens::GameState;

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
#[allow(clippy::struct_field_names)]
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
    use crate::gameplay::battlefield::{BUILD_ZONE_END_X, BUILD_ZONE_START_X};

    if world_pos.x < BUILD_ZONE_START_X
        || world_pos.x >= BUILD_ZONE_END_X
        || world_pos.y < 0.0
        || world_pos.y >= BATTLEFIELD_HEIGHT
    {
        return None;
    }

    // Safety: bounds check above guarantees non-negative values within grid range.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let col = ((world_pos.x - BUILD_ZONE_START_X) / CELL_SIZE) as u16;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Building>()
        .register_type::<BuildingType>()
        .register_type::<Occupied>()
        .register_type::<GridCursor>()
        .register_type::<HoveredCell>()
        .init_resource::<HoveredCell>();

    app.add_systems(
        OnEnter(GameState::InGame),
        placement::spawn_grid_cursor.after(BattlefieldSetup),
    )
    .add_systems(
        Update,
        (
            placement::update_grid_cursor,
            placement::handle_building_placement,
        )
            .chain_ignore_deferred()
            .in_set(crate::GameSet::Input)
            .run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::battlefield::BUILD_ZONE_START_COL;
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
        let x = crate::gameplay::battlefield::col_to_world_x(BUILD_ZONE_START_COL + 2);
        let y = crate::gameplay::battlefield::row_to_world_y(3);
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

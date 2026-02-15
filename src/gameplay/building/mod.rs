//! Building placement: grid cursor, hover highlight, and click-to-place buildings.

mod placement;
mod production;

use bevy::prelude::*;

use crate::gameplay::battlefield::{BATTLEFIELD_HEIGHT, BattlefieldSetup, CELL_SIZE};
use crate::gameplay::units::UnitType;
use crate::screens::GameState;
use crate::{GameSet, gameplay_running};

// === Constants ===

/// Color for the grid cursor hover highlight.
const GRID_CURSOR_COLOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.2);

/// Building sprite size (slightly smaller than cell to show grid outline).
const BUILDING_SPRITE_SIZE: f32 = CELL_SIZE - 4.0;

/// Building health bar width (wider than units since buildings are larger).
const BUILDING_HEALTH_BAR_WIDTH: f32 = 40.0;

/// Building health bar height.
const BUILDING_HEALTH_BAR_HEIGHT: f32 = 4.0;

/// Building health bar Y offset (above center of building sprite).
const BUILDING_HEALTH_BAR_Y_OFFSET: f32 = 36.0;

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

impl BuildingType {
    /// All building types, used by shop card pool.
    pub const ALL: &[Self] = &[Self::Barracks, Self::Farm];

    /// Human-readable display name.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Barracks => "Barracks",
            Self::Farm => "Farm",
        }
    }
}

/// Stats for a building type. All values are compile-time constants.
#[derive(Debug, Clone, Copy)]
pub struct BuildingStats {
    /// Maximum hit points.
    pub hp: f32,
    /// Gold cost to place.
    pub cost: u32,
    /// Sprite color.
    pub color: Color,
    /// Unit type this building produces, if any.
    pub produced_unit: Option<UnitType>,
    /// Production timer interval (seconds), if this building produces units.
    pub production_interval: Option<f32>,
    /// Income timer interval (seconds), if this building generates income.
    pub income_interval: Option<f32>,
}

/// Look up stats for a building type.
#[must_use]
pub const fn building_stats(building_type: BuildingType) -> BuildingStats {
    match building_type {
        BuildingType::Barracks => BuildingStats {
            hp: 300.0,
            cost: 100,
            color: Color::srgb(0.15, 0.2, 0.6),
            produced_unit: Some(UnitType::Soldier),
            production_interval: Some(3.0),
            income_interval: None,
        },
        BuildingType::Farm => BuildingStats {
            hp: 150.0,
            cost: 50,
            color: Color::srgb(0.2, 0.6, 0.1),
            produced_unit: None,
            production_interval: None,
            income_interval: Some(1.0),
        },
    }
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

/// Production timer for buildings that spawn units (e.g., Barracks).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct ProductionTimer(pub Timer);

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
    building_stats(building_type).color
}

/// Get the max HP for a building type.
#[must_use]
pub const fn building_hp(building_type: BuildingType) -> f32 {
    building_stats(building_type).hp
}

// === Observers ===

/// When a building is removed (death, despawn), clear the `Occupied` marker
/// from the corresponding build slot so the grid cell can be reused.
fn clear_build_slot_on_building_removed(
    remove: On<Remove, Building>,
    buildings: Query<&Building>,
    grid_index: Res<crate::gameplay::battlefield::GridIndex>,
    mut commands: Commands,
) {
    let Ok(building) = buildings.get(remove.entity) else {
        return;
    };
    let Some(slot_entity) = grid_index.get(building.grid_col, building.grid_row) else {
        return;
    };
    commands.entity(slot_entity).remove::<Occupied>();
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Building>()
        .register_type::<BuildingType>()
        .register_type::<Occupied>()
        .register_type::<GridCursor>()
        .register_type::<HoveredCell>()
        .register_type::<ProductionTimer>()
        .init_resource::<HoveredCell>();

    app.add_observer(clear_build_slot_on_building_removed);

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
            .in_set(GameSet::Input)
            .run_if(gameplay_running),
    )
    .add_systems(
        Update,
        production::tick_production_and_spawn_units
            .in_set(GameSet::Production)
            .run_if(gameplay_running),
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

    // --- building_stats tests ---

    #[test]
    fn barracks_stats() {
        let stats = building_stats(BuildingType::Barracks);
        assert!(stats.hp > 0.0);
        assert!(stats.cost > 0);
        assert!(stats.produced_unit.is_some());
        assert!(stats.production_interval.is_some());
        assert!(stats.income_interval.is_none());
    }

    #[test]
    fn farm_stats() {
        let stats = building_stats(BuildingType::Farm);
        assert!(stats.hp > 0.0);
        assert!(stats.cost > 0);
        assert!(stats.produced_unit.is_none());
        assert!(stats.production_interval.is_none());
        assert!(stats.income_interval.is_some());
    }

    #[test]
    fn building_type_display_name() {
        assert_eq!(BuildingType::Barracks.display_name(), "Barracks");
        assert_eq!(BuildingType::Farm.display_name(), "Farm");
    }

    #[test]
    fn building_type_all_contains_all_variants() {
        assert!(BuildingType::ALL.contains(&BuildingType::Barracks));
        assert!(BuildingType::ALL.contains(&BuildingType::Farm));
    }

    // --- building_color / building_hp delegate to building_stats ---

    #[test]
    fn building_color_matches_stats() {
        assert_eq!(
            building_color(BuildingType::Barracks),
            building_stats(BuildingType::Barracks).color
        );
        assert_eq!(
            building_color(BuildingType::Farm),
            building_stats(BuildingType::Farm).color
        );
    }

    #[test]
    fn building_hp_matches_stats() {
        assert_eq!(
            building_hp(BuildingType::Barracks),
            building_stats(BuildingType::Barracks).hp
        );
        assert_eq!(
            building_hp(BuildingType::Farm),
            building_stats(BuildingType::Farm).hp
        );
    }
}

#[cfg(test)]
mod observer_tests {
    use super::*;
    use crate::gameplay::Health;
    use crate::gameplay::battlefield::{BuildSlot, GridIndex};
    use crate::testing::assert_entity_count;

    fn create_observer_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<GridIndex>();
        app.add_observer(clear_build_slot_on_building_removed);
        app
    }

    #[test]
    fn building_death_removes_occupied_from_slot() {
        let mut app = create_observer_test_app();

        // Spawn a build slot and register it in the grid index
        let slot = app
            .world_mut()
            .spawn((BuildSlot { col: 2, row: 3 }, Occupied))
            .id();
        app.world_mut()
            .resource_mut::<GridIndex>()
            .insert(2, 3, slot);

        // Spawn a building at that grid position
        let building = app
            .world_mut()
            .spawn((
                Building {
                    building_type: BuildingType::Barracks,
                    grid_col: 2,
                    grid_row: 3,
                },
                Health::new(building_stats(BuildingType::Barracks).hp),
            ))
            .id();

        app.update();

        // Despawn the building (simulates check_death)
        app.world_mut().despawn(building);
        app.update(); // Process deferred commands from observer

        // Slot should no longer be occupied
        assert_entity_count::<(With<BuildSlot>, With<Occupied>)>(&mut app, 0);
        // Slot entity itself should still exist
        assert_entity_count::<With<BuildSlot>>(&mut app, 1);
    }

    #[test]
    fn building_death_slot_remains_when_not_in_grid_index() {
        let mut app = create_observer_test_app();

        // Spawn a building without a matching grid index entry
        let building = app
            .world_mut()
            .spawn((
                Building {
                    building_type: BuildingType::Farm,
                    grid_col: 0,
                    grid_row: 0,
                },
                Health::new(building_stats(BuildingType::Farm).hp),
            ))
            .id();

        app.update();
        app.world_mut().despawn(building);
        app.update();
        // Should not panic — gracefully handles missing slot
    }
}

//! Battlefield layout constants, markers, and systems.

mod camera;
mod renderer;

use std::collections::HashMap;

use bevy::prelude::*;

use crate::{GameState, InGameState};

// === Grid Constants ===

/// Size of a single grid cell in pixels.
pub const CELL_SIZE: f32 = 64.0;

/// Number of rows in the battlefield.
pub const BATTLEFIELD_ROWS: u16 = 10;

/// Number of columns for each fortress.
pub const FORTRESS_COLS: u16 = 2;

/// Number of columns in the building zone.
pub const BUILD_ZONE_COLS: u16 = 6;

/// Number of columns in the combat zone.
pub const COMBAT_ZONE_COLS: u16 = 72;

/// Total columns across the entire battlefield.
pub const TOTAL_COLS: u16 = FORTRESS_COLS + BUILD_ZONE_COLS + COMBAT_ZONE_COLS + FORTRESS_COLS;
// = 2 + 6 + 72 + 2 = 82

/// Total battlefield width in pixels.
pub const BATTLEFIELD_WIDTH: f32 = TOTAL_COLS as f32 * CELL_SIZE;
// = 82 * 64 = 5248.0

/// Total battlefield height in pixels.
pub const BATTLEFIELD_HEIGHT: f32 = BATTLEFIELD_ROWS as f32 * CELL_SIZE;
// = 10 * 64 = 640.0

// === Zone Column Ranges (start column, inclusive) ===

/// Player fortress starts at column 0.
pub const PLAYER_FORT_START_COL: u16 = 0;

/// Building zone starts after player fortress.
pub const BUILD_ZONE_START_COL: u16 = FORTRESS_COLS;
// = 2

/// Combat zone starts after building zone.
pub const COMBAT_ZONE_START_COL: u16 = FORTRESS_COLS + BUILD_ZONE_COLS;
// = 8

/// Enemy fortress starts after combat zone.
pub const ENEMY_FORT_START_COL: u16 = FORTRESS_COLS + BUILD_ZONE_COLS + COMBAT_ZONE_COLS;
// = 80

// === Zone Pixel Boundaries (pre-computed for readability) ===

/// Build zone left edge in world pixels.
pub const BUILD_ZONE_START_X: f32 = BUILD_ZONE_START_COL as f32 * CELL_SIZE;
// = 128.0

/// Build zone right edge in world pixels (exclusive).
pub const BUILD_ZONE_END_X: f32 = (BUILD_ZONE_START_COL + BUILD_ZONE_COLS) as f32 * CELL_SIZE;
// = 512.0

// === Marker Components ===

/// Marks the player's fortress entity. Ticket 8 adds `Health` to this.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct PlayerFortress;

/// Marks the enemy's fortress entity. Ticket 8 adds `Health` to this.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct EnemyFortress;

/// Marks the build zone area entity.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct BuildZone;

/// Marks the combat zone area entity.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct CombatZone;

/// Marks the background entity.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct BattlefieldBackground;

/// Marks a grid cell in the build zone.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct BuildSlot {
    pub row: u16,
    /// Local column (0–5), not global.
    pub col: u16,
}

/// Maps `(col, row)` to the `BuildSlot` entity for O(1) grid lookups.
#[derive(Resource, Default, Debug)]
pub struct GridIndex {
    slots: HashMap<(u16, u16), Entity>,
}

impl GridIndex {
    /// Insert a slot entity at the given grid coordinates.
    pub fn insert(&mut self, col: u16, row: u16, entity: Entity) {
        self.slots.insert((col, row), entity);
    }

    /// Look up the entity at the given grid coordinates.
    #[must_use]
    pub fn get(&self, col: u16, row: u16) -> Option<Entity> {
        self.slots.get(&(col, row)).copied()
    }
}

// === Helper Functions ===

/// Convert a grid column to a world X position (center of the column).
#[must_use]
pub fn col_to_world_x(col: u16) -> f32 {
    f32::from(col).mul_add(CELL_SIZE, CELL_SIZE / 2.0)
}

/// Convert a grid row to a world Y position (center of the row).
#[must_use]
pub fn row_to_world_y(row: u16) -> f32 {
    f32::from(row).mul_add(CELL_SIZE, CELL_SIZE / 2.0)
}

/// Get the world-space center X of a zone given its start column and width in columns.
#[must_use]
pub(crate) fn zone_center_x(start_col: u16, width_cols: u16) -> f32 {
    f32::from(start_col).mul_add(CELL_SIZE, (f32::from(width_cols) * CELL_SIZE) / 2.0)
}

/// Center Y of the battlefield.
#[must_use]
pub(crate) fn battlefield_center_y() -> f32 {
    BATTLEFIELD_HEIGHT / 2.0
}

// === System Sets ===

/// System set for battlefield setup that runs on `OnEnter(GameState::InGame)`.
/// Other plugins can order their `OnEnter` systems relative to this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BattlefieldSetup;

// === Plugin ===

/// Battlefield plugin that spawns zone sprites and handles camera panning.
#[derive(Debug)]
pub struct BattlefieldPlugin;

impl Plugin for BattlefieldPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<PlayerFortress>()
            .register_type::<EnemyFortress>()
            .register_type::<BuildZone>()
            .register_type::<CombatZone>()
            .register_type::<BattlefieldBackground>()
            .register_type::<BuildSlot>()
            .init_resource::<GridIndex>();

        app.add_systems(
            OnEnter(GameState::InGame),
            (
                renderer::spawn_battlefield,
                camera::setup_camera_for_battlefield,
            )
                .chain()
                .in_set(BattlefieldSetup),
        )
        .add_systems(
            Update,
            camera::camera_pan.run_if(in_state(InGameState::Playing)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn total_cols_is_82() {
        assert_eq!(TOTAL_COLS, 82);
    }

    #[test]
    fn battlefield_dimensions_consistent() {
        assert_eq!(BATTLEFIELD_WIDTH, f32::from(TOTAL_COLS) * CELL_SIZE);
        assert_eq!(BATTLEFIELD_HEIGHT, f32::from(BATTLEFIELD_ROWS) * CELL_SIZE);
    }

    #[test]
    fn col_to_world_x_centers_in_cell() {
        assert_eq!(col_to_world_x(0), 32.0); // First cell center
        assert_eq!(col_to_world_x(1), 96.0); // Second cell center
    }

    #[test]
    fn row_to_world_y_centers_in_cell() {
        assert_eq!(row_to_world_y(0), 32.0);
        assert_eq!(row_to_world_y(9), 608.0); // Last row center
    }

    #[test]
    fn zone_start_columns_are_sequential() {
        assert_eq!(PLAYER_FORT_START_COL, 0);
        assert_eq!(BUILD_ZONE_START_COL, 2);
        assert_eq!(COMBAT_ZONE_START_COL, 8);
        assert_eq!(ENEMY_FORT_START_COL, 80);
    }

    #[test]
    fn zone_center_x_calculates_correctly() {
        // Player fortress: cols 0-1, center at col 1.0 * 64 / 2 = 64.0
        assert_eq!(zone_center_x(0, 2), 64.0);
        // Build zone: cols 2-7, center at 2*64 + 6*64/2 = 128 + 192 = 320.0
        assert_eq!(zone_center_x(BUILD_ZONE_START_COL, BUILD_ZONE_COLS), 320.0);
    }

    #[test]
    fn battlefield_center_y_is_half_height() {
        assert_eq!(battlefield_center_y(), BATTLEFIELD_HEIGHT / 2.0);
    }

    #[test]
    fn grid_index_insert_and_get() {
        let mut index = GridIndex::default();
        let entity = Entity::from_bits(42);
        index.insert(3, 5, entity);
        assert_eq!(index.get(3, 5), Some(entity));
        assert_eq!(index.get(0, 0), None);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Helper: set up an app with `BattlefieldPlugin` and transition to `InGame`.
    fn create_battlefield_test_app() -> App {
        let mut app = crate::testing::create_base_test_app();
        app.add_plugins(BattlefieldPlugin);
        crate::testing::transition_to_ingame(&mut app);
        app
    }

    #[test]
    fn spawn_battlefield_creates_expected_sprites() {
        let mut app = create_battlefield_test_app();
        let sprite_count = app
            .world_mut()
            .query_filtered::<(), With<Sprite>>()
            .iter(app.world())
            .count();
        assert_eq!(sprite_count, 65); // 5 zones + 60 grid cells
    }

    #[test]
    fn spawn_battlefield_creates_player_fortress() {
        let mut app = create_battlefield_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), With<PlayerFortress>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn spawn_battlefield_creates_enemy_fortress() {
        let mut app = create_battlefield_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), With<EnemyFortress>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn all_battlefield_entities_have_despawn_on_exit() {
        let mut app = create_battlefield_test_app();
        let with_despawn = app
            .world_mut()
            .query_filtered::<(), (With<Sprite>, With<DespawnOnExit<GameState>>)>()
            .iter(app.world())
            .count();
        assert_eq!(with_despawn, 65); // All 5 zones + 60 grid cells have DespawnOnExit
    }

    #[test]
    fn player_fortress_positioned_on_left() {
        let mut app = create_battlefield_test_app();
        let mut query = app
            .world_mut()
            .query_filtered::<&Transform, With<PlayerFortress>>();
        let transform = query.single(app.world()).unwrap();
        // Player fortress center should be near the left edge
        assert!(transform.translation.x < BATTLEFIELD_WIDTH / 4.0);
    }

    #[test]
    fn enemy_fortress_positioned_on_right() {
        let mut app = create_battlefield_test_app();
        let mut query = app
            .world_mut()
            .query_filtered::<&Transform, With<EnemyFortress>>();
        let transform = query.single(app.world()).unwrap();
        // Enemy fortress center should be near the right edge
        assert!(transform.translation.x > BATTLEFIELD_WIDTH * 3.0 / 4.0);
    }

    #[test]
    fn build_zone_marker_exists() {
        let mut app = create_battlefield_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), With<BuildZone>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn combat_zone_marker_exists() {
        let mut app = create_battlefield_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), With<CombatZone>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn background_marker_exists() {
        let mut app = create_battlefield_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), With<BattlefieldBackground>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn sixty_build_slots_spawned() {
        let mut app = create_battlefield_test_app();
        let count = app
            .world_mut()
            .query_filtered::<(), With<BuildSlot>>()
            .iter(app.world())
            .count();
        assert_eq!(count, 60); // 10 rows × 6 cols
    }

    #[test]
    fn build_slot_positions_match_grid() {
        let mut app = create_battlefield_test_app();
        let mut query = app.world_mut().query::<(&BuildSlot, &Transform)>();
        for (slot, transform) in query.iter(app.world()) {
            let expected_x = col_to_world_x(BUILD_ZONE_START_COL + slot.col);
            let expected_y = row_to_world_y(slot.row);
            assert_eq!(transform.translation.x, expected_x);
            assert_eq!(transform.translation.y, expected_y);
        }
    }

    #[test]
    fn build_slots_have_despawn_on_exit() {
        let mut app = create_battlefield_test_app();
        let with_despawn = app
            .world_mut()
            .query_filtered::<(), (With<BuildSlot>, With<DespawnOnExit<GameState>>)>()
            .iter(app.world())
            .count();
        assert_eq!(with_despawn, 60);
    }

    #[test]
    fn grid_index_has_sixty_entries() {
        let app = create_battlefield_test_app();
        let grid_index = app.world().resource::<GridIndex>();
        // Every (col, row) in the 6×10 grid should resolve to an entity
        let mut count = 0;
        for row in 0..BATTLEFIELD_ROWS {
            for col in 0..BUILD_ZONE_COLS {
                assert!(grid_index.get(col, row).is_some());
                count += 1;
            }
        }
        assert_eq!(count, 60);
    }

    #[test]
    fn grid_index_out_of_bounds_returns_none() {
        let app = create_battlefield_test_app();
        let grid_index = app.world().resource::<GridIndex>();
        assert!(grid_index.get(6, 0).is_none()); // col out of bounds
        assert!(grid_index.get(0, 10).is_none()); // row out of bounds
    }
}

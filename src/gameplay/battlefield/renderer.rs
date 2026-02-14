//! Battlefield zone and entity spawning.

use bevy::prelude::*;

use super::{
    BATTLEFIELD_HEIGHT, BATTLEFIELD_ROWS, BATTLEFIELD_WIDTH, BUILD_ZONE_COLS, BUILD_ZONE_START_COL,
    BattlefieldBackground, BuildSlot, BuildZone, CELL_SIZE, COMBAT_ZONE_COLS,
    COMBAT_ZONE_START_COL, CombatZone, ENEMY_FORT_START_COL, EnemyFortress, FORTRESS_COLS,
    GridIndex, PLAYER_FORT_START_COL, PlayerFortress, battlefield_center_y, col_to_world_x,
    row_to_world_y, zone_center_x,
};
use crate::gameplay::units::{Target, Team};
use crate::screens::GameState;
use crate::{Z_BACKGROUND, Z_GRID, Z_ZONE};

/// Color for individual grid cells in the build zone.
const GRID_CELL_COLOR: Color = Color::srgb(0.3, 0.3, 0.4);

// === Zone Colors ===

const PLAYER_FORT_COLOR: Color = Color::srgb(0.2, 0.3, 0.8);
const ENEMY_FORT_COLOR: Color = Color::srgb(0.8, 0.2, 0.2);
const BUILD_ZONE_COLOR: Color = Color::srgb(0.25, 0.25, 0.35);
const COMBAT_ZONE_COLOR: Color = Color::srgb(0.15, 0.15, 0.2);
const BACKGROUND_COLOR: Color = Color::srgb(0.1, 0.1, 0.12);

/// Spawns all battlefield entities: zone sprites with markers, and build slot grid.
pub(super) fn spawn_battlefield(mut commands: Commands, mut grid_index: ResMut<GridIndex>) {
    let fortress_size = Vec2::new(f32::from(FORTRESS_COLS) * CELL_SIZE, BATTLEFIELD_HEIGHT);

    // Background (slightly larger than battlefield for visual framing)
    commands.spawn((
        BattlefieldBackground,
        Sprite::from_color(
            BACKGROUND_COLOR,
            Vec2::new(BATTLEFIELD_WIDTH + 128.0, BATTLEFIELD_HEIGHT + 128.0),
        ),
        Transform::from_xyz(
            BATTLEFIELD_WIDTH / 2.0,
            battlefield_center_y(),
            Z_BACKGROUND,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Player fortress (blue)
    commands.spawn((
        PlayerFortress,
        Team::Player,
        Target,
        Sprite::from_color(PLAYER_FORT_COLOR, fortress_size),
        Transform::from_xyz(
            zone_center_x(PLAYER_FORT_START_COL, FORTRESS_COLS),
            battlefield_center_y(),
            Z_ZONE,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Building zone (dark blue-gray)
    commands.spawn((
        BuildZone,
        Sprite::from_color(
            BUILD_ZONE_COLOR,
            Vec2::new(f32::from(BUILD_ZONE_COLS) * CELL_SIZE, BATTLEFIELD_HEIGHT),
        ),
        Transform::from_xyz(
            zone_center_x(BUILD_ZONE_START_COL, BUILD_ZONE_COLS),
            battlefield_center_y(),
            Z_ZONE,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Combat zone (dark gray)
    commands.spawn((
        CombatZone,
        Sprite::from_color(
            COMBAT_ZONE_COLOR,
            Vec2::new(f32::from(COMBAT_ZONE_COLS) * CELL_SIZE, BATTLEFIELD_HEIGHT),
        ),
        Transform::from_xyz(
            zone_center_x(COMBAT_ZONE_START_COL, COMBAT_ZONE_COLS),
            battlefield_center_y(),
            Z_ZONE,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Enemy fortress (red)
    commands.spawn((
        EnemyFortress,
        Team::Enemy,
        Target,
        Sprite::from_color(ENEMY_FORT_COLOR, fortress_size),
        Transform::from_xyz(
            zone_center_x(ENEMY_FORT_START_COL, FORTRESS_COLS),
            battlefield_center_y(),
            Z_ZONE,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Build slots: 10 rows × 6 cols — visible grid cells, indexed for O(1) lookup
    for row in 0..BATTLEFIELD_ROWS {
        for col in 0..BUILD_ZONE_COLS {
            let entity = commands
                .spawn((
                    BuildSlot { row, col },
                    Sprite::from_color(GRID_CELL_COLOR, Vec2::splat(CELL_SIZE - 2.0)),
                    Transform::from_xyz(
                        col_to_world_x(BUILD_ZONE_START_COL + col),
                        row_to_world_y(row),
                        Z_GRID,
                    ),
                    DespawnOnExit(GameState::InGame),
                ))
                .id();
            grid_index.insert(col, row, entity);
        }
    }
}

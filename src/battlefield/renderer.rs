//! Battlefield zone and entity spawning.

#![allow(clippy::cast_precision_loss)] // Grid values are small; u32->f32 is safe.

use bevy::prelude::*;

use super::{
    BATTLEFIELD_HEIGHT, BATTLEFIELD_ROWS, BATTLEFIELD_WIDTH, BUILD_ZONE_COLS, BUILD_ZONE_START_COL,
    BattlefieldBackground, BuildSlot, BuildZone, CELL_SIZE, COMBAT_ZONE_COLS,
    COMBAT_ZONE_START_COL, CombatZone, ENEMY_FORT_START_COL, EnemyFortress, FORTRESS_COLS,
    PLAYER_FORT_START_COL, PlayerFortress, battlefield_center_y, col_to_world_x, row_to_world_y,
    zone_center_x,
};
use crate::GameState;

// === Zone Colors ===

const PLAYER_FORT_COLOR: Color = Color::srgb(0.2, 0.3, 0.8);
const ENEMY_FORT_COLOR: Color = Color::srgb(0.8, 0.2, 0.2);
const BUILD_ZONE_COLOR: Color = Color::srgb(0.25, 0.25, 0.35);
const COMBAT_ZONE_COLOR: Color = Color::srgb(0.15, 0.15, 0.2);
const BACKGROUND_COLOR: Color = Color::srgb(0.1, 0.1, 0.12);

/// Spawns all battlefield entities: zone sprites with markers, and data-only build slots.
pub(super) fn spawn_battlefield(mut commands: Commands) {
    let fortress_size = Vec2::new(FORTRESS_COLS as f32 * CELL_SIZE, BATTLEFIELD_HEIGHT);

    // Background (slightly larger than battlefield for visual framing)
    commands.spawn((
        BattlefieldBackground,
        Sprite::from_color(
            BACKGROUND_COLOR,
            Vec2::new(BATTLEFIELD_WIDTH + 128.0, BATTLEFIELD_HEIGHT + 128.0),
        ),
        Transform::from_xyz(BATTLEFIELD_WIDTH / 2.0, battlefield_center_y(), -1.0),
        DespawnOnExit(GameState::InGame),
    ));

    // Player fortress (blue)
    commands.spawn((
        PlayerFortress,
        Sprite::from_color(PLAYER_FORT_COLOR, fortress_size),
        Transform::from_xyz(
            zone_center_x(PLAYER_FORT_START_COL, FORTRESS_COLS),
            battlefield_center_y(),
            0.0,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Building zone (dark blue-gray)
    commands.spawn((
        BuildZone,
        Sprite::from_color(
            BUILD_ZONE_COLOR,
            Vec2::new(BUILD_ZONE_COLS as f32 * CELL_SIZE, BATTLEFIELD_HEIGHT),
        ),
        Transform::from_xyz(
            zone_center_x(BUILD_ZONE_START_COL, BUILD_ZONE_COLS),
            battlefield_center_y(),
            0.0,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Combat zone (dark gray)
    commands.spawn((
        CombatZone,
        Sprite::from_color(
            COMBAT_ZONE_COLOR,
            Vec2::new(COMBAT_ZONE_COLS as f32 * CELL_SIZE, BATTLEFIELD_HEIGHT),
        ),
        Transform::from_xyz(
            zone_center_x(COMBAT_ZONE_START_COL, COMBAT_ZONE_COLS),
            battlefield_center_y(),
            0.0,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Enemy fortress (red)
    commands.spawn((
        EnemyFortress,
        Sprite::from_color(ENEMY_FORT_COLOR, fortress_size),
        Transform::from_xyz(
            zone_center_x(ENEMY_FORT_START_COL, FORTRESS_COLS),
            battlefield_center_y(),
            0.0,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Build slots: 10 rows × 6 cols = 60 data-only entities (no visual yet — Ticket 2 adds UI)
    for row in 0..BATTLEFIELD_ROWS {
        for col in 0..BUILD_ZONE_COLS {
            commands.spawn((
                BuildSlot { row, col },
                Transform::from_xyz(
                    col_to_world_x(BUILD_ZONE_START_COL + col),
                    row_to_world_y(row),
                    0.0,
                ),
                DespawnOnExit(GameState::InGame),
            ));
        }
    }
}

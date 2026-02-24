//! Battlefield zone and entity spawning.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{
    BATTLEFIELD_HEIGHT, BATTLEFIELD_ROWS, BATTLEFIELD_WIDTH, BUILD_ZONE_COLS, BUILD_ZONE_START_COL,
    BattlefieldBackground, BuildSlot, BuildZone, CELL_SIZE, COMBAT_ZONE_COLS,
    COMBAT_ZONE_START_COL, CombatZone, ENEMY_FORT_START_COL, EnemyFortress, FORTRESS_ATTACK_SPEED,
    FORTRESS_COLS, FORTRESS_DAMAGE, FORTRESS_HEALTH_BAR_HEIGHT, FORTRESS_HEALTH_BAR_WIDTH,
    FORTRESS_HEALTH_BAR_Y_OFFSET, FORTRESS_HP, FORTRESS_RANGE, FORTRESS_ROWS, GridIndex,
    PLAYER_FORT_START_COL, PlayerFortress, battlefield_center_y, col_to_world_x, row_to_world_y,
    zone_center_x,
};
use crate::gameplay::combat::{AttackTimer, HealthBarConfig};
use crate::gameplay::units::UNIT_RADIUS;
use crate::gameplay::{CombatStats, CurrentTarget, Health, Target, Team};
use crate::screens::GameState;
use crate::third_party::{CollisionLayer, NavObstacle};
use crate::{Z_BACKGROUND, Z_FORTRESS, Z_GRID, Z_ZONE};
use vleue_navigator::prelude::*;

use crate::theme::palette;

/// Spawns all battlefield entities: zone sprites with markers, and build slot grid.
#[allow(clippy::too_many_lines)]
pub(super) fn spawn_battlefield(mut commands: Commands, mut grid_index: ResMut<GridIndex>) {
    let fortress_size = Vec2::new(
        f32::from(FORTRESS_COLS) * CELL_SIZE,
        f32::from(FORTRESS_ROWS) * CELL_SIZE,
    );

    // Background (slightly larger than battlefield for visual framing)
    commands.spawn((
        Name::new("Battlefield Background"),
        BattlefieldBackground,
        Sprite::from_color(
            palette::BACKGROUND,
            Vec2::new(BATTLEFIELD_WIDTH + 128.0, BATTLEFIELD_HEIGHT + 128.0),
        ),
        Transform::from_xyz(
            BATTLEFIELD_WIDTH / 2.0,
            battlefield_center_y(),
            Z_BACKGROUND,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    let fortress_zone_size = Vec2::new(f32::from(FORTRESS_COLS) * CELL_SIZE, BATTLEFIELD_HEIGHT);

    // Player fortress zone backdrop (full-height, behind the fortress entity)
    commands.spawn((
        Name::new("Player Fortress Zone"),
        Sprite::from_color(palette::COMBAT_ZONE, fortress_zone_size),
        Transform::from_xyz(
            zone_center_x(PLAYER_FORT_START_COL, FORTRESS_COLS),
            battlefield_center_y(),
            Z_ZONE,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Player fortress (blue)
    commands
        .spawn((
            Name::new("Player Fortress"),
            PlayerFortress,
            Team::Player,
            Target,
            Health::new(FORTRESS_HP),
            HealthBarConfig {
                width: FORTRESS_HEALTH_BAR_WIDTH,
                height: FORTRESS_HEALTH_BAR_HEIGHT,
                y_offset: FORTRESS_HEALTH_BAR_Y_OFFSET,
            },
            CombatStats {
                damage: FORTRESS_DAMAGE,
                attack_speed: FORTRESS_ATTACK_SPEED,
                range: FORTRESS_RANGE,
            },
            AttackTimer(Timer::from_seconds(
                1.0 / FORTRESS_ATTACK_SPEED,
                TimerMode::Repeating,
            )),
            CurrentTarget(None),
            Sprite::from_color(palette::PLAYER_FORTRESS, fortress_size),
            Transform::from_xyz(
                zone_center_x(PLAYER_FORT_START_COL, FORTRESS_COLS),
                battlefield_center_y(),
                Z_FORTRESS,
            ),
            DespawnOnExit(GameState::InGame),
        ))
        .insert((
            NavObstacle,
            RigidBody::Static,
            Collider::rectangle(fortress_size.x, fortress_size.y),
            CollisionLayers::new(
                [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
                [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
            ),
        ));

    // Building zone (dark blue-gray)
    commands.spawn((
        Name::new("Build Zone"),
        BuildZone,
        Sprite::from_color(
            palette::BUILD_ZONE,
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
        Name::new("Combat Zone"),
        CombatZone,
        Sprite::from_color(
            palette::COMBAT_ZONE,
            Vec2::new(f32::from(COMBAT_ZONE_COLS) * CELL_SIZE, BATTLEFIELD_HEIGHT),
        ),
        Transform::from_xyz(
            zone_center_x(COMBAT_ZONE_START_COL, COMBAT_ZONE_COLS),
            battlefield_center_y(),
            Z_ZONE,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Enemy fortress zone backdrop (full-height, behind the fortress entity)
    commands.spawn((
        Name::new("Enemy Fortress Zone"),
        Sprite::from_color(palette::COMBAT_ZONE, fortress_zone_size),
        Transform::from_xyz(
            zone_center_x(ENEMY_FORT_START_COL, FORTRESS_COLS),
            battlefield_center_y(),
            Z_ZONE,
        ),
        DespawnOnExit(GameState::InGame),
    ));

    // Enemy fortress (red)
    commands
        .spawn((
            Name::new("Enemy Fortress"),
            EnemyFortress,
            Team::Enemy,
            Target,
            Health::new(FORTRESS_HP),
            HealthBarConfig {
                width: FORTRESS_HEALTH_BAR_WIDTH,
                height: FORTRESS_HEALTH_BAR_HEIGHT,
                y_offset: FORTRESS_HEALTH_BAR_Y_OFFSET,
            },
            CombatStats {
                damage: FORTRESS_DAMAGE,
                attack_speed: FORTRESS_ATTACK_SPEED,
                range: FORTRESS_RANGE,
            },
            AttackTimer(Timer::from_seconds(
                1.0 / FORTRESS_ATTACK_SPEED,
                TimerMode::Repeating,
            )),
            CurrentTarget(None),
            Sprite::from_color(palette::ENEMY_FORTRESS, fortress_size),
            Transform::from_xyz(
                zone_center_x(ENEMY_FORT_START_COL, FORTRESS_COLS),
                battlefield_center_y(),
                Z_FORTRESS,
            ),
            DespawnOnExit(GameState::InGame),
        ))
        .insert((
            NavObstacle,
            RigidBody::Static,
            Collider::rectangle(fortress_size.x, fortress_size.y),
            CollisionLayers::new(
                [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
                [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
            ),
        ));

    // Build slots: 10 rows × 6 cols — visible grid cells, indexed for O(1) lookup
    for row in 0..BATTLEFIELD_ROWS {
        for col in 0..BUILD_ZONE_COLS {
            let entity = commands
                .spawn((
                    Name::new(format!("Build Slot ({col}, {row})")),
                    BuildSlot { row, col },
                    Sprite::from_color(palette::GRID_CELL, Vec2::splat(CELL_SIZE - 2.0)),
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

    // NavMesh for unit pathfinding — covers the full battlefield.
    // Obstacles (buildings, fortresses with NavObstacle marker) are auto-carved by
    // NavmeshUpdaterPlugin. Agent radius ensures paths keep unit centers clear.
    commands.spawn((
        Name::new("Battlefield NavMesh"),
        NavMeshSettings {
            fixed: Triangulation::from_outer_edges(&[
                Vec2::new(0.0, 0.0),
                Vec2::new(BATTLEFIELD_WIDTH, 0.0),
                Vec2::new(BATTLEFIELD_WIDTH, BATTLEFIELD_HEIGHT),
                Vec2::new(0.0, BATTLEFIELD_HEIGHT),
            ]),
            agent_radius: UNIT_RADIUS,
            ..default()
        },
        NavMeshUpdateMode::Direct,
        DespawnOnExit(GameState::InGame),
    ));
}

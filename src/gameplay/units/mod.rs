//! Unit components, constants, and shared rendering assets.

pub mod avoidance;
mod movement;
pub mod pathfinding;
pub mod spawn;

use avian2d::prelude::*;
use bevy::prelude::*;
use vleue_navigator::prelude::NavMesh;

use self::avoidance::spatial_hash::SpatialHash;
use self::avoidance::{AvoidanceAgent, AvoidanceConfig, PreferredVelocity};
use crate::gameplay::combat::{
    AttackTimer, HealthBarConfig, UNIT_HEALTH_BAR_HEIGHT, UNIT_HEALTH_BAR_WIDTH,
    UNIT_HEALTH_BAR_Y_OFFSET,
};
use crate::gameplay::{CombatStats, CurrentTarget, Health, Movement, Target, Team};
use crate::screens::GameState;
use crate::third_party::solid_entity_layers;
use crate::{GameSet, Z_UNIT, gameplay_running};

// === Constants ===

/// Visual radius of a unit circle.
pub const UNIT_RADIUS: f32 = 6.0;

use crate::theme::palette;

// === Components ===

/// Marker for unit entities.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Unit;

// === Unit Type System ===

/// Types of units in the game.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub enum UnitType {
    Soldier,
}

impl UnitType {
    /// All unit types, for iteration.
    #[allow(dead_code)] // Used in tests; will be used by future unit type additions
    pub const ALL: &[Self] = &[Self::Soldier];

    /// Human-readable display name.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Soldier => "Soldier",
        }
    }
}

/// Stats for a unit type. All values are compile-time constants.
#[derive(Debug, Clone, Copy)]
pub struct UnitStats {
    pub hp: f32,
    pub damage: f32,
    pub attack_speed: f32,
    pub move_speed: f32,
    pub attack_range: f32,
}

/// Look up stats for a unit type.
#[must_use]
pub const fn unit_stats(unit_type: UnitType) -> UnitStats {
    match unit_type {
        UnitType::Soldier => UnitStats {
            hp: 100.0,
            damage: 10.0,
            attack_speed: 1.0,
            move_speed: 50.0,
            attack_range: 5.0,
        },
    }
}

/// Spawn a unit entity with all required components.
/// Single source of truth for the unit archetype.
pub fn spawn_unit(
    commands: &mut Commands,
    unit_type: UnitType,
    team: Team,
    position: Vec2,
    assets: &UnitAssets,
) -> Entity {
    let stats = unit_stats(unit_type);
    let material = match team {
        Team::Player => assets.player_material.clone(),
        Team::Enemy => assets.enemy_material.clone(),
    };

    commands
        .spawn((
            Name::new(format!("{team:?} {}", unit_type.display_name())),
            Unit,
            unit_type,
            team,
            Target,
            CurrentTarget(None),
            Health::new(stats.hp),
            HealthBarConfig {
                width: UNIT_HEALTH_BAR_WIDTH,
                height: UNIT_HEALTH_BAR_HEIGHT,
                y_offset: UNIT_HEALTH_BAR_Y_OFFSET,
            },
            CombatStats {
                damage: stats.damage,
                attack_speed: stats.attack_speed,
                range: stats.attack_range,
            },
            Movement {
                speed: stats.move_speed,
            },
            AttackTimer(Timer::from_seconds(
                1.0 / stats.attack_speed,
                TimerMode::Repeating,
            )),
            Mesh2d(assets.mesh.clone()),
            MeshMaterial2d(material),
            Transform::from_xyz(position.x, position.y, Z_UNIT),
            DespawnOnExit(GameState::InGame),
        ))
        .insert((
            pathfinding::NavPath::default(),
            RigidBody::Dynamic,
            Collider::circle(UNIT_RADIUS),
            solid_entity_layers(),
            LockedAxes::ROTATION_LOCKED,
            LinearVelocity::ZERO,
            PreferredVelocity::default(),
            AvoidanceAgent::default(),
        ))
        .id()
}

// === Spawn Placement ===

/// Max retry attempts for finding a navigable spawn point.
const SPAWN_PLACEMENT_ATTEMPTS: u32 = 8;

/// Pick a random position at `radius` from `center` that is navigable.
///
/// Tries up to `SPAWN_PLACEMENT_ATTEMPTS` random angles. When `navmesh` is `Some`,
/// each candidate is validated with `is_in_mesh()`. When `None` (navmesh not built
/// yet), returns the first random point without validation.
///
/// Falls back to `center` if all attempts land outside the mesh.
pub fn random_navigable_spawn(center: Vec2, radius: f32, navmesh: Option<&NavMesh>) -> Vec2 {
    use rand::Rng;
    let mut rng = rand::rng();

    for _ in 0..SPAWN_PLACEMENT_ATTEMPTS {
        let angle = rng.random_range(0.0..std::f32::consts::TAU);
        let point = Vec2::new(
            radius.mul_add(angle.cos(), center.x),
            radius.mul_add(angle.sin(), center.y),
        );

        if navmesh.is_none_or(|mesh| mesh.is_in_mesh(point)) {
            return point;
        }
    }

    // All attempts failed — spawn at center (pathfinding handles off-mesh start)
    center
}

// === Resources ===

/// Shared mesh and material handles for unit circle rendering.
#[derive(Resource, Debug)]
pub struct UnitAssets {
    pub mesh: Handle<Mesh>,
    pub player_material: Handle<ColorMaterial>,
    pub enemy_material: Handle<ColorMaterial>,
}

// === Systems ===

fn setup_unit_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    existing: Option<Res<UnitAssets>>,
) {
    if existing.is_some() {
        return; // Already created — don't leak handles
    }
    commands.insert_resource(UnitAssets {
        mesh: meshes.add(Circle::new(UNIT_RADIUS)),
        player_material: materials.add(palette::PLAYER_UNIT),
        enemy_material: materials.add(palette::ENEMY_UNIT),
    });
}

fn reset_path_refresh_timer(mut commands: Commands) {
    commands.insert_resource(pathfinding::PathRefreshTimer::default());
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Unit>()
        .register_type::<UnitType>()
        .register_type::<PreferredVelocity>()
        .register_type::<AvoidanceAgent>()
        .register_type::<AvoidanceConfig>()
        .register_type::<pathfinding::NavPath>()
        .register_type::<pathfinding::PathRefreshTimer>()
        .init_resource::<pathfinding::PathRefreshTimer>()
        .init_resource::<AvoidanceConfig>();

    let config = AvoidanceConfig::default();
    app.insert_resource(SpatialHash::new(config.neighbor_distance));

    app.add_systems(
        OnEnter(GameState::InGame),
        (setup_unit_assets, reset_path_refresh_timer),
    );

    spawn::plugin(app);

    app.add_systems(
        Update,
        (
            pathfinding::compute_paths
                .in_set(GameSet::Ai)
                .after(crate::gameplay::ai::find_target),
            (
                movement::unit_movement,
                avoidance::rebuild_spatial_hash,
                avoidance::compute_avoidance,
            )
                .chain_ignore_deferred()
                .in_set(GameSet::Movement),
        )
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn health_new_sets_current_to_max() {
        let health = crate::gameplay::Health::new(100.0);
        assert_eq!(health.current, 100.0);
        assert_eq!(health.max, 100.0);
    }

    #[test]
    fn team_variants_are_distinct() {
        use crate::gameplay::Team;
        assert_ne!(Team::Player, Team::Enemy);
    }

    #[test]
    fn team_opposing_returns_other_team() {
        use crate::gameplay::Team;
        assert_eq!(Team::Player.opposing(), Team::Enemy);
        assert_eq!(Team::Enemy.opposing(), Team::Player);
    }

    #[test]
    fn soldier_stats_are_positive() {
        let stats = unit_stats(UnitType::Soldier);
        assert!(stats.hp > 0.0);
        assert!(stats.damage > 0.0);
        assert!(stats.attack_speed > 0.0);
        assert!(stats.move_speed > 0.0);
        assert!(stats.attack_range > 0.0);
    }

    #[test]
    fn unit_type_display_name() {
        assert_eq!(UnitType::Soldier.display_name(), "Soldier");
    }

    #[test]
    fn unit_type_all_contains_soldier() {
        assert!(UnitType::ALL.contains(&UnitType::Soldier));
    }

    #[test]
    fn random_navigable_spawn_correct_distance_without_navmesh() {
        let center = Vec2::new(100.0, 200.0);
        let radius = 40.0;
        let result = random_navigable_spawn(center, radius, None);
        let dist = center.distance(result);
        assert!(
            (dist - radius).abs() < 0.01,
            "Expected distance {radius}, got {dist}"
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::testing::transition_to_ingame;

    #[test]
    fn unit_assets_created_on_enter_ingame() {
        let mut app = crate::testing::create_base_test_app();
        crate::testing::init_asset_resources(&mut app);
        app.add_plugins(plugin);
        transition_to_ingame(&mut app);

        assert!(app.world().get_resource::<UnitAssets>().is_some());
    }
}

//! Unit components, constants, and shared rendering assets.

mod movement;
pub mod pathfinding;
pub mod spawn;

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::gameplay::combat::{
    AttackTimer, HealthBarConfig, UNIT_HEALTH_BAR_HEIGHT, UNIT_HEALTH_BAR_WIDTH,
    UNIT_HEALTH_BAR_Y_OFFSET,
};
use crate::gameplay::{CombatStats, CurrentTarget, Health, Movement, Target, Team};
use crate::screens::GameState;
use crate::third_party::CollisionLayer;
use crate::{GameSet, Z_UNIT, gameplay_running};

// === Constants ===

/// Visual radius of a unit circle.
pub const UNIT_RADIUS: f32 = 6.0;

/// Player unit color (green).
const PLAYER_UNIT_COLOR: Color = Color::srgb(0.2, 0.8, 0.2);

/// Enemy unit color (red).
const ENEMY_UNIT_COLOR: Color = Color::srgb(0.8, 0.2, 0.2);

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
    position: Vec3,
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
            CollisionLayers::new(
                [CollisionLayer::Pushbox, CollisionLayer::Hurtbox],
                [CollisionLayer::Pushbox, CollisionLayer::Hitbox],
            ),
            LockedAxes::ROTATION_LOCKED,
            LinearVelocity::ZERO,
        ))
        .id()
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
) {
    commands.insert_resource(UnitAssets {
        mesh: meshes.add(Circle::new(UNIT_RADIUS)),
        player_material: materials.add(PLAYER_UNIT_COLOR),
        enemy_material: materials.add(ENEMY_UNIT_COLOR),
    });
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Unit>()
        .register_type::<UnitType>()
        .register_type::<pathfinding::NavPath>()
        .register_type::<pathfinding::PathRefreshTimer>()
        .init_resource::<pathfinding::PathRefreshTimer>();

    app.add_systems(OnEnter(GameState::InGame), setup_unit_assets);

    spawn::plugin(app);

    app.add_systems(
        Update,
        (
            pathfinding::compute_paths
                .in_set(GameSet::Ai)
                .after(crate::gameplay::ai::find_target),
            movement::unit_movement.in_set(GameSet::Movement),
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
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::testing::transition_to_ingame;

    #[test]
    fn unit_assets_created_on_enter_ingame() {
        let mut app = crate::testing::create_base_test_app();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();
        app.add_plugins(plugin);
        transition_to_ingame(&mut app);

        assert!(app.world().get_resource::<UnitAssets>().is_some());
    }
}

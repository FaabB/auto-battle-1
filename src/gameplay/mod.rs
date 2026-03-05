//! Gameplay domain plugins and cross-cutting components.
//!
//! # Entity Archetypes
//!
//! **Units**: `Unit`, `Team`, `Target`, `TargetingState`, `Health`, `CombatStats`, `Movement`,
//!           `AttackTimer`, `HealthBarConfig`, `Mesh2d`, `MeshMaterial2d`,
//!           `RigidBody::Dynamic`, `Collider`, `CollisionLayers`, `LockedAxes`, `LinearVelocity`
//!
//! **Buildings**: `Building`, `Team`, `Target`, `Health`, `HealthBarConfig`,
//!           `ProductionTimer` or `IncomeTimer`, `RigidBody::Static`, `Collider`, `CollisionLayers`
//!
//! **Fortresses**: `PlayerFortress`/`EnemyFortress`, `Team`, `Target`, `TargetingState`,
//!           `Health`, `CombatStats`, `AttackTimer`, `HealthBarConfig`,
//!           `RigidBody::Static`, `Collider`, `CollisionLayers`
//!
//! **Projectiles**: `Projectile`, `Team`, `Hitbox`, `Sensor`, `RigidBody::Kinematic`,
//!           `Collider`, `CollisionLayers`, `CollisionEventsEnabled`, `CollidingEntities`

pub mod ai;
pub mod battlefield;
pub mod building;
pub mod combat;
pub mod economy;
pub mod endgame_detection;
mod hud;
pub mod spatial_hash;
pub mod units;

use bevy::prelude::*;

// === Cross-Cutting Components ===

/// Which side an entity belongs to. Used on units, buildings, and fortresses.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[reflect(Component)]
pub enum Team {
    Player,
    Enemy,
}

impl Team {
    /// Returns the opposing team.
    #[must_use]
    pub const fn opposing(self) -> Self {
        match self {
            Self::Player => Self::Enemy,
            Self::Enemy => Self::Player,
        }
    }
}

/// Hit points for any damageable entity (units, buildings, fortresses).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Health {
    #[must_use]
    pub const fn new(max: f32) -> Self {
        Self { current: max, max }
    }
}

/// Marker: this entity can be targeted by units.
/// Placed on units, buildings, and fortresses.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Target;

/// State machine for targeting behavior.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Reflect)]
#[reflect(Component)]
pub enum TargetingState {
    /// Following flow field toward assigned goal. No spatial queries.
    Moving,
    /// Looking for targets. Default state for static entities (fortresses).
    Seeking,
    /// Locked onto a target. Movement system steers directly toward it.
    Engaging(Entity),
    /// In attack range, firing. Velocity = 0.
    Attacking(Entity),
}

impl TargetingState {
    /// Returns the target entity if in `Engaging` or `Attacking` state.
    #[must_use]
    pub const fn target_entity(self) -> Option<Entity> {
        match self {
            Self::Engaging(e) | Self::Attacking(e) => Some(e),
            Self::Moving | Self::Seeking => None,
        }
    }
}

/// Leash that pulls a unit back to Seeking if it moves too far from origin.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct EngagementLeash {
    pub origin: Vec2,
    pub max_distance: f32,
}

/// Default leash distance in pixels (3 cells).
#[allow(dead_code)]
pub const LEASH_DISTANCE: f32 = 192.0;

/// Movement speed for any mobile entity.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Movement {
    pub speed: f32,
}

/// Combat parameters for any attacking entity (units, fortresses, future turrets).
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct CombatStats {
    pub damage: f32,
    pub attack_speed: f32,
    pub range: f32,
}

/// Virtual time when the current game started.
/// Used to compute elapsed game time for the HUD.
#[derive(Resource, Debug, Default, Reflect)]
#[reflect(Resource)]
pub struct GameStartTime(pub f32);

pub fn plugin(app: &mut App) {
    app.register_type::<Team>()
        .register_type::<Health>()
        .register_type::<Target>()
        .register_type::<TargetingState>()
        .register_type::<EngagementLeash>()
        .register_type::<Movement>()
        .register_type::<CombatStats>()
        .register_type::<GameStartTime>()
        .init_resource::<GameStartTime>();

    app.add_plugins((
        ai::plugin,
        battlefield::plugin,
        building::plugin,
        combat::plugin,
        economy::plugin,
        endgame_detection::plugin,
        hud::plugin,
        units::plugin,
    ));
}

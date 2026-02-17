//! Gameplay domain plugins and cross-cutting components.
//!
//! # Entity Archetypes
//!
//! **Units**: `Unit`, `Team`, `Target`, `CurrentTarget`, `Health`, `CombatStats`, `Movement`,
//!           `AttackTimer`, `HealthBarConfig`, `Mesh2d`, `MeshMaterial2d`,
//!           `RigidBody::Dynamic`, `Collider`, `CollisionLayers`, `LockedAxes`, `LinearVelocity`
//!
//! **Buildings**: `Building`, `Team`, `Target`, `Health`, `HealthBarConfig`,
//!           `ProductionTimer` or `IncomeTimer`, `RigidBody::Static`, `Collider`, `CollisionLayers`
//!
//! **Fortresses**: `PlayerFortress`/`EnemyFortress`, `Team`, `Target`, `CurrentTarget`,
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

/// Tracks the entity this entity is currently moving toward / attacking.
/// Updated by the AI system; read by movement and combat systems.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct CurrentTarget(pub Option<Entity>);

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

pub fn plugin(app: &mut App) {
    app.register_type::<Team>()
        .register_type::<Health>()
        .register_type::<Target>()
        .register_type::<CurrentTarget>()
        .register_type::<Movement>()
        .register_type::<CombatStats>();

    app.add_plugins((
        ai::plugin,
        battlefield::plugin,
        building::plugin,
        combat::plugin,
        economy::plugin,
        endgame_detection::plugin,
        units::plugin,
    ));
}

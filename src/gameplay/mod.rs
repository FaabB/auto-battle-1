//! Gameplay domain plugins and cross-cutting components.
//!
//! # Entity Archetypes
//!
//! **Units**: `Unit`, `Team`, `Target`, `CurrentTarget`, `Health`, `CombatStats`, `Movement`,
//!           `AttackTimer`, `HealthBarConfig`, `Mesh2d`, `MeshMaterial2d`,
//!           `RigidBody::Dynamic`, `Collider`, `LockedAxes`, `LinearVelocity`
//!
//! **Buildings**: `Building`, `Team`, `Target`, `Health`, `HealthBarConfig`,
//!           `ProductionTimer` or `IncomeTimer`, `RigidBody::Static`, `Collider`
//!
//! **Fortresses**: `PlayerFortress`/`EnemyFortress`, `Team`, `Target`, `Health`,
//!           `HealthBarConfig`, `RigidBody::Static`, `Collider`

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

pub fn plugin(app: &mut App) {
    app.register_type::<Team>()
        .register_type::<Health>()
        .register_type::<Target>();

    app.add_plugins((
        battlefield::plugin,
        building::plugin,
        combat::plugin,
        economy::plugin,
        endgame_detection::plugin,
        units::plugin,
    ));
}

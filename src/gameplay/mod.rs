//! Gameplay domain plugins and cross-cutting components.
//!
//! # Entity Archetypes
//!
//! **Units**: `Unit`, `Team`, `Target`, `TargetingState`, `Health`, `CombatStats`, `Movement`,
//!           `AttackTimer`, `HealthBarConfig`, `EntityExtent`, `Mesh2d`, `MeshMaterial2d`,
//!           `RigidBody::Dynamic`, `Collider`, `CollisionLayers`, `LockedAxes`, `LinearVelocity`
//!
//! **Buildings**: `Building`, `Team`, `Target`, `Health`, `HealthBarConfig`, `EntityExtent`,
//!           `ProductionTimer` or `IncomeTimer`, `RigidBody::Static`, `Collider`, `CollisionLayers`
//!
//! **Fortresses**: `PlayerFortress`/`EnemyFortress`, `Team`, `Target`, `TargetingState`,
//!           `Health`, `CombatStats`, `AttackTimer`, `HealthBarConfig`, `EntityExtent`,
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

/// Physical extent of a targetable entity, used for surface-distance range checks.
/// Replaces GJK `surface_distance()` with simple geometry.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub enum EntityExtent {
    /// Circular extent with the given radius (units).
    Circle(f32),
    /// Rectangular extent with half-width and half-height (fortresses, buildings).
    Rect(f32, f32),
}

impl EntityExtent {
    /// Minimum distance from `point` to the surface of this extent centered at `self_pos`.
    /// Returns 0.0 if the point is inside or overlapping.
    #[must_use]
    pub fn surface_distance_from(&self, self_pos: Vec2, point: Vec2) -> f32 {
        match self {
            Self::Circle(r) => (self_pos.distance(point) - r).max(0.0),
            Self::Rect(hw, hh) => {
                let d = (point - self_pos).abs();
                let dx = (d.x - hw).max(0.0);
                let dy = (d.y - hh).max(0.0);
                dx.hypot(dy)
            }
        }
    }
}

/// Surface-to-surface distance between two extents. Returns 0.0 if overlapping.
/// Drop-in replacement for `third_party::surface_distance()`.
#[must_use]
#[allow(clippy::similar_names)]
pub fn extent_distance(a: &EntityExtent, a_pos: Vec2, b: &EntityExtent, b_pos: Vec2) -> f32 {
    match (a, b) {
        (EntityExtent::Circle(r1), EntityExtent::Circle(r2)) => {
            (a_pos.distance(b_pos) - r1 - r2).max(0.0)
        }
        (EntityExtent::Circle(r), EntityExtent::Rect(hw, hh))
        | (EntityExtent::Rect(hw, hh), EntityExtent::Circle(r)) => {
            let (circle_pos, rect_pos) = if matches!(a, EntityExtent::Circle(_)) {
                (a_pos, b_pos)
            } else {
                (b_pos, a_pos)
            };
            let rect = EntityExtent::Rect(*hw, *hh);
            (rect.surface_distance_from(rect_pos, circle_pos) - r).max(0.0)
        }
        (EntityExtent::Rect(half_w1, half_h1), EntityExtent::Rect(half_w2, half_h2)) => {
            let d = (a_pos - b_pos).abs();
            let dx = (d.x - half_w1 - half_w2).max(0.0);
            let dy = (d.y - half_h1 - half_h2).max(0.0);
            dx.hypot(dy)
        }
    }
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
        .register_type::<EntityExtent>()
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

#[cfg(test)]
mod tests {
    use super::*;

    // === surface_distance_from tests ===

    #[test]
    fn circle_surface_distance_outside() {
        let c = EntityExtent::Circle(10.0);
        let dist = c.surface_distance_from(Vec2::ZERO, Vec2::new(25.0, 0.0));
        assert!((dist - 15.0).abs() < 0.001);
    }

    #[test]
    fn circle_surface_distance_inside_returns_zero() {
        let c = EntityExtent::Circle(10.0);
        let dist = c.surface_distance_from(Vec2::ZERO, Vec2::new(5.0, 0.0));
        assert!(dist < 0.001);
    }

    #[test]
    fn circle_surface_distance_on_surface() {
        let c = EntityExtent::Circle(10.0);
        let dist = c.surface_distance_from(Vec2::ZERO, Vec2::new(10.0, 0.0));
        assert!(dist < 0.001);
    }

    #[test]
    fn rect_surface_distance_outside_axis() {
        let r = EntityExtent::Rect(64.0, 64.0);
        // Point at x=100 along X axis: dist = 100 - 64 = 36
        let dist = r.surface_distance_from(Vec2::ZERO, Vec2::new(100.0, 0.0));
        assert!((dist - 36.0).abs() < 0.001);
    }

    #[test]
    fn rect_surface_distance_outside_corner() {
        let r = EntityExtent::Rect(64.0, 64.0);
        // Point at (74, 74): dx = 10, dy = 10, dist = sqrt(200) ≈ 14.14
        let dist = r.surface_distance_from(Vec2::ZERO, Vec2::new(74.0, 74.0));
        assert!((dist - 200.0_f32.sqrt()).abs() < 0.001);
    }

    #[test]
    fn rect_surface_distance_inside_returns_zero() {
        let r = EntityExtent::Rect(64.0, 64.0);
        let dist = r.surface_distance_from(Vec2::ZERO, Vec2::new(30.0, 30.0));
        assert!(dist < 0.001);
    }

    // === extent_distance tests ===

    #[test]
    fn extent_distance_circle_circle_separated() {
        let a = EntityExtent::Circle(10.0);
        let b = EntityExtent::Circle(5.0);
        let dist = extent_distance(&a, Vec2::ZERO, &b, Vec2::new(25.0, 0.0));
        // center dist 25, radii 10 + 5 = 15, surface dist = 10
        assert!((dist - 10.0).abs() < 0.001);
    }

    #[test]
    fn extent_distance_circle_circle_overlapping() {
        let a = EntityExtent::Circle(10.0);
        let b = EntityExtent::Circle(10.0);
        let dist = extent_distance(&a, Vec2::ZERO, &b, Vec2::new(5.0, 0.0));
        assert!(dist < 0.001);
    }

    #[test]
    fn extent_distance_circle_rect_unit_to_fortress() {
        let unit = EntityExtent::Circle(6.0);
        let fortress = EntityExtent::Rect(64.0, 64.0);
        // Unit at x=100, fortress at origin. Rect surface at x=64.
        // Point-to-rect = 100 - 64 = 36. Circle radius = 6. Surface dist = 30.
        let dist = extent_distance(&unit, Vec2::new(100.0, 0.0), &fortress, Vec2::ZERO);
        assert!((dist - 30.0).abs() < 0.001);
    }

    #[test]
    fn extent_distance_rect_circle_commutative() {
        let unit = EntityExtent::Circle(6.0);
        let fortress = EntityExtent::Rect(64.0, 64.0);
        let d1 = extent_distance(&unit, Vec2::new(100.0, 0.0), &fortress, Vec2::ZERO);
        let d2 = extent_distance(&fortress, Vec2::ZERO, &unit, Vec2::new(100.0, 0.0));
        assert!((d1 - d2).abs() < 0.001);
    }

    #[test]
    fn extent_distance_circle_rect_unit_to_building() {
        let unit = EntityExtent::Circle(6.0);
        let building = EntityExtent::Rect(20.0, 20.0);
        // Unit at x=50, building at origin. Rect surface at x=20.
        // Point-to-rect = 50 - 20 = 30. Circle radius = 6. Surface dist = 24.
        let dist = extent_distance(&unit, Vec2::new(50.0, 0.0), &building, Vec2::ZERO);
        assert!((dist - 24.0).abs() < 0.001);
    }

    #[test]
    fn extent_distance_rect_rect_separated() {
        let a = EntityExtent::Rect(64.0, 64.0);
        let b = EntityExtent::Rect(20.0, 20.0);
        // A at origin, B at (200, 0). dx = 200 - 64 - 20 = 116. dy = 0.
        let dist = extent_distance(&a, Vec2::ZERO, &b, Vec2::new(200.0, 0.0));
        assert!((dist - 116.0).abs() < 0.001);
    }

    #[test]
    fn extent_distance_rect_rect_overlapping() {
        let a = EntityExtent::Rect(64.0, 64.0);
        let b = EntityExtent::Rect(20.0, 20.0);
        let dist = extent_distance(&a, Vec2::ZERO, &b, Vec2::new(50.0, 0.0));
        assert!(dist < 0.001);
    }

    // === Parity tests: extent_distance vs GJK surface_distance ===

    #[test]
    fn parity_circle_circle() {
        use crate::third_party::surface_distance;
        use avian2d::prelude::Collider;

        let c1 = EntityExtent::Circle(10.0);
        let c2 = EntityExtent::Circle(5.0);
        let gjk = surface_distance(
            &Collider::circle(10.0),
            Vec2::ZERO,
            &Collider::circle(5.0),
            Vec2::new(25.0, 0.0),
        );
        let ours = extent_distance(&c1, Vec2::ZERO, &c2, Vec2::new(25.0, 0.0));
        assert!(
            (gjk - ours).abs() < 0.01,
            "circle-circle: gjk={gjk}, ours={ours}"
        );
    }

    #[test]
    fn parity_circle_rect() {
        use crate::third_party::surface_distance;
        use avian2d::prelude::Collider;

        let unit_e = EntityExtent::Circle(6.0);
        let fort_e = EntityExtent::Rect(64.0, 64.0);
        let gjk = surface_distance(
            &Collider::circle(6.0),
            Vec2::new(100.0, 0.0),
            &Collider::rectangle(128.0, 128.0),
            Vec2::ZERO,
        );
        let ours = extent_distance(&unit_e, Vec2::new(100.0, 0.0), &fort_e, Vec2::ZERO);
        assert!(
            (gjk - ours).abs() < 0.01,
            "circle-rect: gjk={gjk}, ours={ours}"
        );
    }

    #[test]
    fn parity_rect_rect() {
        use crate::third_party::surface_distance;
        use avian2d::prelude::Collider;

        let a_e = EntityExtent::Rect(64.0, 64.0);
        let b_e = EntityExtent::Rect(20.0, 20.0);
        let gjk = surface_distance(
            &Collider::rectangle(128.0, 128.0),
            Vec2::ZERO,
            &Collider::rectangle(40.0, 40.0),
            Vec2::new(200.0, 0.0),
        );
        let ours = extent_distance(&a_e, Vec2::ZERO, &b_e, Vec2::new(200.0, 0.0));
        assert!(
            (gjk - ours).abs() < 0.01,
            "rect-rect: gjk={gjk}, ours={ours}"
        );
    }
}

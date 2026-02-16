//! Avian2d physics configuration for top-down gameplay.

use avian2d::collision::collider::contact_query;
use avian2d::prelude::*;
use bevy::prelude::*;

use crate::gameplay::battlefield::CELL_SIZE;

// === Collision Layers ===

/// Physics collision layers for the hitbox/hurtbox system.
///
/// - **Pushbox**: Physical presence — entities push/block each other.
/// - **Hitbox**: Attack collider (on projectiles, future melee swings).
/// - **Hurtbox**: Damageable surface (on units, buildings, fortresses).
#[derive(PhysicsLayer, Clone, Copy, Debug, Default)]
pub enum CollisionLayer {
    /// Physical body — blocks movement. All solid entities are pushboxes.
    #[default]
    Pushbox,
    /// Attack collider — lives on projectiles and (future) melee swings.
    Hitbox,
    /// Damageable surface — lives on units, buildings, fortresses.
    Hurtbox,
}

// === Helpers ===

/// Compute the minimum distance between two collider *surfaces*.
///
/// Uses avian2d's GJK-based `contact_query::distance()` under the hood.
/// Game systems call this instead of `contact_query` directly — if the
/// physics engine changes, only this wrapper changes.
///
/// Returns `f32::MAX` if the shape is unsupported (should never happen
/// with circles and rectangles).
#[must_use]
pub fn surface_distance(c1: &Collider, pos1: Vec2, c2: &Collider, pos2: Vec2) -> f32 {
    contact_query::distance(c1, pos1, 0.0, c2, pos2, 0.0).unwrap_or(f32::MAX)
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(PhysicsPlugins::default().with_length_unit(CELL_SIZE));
    app.insert_resource(Gravity::ZERO);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_distance_circle_circle() {
        let c1 = Collider::circle(10.0);
        let c2 = Collider::circle(5.0);
        let dist = surface_distance(&c1, Vec2::ZERO, &c2, Vec2::new(25.0, 0.0));
        // Center distance 25, radii 10 + 5 = 15 → surface distance 10
        assert!((dist - 10.0).abs() < 0.01);
    }

    #[test]
    fn surface_distance_circle_rectangle() {
        let circle = Collider::circle(12.0); // unit
        let rect = Collider::rectangle(128.0, 640.0); // fortress
        let dist = surface_distance(&circle, Vec2::new(100.0, 0.0), &rect, Vec2::ZERO);
        // Circle center at x=100, fortress half-width 64 → surface at x=64.
        // Distance from circle surface (100-12=88) to fortress surface (64) = 24.
        assert!((dist - 24.0).abs() < 0.01);
    }

    #[test]
    fn surface_distance_overlapping_returns_zero() {
        let c1 = Collider::circle(10.0);
        let c2 = Collider::circle(10.0);
        let dist = surface_distance(&c1, Vec2::ZERO, &c2, Vec2::new(5.0, 0.0));
        // Overlap: center distance 5 < sum of radii 20 → 0
        assert!(dist <= 0.01);
    }

    #[test]
    fn surface_distance_same_position() {
        let c1 = Collider::circle(10.0);
        let c2 = Collider::circle(10.0);
        let dist = surface_distance(&c1, Vec2::ZERO, &c2, Vec2::ZERO);
        assert!(dist <= 0.01);
    }

    #[test]
    fn surface_distance_circle_building_rect() {
        let circle = Collider::circle(12.0); // unit
        let rect = Collider::rectangle(60.0, 60.0); // building
        let dist = surface_distance(&circle, Vec2::new(72.0, 0.0), &rect, Vec2::ZERO);
        // Circle at x=72, building half-width 30 → building edge at x=30.
        // Circle surface at x=72-12=60. Distance = 60-30 = 30.
        assert!((dist - 30.0).abs() < 0.01);
    }
}

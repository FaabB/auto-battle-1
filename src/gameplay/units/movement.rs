//! Unit movement toward current target.

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{CombatStats, CurrentTarget, Movement, Unit};
use crate::third_party::surface_distance;

/// Sets unit `LinearVelocity` toward their `CurrentTarget`, stopping at attack range.
/// Uses surface-to-surface distance so units stop correctly for large targets (buildings, fortresses).
/// The physics engine handles actual position updates and collision resolution.
/// Runs in `GameSet::Movement`.
pub(super) fn unit_movement(
    mut units: Query<
        (
            &CurrentTarget,
            &Movement,
            &CombatStats,
            &GlobalTransform,
            &Collider,
            &mut LinearVelocity,
        ),
        With<Unit>,
    >,
    targets: Query<(&GlobalTransform, &Collider)>,
) {
    for (current_target, movement, stats, global_transform, unit_collider, mut velocity) in
        &mut units
    {
        let Some(target_entity) = current_target.0 else {
            velocity.0 = Vec2::ZERO;
            continue;
        };
        let Ok((target_pos, target_collider)) = targets.get(target_entity) else {
            velocity.0 = Vec2::ZERO;
            continue;
        };

        let current_xy = global_transform.translation().xy();
        let target_xy = target_pos.translation().xy();
        let distance = surface_distance(unit_collider, current_xy, target_collider, target_xy);

        // Already within attack range — stop
        if distance <= stats.range {
            velocity.0 = Vec2::ZERO;
            continue;
        }

        // Direction toward target (center-to-center for heading)
        let diff = target_xy - current_xy;
        let center_distance = diff.length();
        if center_distance < f32::EPSILON {
            velocity.0 = Vec2::ZERO;
            continue;
        }

        let direction = diff / center_distance;
        velocity.0 = direction * movement.speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::units::{UNIT_RADIUS, UnitType, unit_stats};

    fn create_movement_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, unit_movement);
        app.update(); // Initialize time
        app
    }

    fn spawn_unit_at(world: &mut World, x: f32, speed: f32, target: Option<Entity>) -> Entity {
        let stats = unit_stats(UnitType::Soldier);
        world
            .spawn((
                Unit,
                CurrentTarget(target),
                Movement { speed },
                CombatStats {
                    damage: stats.damage,
                    attack_speed: stats.attack_speed,
                    range: stats.attack_range,
                },
                Transform::from_xyz(x, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, 100.0, 0.0)),
                Collider::circle(UNIT_RADIUS),
                LinearVelocity::ZERO,
            ))
            .id()
    }

    fn spawn_target_at(world: &mut World, x: f32) -> Entity {
        world
            .spawn((
                Transform::from_xyz(x, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, 100.0, 0.0)),
                Collider::circle(5.0),
            ))
            .id()
    }

    #[test]
    fn unit_sets_velocity_toward_target() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        // Velocity should point right (positive x) toward target
        assert!(
            velocity.x > 0.0,
            "Velocity x should be positive toward target, got {}",
            velocity.x
        );
        // Magnitude should be approximately move_speed
        let speed = velocity.0.length();
        assert!(
            (speed - stats.move_speed).abs() < 0.1,
            "Velocity magnitude should be ~{}, got {}",
            stats.move_speed,
            speed
        );
    }

    #[test]
    fn unit_stops_at_attack_range() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        // Place unit within attack range
        let unit = spawn_unit_at(
            app.world_mut(),
            500.0 - stats.attack_range + 1.0,
            stats.move_speed,
            Some(target),
        );

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Unit within range should have zero velocity, got {:?}",
            velocity.0
        );
    }

    #[test]
    fn unit_zero_velocity_without_target() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, None);

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Unit with no target should have zero velocity, got {:?}",
            velocity.0
        );
    }

    #[test]
    fn unit_zero_velocity_when_target_despawned() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        // Despawn the target
        app.world_mut().despawn(target);
        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Unit with despawned target should have zero velocity, got {:?}",
            velocity.0
        );
    }

    #[test]
    fn unit_velocity_direction_is_normalized() {
        let mut app = create_movement_test_app();

        // Target at a diagonal — velocity direction should be normalized * speed
        let target = app
            .world_mut()
            .spawn((
                Transform::from_xyz(400.0, 200.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(400.0, 200.0, 0.0)),
                Collider::circle(5.0),
            ))
            .id();
        let unit = spawn_unit_at(app.world_mut(), 100.0, 50.0, Some(target));
        // Move unit to different Y to create diagonal
        let new_transform = Transform::from_xyz(100.0, 0.0, 0.0);
        *app.world_mut().get_mut::<Transform>(unit).unwrap() = new_transform;
        *app.world_mut().get_mut::<GlobalTransform>(unit).unwrap() =
            GlobalTransform::from(new_transform);

        app.update();

        let velocity = app.world().get::<LinearVelocity>(unit).unwrap();
        let speed = velocity.0.length();
        assert!(
            (speed - 50.0).abs() < 0.1,
            "Velocity magnitude should be 50.0, got {}",
            speed
        );
    }
}

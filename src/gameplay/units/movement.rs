//! Unit movement toward current target, following `NavPath` waypoints around obstacles.

use avian2d::prelude::Collider;
use bevy::prelude::*;

use super::avoidance::PreferredVelocity;
use super::pathfinding::NavPath;
use super::{CombatStats, CurrentTarget, Movement, Unit};
use crate::third_party::surface_distance;

/// Distance threshold for reaching a waypoint — when the unit's center
/// is within this distance of a waypoint, advance to the next one.
const WAYPOINT_REACHED_DISTANCE: f32 = 4.0;

/// Sets unit `PreferredVelocity` toward their current navmesh waypoint.
///
/// If the unit has a `NavPath` with remaining waypoints, steers toward
/// the next waypoint. When close enough, advances to the next waypoint.
/// When all waypoints are consumed (or no path exists), stops the unit
/// and waits for path recomputation — never steers directly at the target.
///
/// Always checks attack range against the actual target — if in range,
/// stops regardless of remaining waypoints.
///
/// The downstream `compute_avoidance` system reads `PreferredVelocity`
/// and writes the final `LinearVelocity`.
///
/// Runs in `GameSet::Movement`.
pub(super) fn unit_movement(
    mut units: Query<
        (
            &CurrentTarget,
            &Movement,
            &CombatStats,
            &GlobalTransform,
            &Collider,
            &mut PreferredVelocity,
            &mut NavPath,
        ),
        With<Unit>,
    >,
    targets: Query<(&GlobalTransform, &Collider)>,
) {
    for (
        current_target,
        movement,
        stats,
        global_transform,
        unit_collider,
        mut preferred,
        mut nav_path,
    ) in &mut units
    {
        let Some(target_entity) = current_target.0 else {
            preferred.0 = Vec2::ZERO;
            continue;
        };
        let Ok((target_pos, target_collider)) = targets.get(target_entity) else {
            preferred.0 = Vec2::ZERO;
            continue;
        };

        let current_xy = global_transform.translation().xy();
        let target_xy = target_pos.translation().xy();
        let distance_to_target =
            surface_distance(unit_collider, current_xy, target_collider, target_xy);

        // Already within attack range — stop
        if distance_to_target <= stats.range {
            preferred.0 = Vec2::ZERO;
            continue;
        }

        // Determine steering target from navmesh waypoints — never steer direct to target
        let Some(steer_toward) = nav_path.current_waypoint().and_then(|waypoint| {
            let dist_to_waypoint = current_xy.distance(waypoint);
            if dist_to_waypoint < WAYPOINT_REACHED_DISTANCE {
                if nav_path.advance() {
                    nav_path.current_waypoint()
                } else {
                    None // All waypoints consumed — stop, re-path next frame
                }
            } else {
                Some(waypoint)
            }
        }) else {
            // No waypoints available — stop and wait for path computation
            preferred.0 = Vec2::ZERO;
            continue;
        };

        // Compute velocity toward steering target
        let diff = steer_toward - current_xy;
        let dist = diff.length();
        if dist < f32::EPSILON {
            preferred.0 = Vec2::ZERO;
            continue;
        }

        let direction = diff / dist;
        preferred.0 = direction * movement.speed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::Team;
    use crate::gameplay::units::UnitType;
    use crate::gameplay::units::unit_stats;

    fn create_movement_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, unit_movement);
        app.update(); // Initialize time
        app
    }

    fn spawn_unit_at(world: &mut World, x: f32, speed: f32, target: Option<Entity>) -> Entity {
        let id = crate::testing::spawn_test_unit(world, Team::Player, x, 100.0);
        world
            .entity_mut(id)
            .insert((Movement { speed }, CurrentTarget(target)));
        id
    }

    fn spawn_target_at(world: &mut World, x: f32) -> Entity {
        crate::testing::spawn_test_target(world, Team::Player, x, 100.0)
    }

    #[test]
    fn unit_sets_velocity_toward_target() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        // Give the unit a path toward the target
        let mut nav_path = app.world_mut().get_mut::<NavPath>(unit).unwrap();
        nav_path.set(vec![Vec2::new(500.0, 100.0)], Some(target));

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        // Velocity should point right (positive x) toward target
        assert!(
            velocity.0.x > 0.0,
            "Velocity x should be positive toward target, got {}",
            velocity.0.x
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

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
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

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
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

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
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

        // Give the unit a path toward the target at a diagonal
        let mut nav_path = app.world_mut().get_mut::<NavPath>(unit).unwrap();
        nav_path.set(vec![Vec2::new(400.0, 200.0)], Some(target));

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        let speed = velocity.0.length();
        assert!(
            (speed - 50.0).abs() < 0.1,
            "Velocity magnitude should be 50.0, got {}",
            speed
        );
    }

    // === NavPath waypoint tests ===

    #[test]
    fn unit_follows_waypoint_instead_of_direct() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        // Set a path that goes up then right (around an obstacle)
        let mut nav_path = app.world_mut().get_mut::<NavPath>(unit).unwrap();
        nav_path.set(
            vec![
                Vec2::new(100.0, 300.0),
                Vec2::new(500.0, 300.0),
                Vec2::new(500.0, 100.0),
            ],
            Some(target),
        );

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        // Should head toward first waypoint (100, 300) = upward from (100, 100)
        assert!(
            velocity.0.y > 0.0,
            "Unit should move upward toward first waypoint, got vy={}",
            velocity.0.y
        );
        assert!(
            velocity.0.x.abs() < 0.1,
            "Unit should not move horizontally toward first waypoint, got vx={}",
            velocity.0.x
        );
    }

    #[test]
    fn unit_advances_to_next_waypoint() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        // Place unit very close to the first waypoint
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        // Set path with first waypoint very close to current position
        let mut nav_path = app.world_mut().get_mut::<NavPath>(unit).unwrap();
        nav_path.set(
            vec![Vec2::new(102.0, 100.0), Vec2::new(500.0, 100.0)],
            Some(target),
        );

        app.update();

        // Should have advanced past the first waypoint (within WAYPOINT_REACHED_DISTANCE)
        let nav_path = app.world().get::<NavPath>(unit).unwrap();
        assert!(
            nav_path.current_index >= 1,
            "Should have advanced past first waypoint, index={}",
            nav_path.current_index
        );
    }

    #[test]
    fn unit_stops_when_no_path() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));
        // NavPath is default (empty) — should stop, not steer direct

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Unit with no path should stop, got {:?}",
            velocity.0
        );
    }

    #[test]
    fn unit_stops_when_all_waypoints_consumed() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        // Target far away (not in attack range)
        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        // Set a single waypoint very close to the unit so it's consumed immediately
        let mut nav_path = app.world_mut().get_mut::<NavPath>(unit).unwrap();
        nav_path.set(vec![Vec2::new(101.0, 100.0)], Some(target));

        app.update();

        // Waypoint consumed, but not in attack range — unit should stop
        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Unit should stop when all waypoints consumed, got {:?}",
            velocity.0
        );
    }

    #[test]
    fn unit_stops_at_range_even_with_remaining_waypoints() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        // Place unit within attack range of target
        let unit = spawn_unit_at(
            app.world_mut(),
            500.0 - stats.attack_range + 1.0,
            stats.move_speed,
            Some(target),
        );

        // Give it a path with remaining waypoints
        let mut nav_path = app.world_mut().get_mut::<NavPath>(unit).unwrap();
        nav_path.set(
            vec![Vec2::new(600.0, 100.0), Vec2::new(700.0, 100.0)],
            Some(target),
        );

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Unit in attack range should stop even with waypoints, got {:?}",
            velocity.0
        );
    }
}

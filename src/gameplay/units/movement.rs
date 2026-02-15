//! Unit movement toward current target.

use bevy::prelude::*;

use super::{CombatStats, CurrentTarget, Movement, Unit};

/// Moves units toward their `CurrentTarget`, stopping at attack range.
/// Runs in `GameSet::Movement`.
pub(super) fn unit_movement(
    time: Res<Time>,
    mut units: Query<(&CurrentTarget, &Movement, &CombatStats, &mut Transform), With<Unit>>,
    positions: Query<&GlobalTransform>,
) {
    for (current_target, movement, stats, mut transform) in &mut units {
        let Some(target_entity) = current_target.0 else {
            continue;
        };
        let Ok(target_pos) = positions.get(target_entity) else {
            continue;
        };

        let target_xy = target_pos.translation().xy();
        let current_xy = transform.translation.xy();
        let diff = target_xy - current_xy;
        let distance = diff.length();

        // Already within attack range — stop
        if distance <= stats.range {
            continue;
        }

        let direction = diff / distance; // normalized
        let move_amount = movement.speed * time.delta_secs();
        let max_move = distance - stats.range;

        if move_amount >= max_move {
            // Would overshoot — snap to attack range distance
            transform.translation.x = direction.x.mul_add(-stats.range, target_xy.x);
            transform.translation.y = direction.y.mul_add(-stats.range, target_xy.y);
        } else {
            transform.translation.x = direction.x.mul_add(move_amount, transform.translation.x);
            transform.translation.y = direction.y.mul_add(move_amount, transform.translation.y);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::gameplay::units::{UnitType, unit_stats};

    fn create_movement_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, unit_movement);
        // Run one update to initialize time (first frame has delta=0)
        app.update();
        app
    }

    /// Advance virtual time by the given duration and run one update.
    fn advance_and_update(app: &mut App, dt: Duration) {
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(dt);
        app.update();
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
            ))
            .id()
    }

    fn spawn_target_at(world: &mut World, x: f32) -> Entity {
        world
            .spawn((
                Transform::from_xyz(x, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, 100.0, 0.0)),
            ))
            .id()
    }

    #[test]
    fn unit_moves_toward_target() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, Some(target));

        advance_and_update(&mut app, Duration::from_millis(100));

        let transform = app.world().get::<Transform>(unit).unwrap();
        assert!(
            transform.translation.x > 100.0,
            "Unit should have moved right toward target, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn unit_stops_at_attack_range() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        // Place unit within attack range of target
        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(
            app.world_mut(),
            500.0 - stats.attack_range + 1.0,
            stats.move_speed,
            Some(target),
        );

        advance_and_update(&mut app, Duration::from_millis(100));

        let transform = app.world().get::<Transform>(unit).unwrap();
        // Should not have moved — already within range
        assert!(
            (transform.translation.x - (500.0 - stats.attack_range + 1.0)).abs() < f32::EPSILON,
            "Unit should not move when within attack range, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn unit_no_movement_without_target() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let unit = spawn_unit_at(app.world_mut(), 100.0, stats.move_speed, None);

        advance_and_update(&mut app, Duration::from_millis(100));

        let transform = app.world().get::<Transform>(unit).unwrap();
        assert!(
            (transform.translation.x - 100.0).abs() < f32::EPSILON,
            "Unit with no target should not move, x={}",
            transform.translation.x
        );
    }

    #[test]
    fn unit_snaps_to_range_on_overshoot() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        // Place unit very close to target (just outside range)
        // Distance = 31.0 - 30.0 range = 1.0px to travel
        // Use very high speed (100_000) so even microsecond delta causes overshoot
        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(
            app.world_mut(),
            500.0 - stats.attack_range - 1.0,
            100_000.0,
            Some(target),
        );

        advance_and_update(&mut app, Duration::from_millis(100));

        let transform = app.world().get::<Transform>(unit).unwrap();
        // Should snap to exactly attack range from target
        let distance_to_target = (500.0 - transform.translation.x).abs();
        assert!(
            (distance_to_target - stats.attack_range).abs() < 0.01,
            "Unit should snap to attack range distance, actual distance={}",
            distance_to_target
        );
    }
}

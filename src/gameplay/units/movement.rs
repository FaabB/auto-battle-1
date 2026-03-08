//! Unit movement using flow field directions.

use bevy::prelude::*;

use super::avoidance::PreferredVelocity;
use super::{CombatStats, Movement, TargetingState, Unit};
use crate::gameplay::flow_field::{AssignedGoal, GoalRegistry};
use crate::gameplay::{EntityExtent, extent_distance};

/// Sets unit `PreferredVelocity` based on targeting state and flow field.
///
/// - `Moving`/`Seeking`: follow flow field toward assigned goal.
/// - `Engaging`: steer directly toward target; stop if in attack range.
/// - `Attacking`: zero velocity.
///
/// The downstream `compute_avoidance` system reads `PreferredVelocity`
/// and writes the final `LinearVelocity`.
///
/// Runs in `GameSet::Movement`.
pub(super) fn unit_movement(
    mut units: Query<
        (
            &TargetingState,
            &Movement,
            &CombatStats,
            &GlobalTransform,
            &EntityExtent,
            &AssignedGoal,
            &mut PreferredVelocity,
        ),
        With<Unit>,
    >,
    targets: Query<(&GlobalTransform, &EntityExtent)>,
    registry: Option<Res<GoalRegistry>>,
) {
    let Some(registry) = registry else { return };

    for (targeting_state, movement, stats, global_transform, unit_extent, goal, mut preferred) in
        &mut units
    {
        let current_xy = global_transform.translation().xy();

        match *targeting_state {
            TargetingState::Moving | TargetingState::Seeking => {
                // Follow flow field
                let flow_field = match goal {
                    AssignedGoal::EnemyFortress => &registry.enemy_fortress.flow_field,
                    AssignedGoal::PlayerFortress => &registry.player_fortress.flow_field,
                };
                let direction = flow_field.direction_at(current_xy);
                preferred.0 = direction * movement.speed;
            }
            TargetingState::Engaging(target_entity) => {
                // Steer directly toward target
                let Ok((target_pos, target_extent)) = targets.get(target_entity) else {
                    preferred.0 = Vec2::ZERO;
                    continue;
                };
                let target_xy = target_pos.translation().xy();
                let distance = extent_distance(unit_extent, current_xy, target_extent, target_xy);

                if distance <= stats.range {
                    preferred.0 = Vec2::ZERO;
                    continue;
                }

                let diff = target_xy - current_xy;
                let dist = diff.length();
                if dist < f32::EPSILON {
                    preferred.0 = Vec2::ZERO;
                } else {
                    preferred.0 = (diff / dist) * movement.speed;
                }
            }
            TargetingState::Attacking(_) => {
                preferred.0 = Vec2::ZERO;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::Team;
    use crate::gameplay::flow_field::{
        CostGrid, DijkstraFlowField, FLOW_COLS, FLOW_ROWS, FlowFieldAlgorithm, GoalFlowField,
    };
    use crate::gameplay::units::UnitType;
    use crate::gameplay::units::unit_stats;
    use avian2d::prelude::Collider;

    /// Create a GoalRegistry with open-grid flow fields.
    /// Player fortress goal at (0, 20), enemy fortress goal at (327, 20).
    fn test_registry() -> GoalRegistry {
        let cost_grid = CostGrid::new(FLOW_COLS, FLOW_ROWS);
        let algo = Box::new(DijkstraFlowField);
        let pf_cells = vec![(0, 20)];
        let ef_cells = vec![(FLOW_COLS - 1, 20)];
        let player_ff = algo.compute(&cost_grid, &pf_cells);
        let enemy_ff = algo.compute(&cost_grid, &ef_cells);

        GoalRegistry {
            player_fortress: GoalFlowField {
                flow_field: player_ff,
                goal_cells: pf_cells,
            },
            enemy_fortress: GoalFlowField {
                flow_field: enemy_ff,
                goal_cells: ef_cells,
            },
            cost_grid,
            algorithm: algo,
        }
    }

    fn create_movement_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(test_registry());
        app.add_systems(Update, unit_movement);
        app.update(); // Initialize time
        app
    }

    fn spawn_unit_at(
        world: &mut World,
        x: f32,
        speed: f32,
        targeting_state: TargetingState,
    ) -> Entity {
        let id = crate::testing::spawn_test_unit(world, Team::Player, x, 320.0);
        world
            .entity_mut(id)
            .insert((Movement { speed }, targeting_state));
        id
    }

    fn spawn_target_at(world: &mut World, x: f32) -> Entity {
        crate::testing::spawn_test_target(world, Team::Player, x, 320.0)
    }

    #[test]
    fn seeking_unit_gets_flow_field_velocity() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        // Player unit at x=100 with Seeking state → should follow enemy fortress flow field (rightward)
        let unit = spawn_unit_at(
            app.world_mut(),
            100.0,
            stats.move_speed,
            TargetingState::Seeking,
        );

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        // Flow field for enemy fortress goal at right edge → velocity should point right
        assert!(
            velocity.0.x > 0.0,
            "Seeking unit should move toward enemy fortress (rightward), got {:?}",
            velocity.0
        );
        let speed = velocity.0.length();
        assert!(
            (speed - stats.move_speed).abs() < 1.0,
            "Velocity magnitude should be ~{}, got {}",
            stats.move_speed,
            speed
        );
    }

    #[test]
    fn moving_unit_gets_flow_field_velocity() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let unit = spawn_unit_at(
            app.world_mut(),
            100.0,
            stats.move_speed,
            TargetingState::Moving,
        );

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        assert!(
            velocity.0.x > 0.0,
            "Moving unit should move toward enemy fortress (rightward), got {:?}",
            velocity.0
        );
    }

    #[test]
    fn engaging_unit_steers_toward_target() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(
            app.world_mut(),
            100.0,
            stats.move_speed,
            TargetingState::Engaging(target),
        );

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        assert!(
            velocity.0.x > 0.0,
            "Engaging unit should steer toward target, got {:?}",
            velocity.0
        );
        let speed = velocity.0.length();
        assert!(
            (speed - stats.move_speed).abs() < 0.1,
            "Velocity magnitude should be ~{}, got {}",
            stats.move_speed,
            speed
        );
    }

    #[test]
    fn engaging_unit_stops_at_attack_range() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        // Place unit within attack range
        let unit = spawn_unit_at(
            app.world_mut(),
            500.0 - stats.attack_range + 1.0,
            stats.move_speed,
            TargetingState::Engaging(target),
        );

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Engaging unit within range should stop, got {:?}",
            velocity.0
        );
    }

    #[test]
    fn attacking_unit_gets_zero_velocity() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(
            app.world_mut(),
            100.0,
            stats.move_speed,
            TargetingState::Attacking(target),
        );

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        assert!(
            velocity.0.length() < f32::EPSILON,
            "Attacking unit should have zero velocity, got {:?}",
            velocity.0
        );
    }

    #[test]
    fn engaging_unit_zero_velocity_when_target_despawned() {
        let mut app = create_movement_test_app();
        let stats = unit_stats(UnitType::Soldier);

        let target = spawn_target_at(app.world_mut(), 500.0);
        let unit = spawn_unit_at(
            app.world_mut(),
            100.0,
            stats.move_speed,
            TargetingState::Engaging(target),
        );

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
    fn engaging_unit_velocity_direction_is_normalized() {
        let mut app = create_movement_test_app();

        // Target at a diagonal
        let target = app
            .world_mut()
            .spawn((
                Transform::from_xyz(400.0, 500.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(400.0, 500.0, 0.0)),
                EntityExtent::Circle(5.0),
                Collider::circle(5.0),
            ))
            .id();
        let unit = spawn_unit_at(
            app.world_mut(),
            100.0,
            50.0,
            TargetingState::Engaging(target),
        );

        app.update();

        let velocity = app.world().get::<PreferredVelocity>(unit).unwrap();
        let speed = velocity.0.length();
        assert!(
            (speed - 50.0).abs() < 0.1,
            "Velocity magnitude should be 50.0, got {}",
            speed
        );
    }
}

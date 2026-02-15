//! Unit AI: target selection.

use bevy::prelude::*;

use crate::gameplay::{Target, Team};

use super::{BACKTRACK_DISTANCE, CurrentTarget, Unit};

/// How many frames between full re-evaluations for units that already have a valid target.
/// Units with no target (or a despawned target) always evaluate immediately.
const RETARGET_INTERVAL_FRAMES: u32 = 10;

/// Finds the nearest valid target for each unit. Runs in `GameSet::Ai`.
///
/// - Units without a target evaluate every frame (so newly spawned units react instantly).
/// - Units with a valid target only re-evaluate every [`RETARGET_INTERVAL_FRAMES`] frames.
/// - Respects the backtrack limit — ignores enemies too far behind.
pub(super) fn unit_find_target(
    mut counter: Local<u32>,
    mut units: Query<(Entity, &Team, &GlobalTransform, &mut CurrentTarget), With<Unit>>,
    all_targets: Query<(Entity, &Team, &GlobalTransform), With<Target>>,
) {
    *counter = counter.wrapping_add(1);

    for (entity, team, transform, mut current_target) in &mut units {
        let has_valid_target = current_target.0.is_some_and(|e| all_targets.get(e).is_ok());

        // Stagger: each unit retargets on a different frame based on its entity index
        let should_retarget =
            (entity.index().index().wrapping_add(*counter)) % RETARGET_INTERVAL_FRAMES == 0;
        if has_valid_target && !should_retarget {
            continue;
        }

        let my_pos = transform.translation().xy();
        let opposing_team = match team {
            Team::Player => Team::Enemy,
            Team::Enemy => Team::Player,
        };

        // Find nearest enemy target within backtrack limit
        let mut nearest: Option<(Entity, f32)> = None;
        for (candidate, candidate_team, candidate_pos) in &all_targets {
            if candidate == entity || *candidate_team != opposing_team {
                continue;
            }
            let candidate_xy = candidate_pos.translation().xy();

            // Backtrack filter: ignore targets too far behind
            let behind = match team {
                Team::Player => my_pos.x - candidate_xy.x,
                Team::Enemy => candidate_xy.x - my_pos.x,
            };
            if behind > BACKTRACK_DISTANCE {
                continue;
            }

            let dist = my_pos.distance(candidate_xy);
            if nearest.is_none_or(|(_, d)| dist < d) {
                nearest = Some((candidate, dist));
            }
        }

        current_target.0 = nearest.map(|(e, _)| e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spawn a unit entity with Transform + GlobalTransform at the given position.
    fn spawn_unit(world: &mut World, team: Team, x: f32, y: f32) -> Entity {
        world
            .spawn((
                Unit,
                team,
                Target,
                CurrentTarget(None),
                Transform::from_xyz(x, y, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
            ))
            .id()
    }

    /// Spawn a targetable entity (non-unit) at the given position.
    fn spawn_target(world: &mut World, team: Team, x: f32, y: f32) -> Entity {
        world
            .spawn((
                team,
                Target,
                Transform::from_xyz(x, y, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
            ))
            .id()
    }

    fn create_ai_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, unit_find_target);
        app
    }

    #[test]
    fn unit_targets_nearest_enemy() {
        let mut app = create_ai_test_app();

        let player = spawn_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let _far_enemy = spawn_unit(app.world_mut(), Team::Enemy, 500.0, 100.0);
        let near_enemy = spawn_unit(app.world_mut(), Team::Enemy, 200.0, 100.0);

        app.update();

        let current_target = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(current_target.0, Some(near_enemy));
    }

    #[test]
    fn unit_targets_fortress_when_no_enemies() {
        let mut app = create_ai_test_app();

        let player = spawn_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let fortress = spawn_target(app.world_mut(), Team::Enemy, 5000.0, 320.0);

        app.update();

        let current_target = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(current_target.0, Some(fortress));
    }

    #[test]
    fn unit_retargets_when_target_despawned() {
        let mut app = create_ai_test_app();

        let player = spawn_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let enemy1 = spawn_unit(app.world_mut(), Team::Enemy, 300.0, 100.0);
        let enemy2 = spawn_unit(app.world_mut(), Team::Enemy, 500.0, 100.0);

        // First update: targets nearest (enemy1)
        app.update();
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, Some(enemy1));

        // Despawn enemy1
        app.world_mut().despawn(enemy1);

        // Next update: target is invalid, re-evaluates immediately → enemy2
        app.update();
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, Some(enemy2));
    }

    #[test]
    fn unit_switches_to_closer_target_on_retarget_frame() {
        let mut app = create_ai_test_app();

        let player = spawn_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let enemy_far = spawn_unit(app.world_mut(), Team::Enemy, 300.0, 100.0);

        // First update gives a target (counter=1, no target yet → evaluates)
        app.update();
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, Some(enemy_far));

        // Spawn a closer enemy
        let enemy_near = spawn_unit(app.world_mut(), Team::Enemy, 150.0, 100.0);

        // Run enough updates to trigger a retarget frame (counter resets at 10)
        for _ in 0..RETARGET_INTERVAL_FRAMES {
            app.update();
        }

        // Should have switched to the closer enemy
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, Some(enemy_near));
    }

    #[test]
    fn unit_respects_backtrack_limit() {
        let mut app = create_ai_test_app();

        // Player unit at x=500, enemy far behind at x=100 (400px behind > 128px limit)
        let player = spawn_unit(app.world_mut(), Team::Player, 500.0, 100.0);
        let _behind_enemy = spawn_unit(app.world_mut(), Team::Enemy, 100.0, 100.0);

        app.update();

        // Should NOT target the enemy behind (too far)
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, None);
    }

    #[test]
    fn unit_targets_building() {
        let mut app = create_ai_test_app();

        // Enemy unit should target a player building
        let enemy = spawn_unit(app.world_mut(), Team::Enemy, 500.0, 100.0);
        let building = spawn_target(app.world_mut(), Team::Player, 300.0, 100.0);

        app.update();

        let ct = app.world().get::<CurrentTarget>(enemy).unwrap();
        assert_eq!(ct.0, Some(building));
    }
}

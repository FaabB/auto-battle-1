//! AI: target selection for all combat entities (units, fortresses, turrets).

use avian2d::prelude::*;
use bevy::prelude::*;

use super::{CurrentTarget, Movement, Target, Team};
use crate::screens::GameState;
use crate::third_party::surface_distance;
use crate::{GameSet, gameplay_running};

/// Maximum distance (pixels) a mobile entity will backtrack to chase a target behind it.
/// 2 cells = 128 pixels.
const BACKTRACK_DISTANCE: f32 = 2.0 * super::battlefield::CELL_SIZE;

/// Number of stagger slots. Entities are distributed across slots by their index.
/// Each timer tick evaluates one slot's worth of entities, spreading the load.
/// Full retarget cycle = `RETARGET_SLOT_INTERVAL_SECS * RETARGET_SLOTS` = 0.15s.
const RETARGET_SLOTS: u32 = 10;

/// Seconds between slot ticks (0.15s full cycle / 10 slots = 0.015s per slot).
/// Entities without a target (or with a despawned target) always evaluate immediately.
const RETARGET_SLOT_INTERVAL_SECS: f32 = 0.015;

/// Timer and slot state for staggered retargeting.
/// Entities re-evaluate targets in round-robin fashion: slot 0 first, then slot 1, etc.
/// The timer fires every `RETARGET_INTERVAL_SECS / RETARGET_SLOTS` seconds.
/// Exposed as a resource so tests can manipulate slot and timer state.
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct RetargetTimer {
    pub timer: Timer,
    pub current_slot: u32,
}

impl Default for RetargetTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(RETARGET_SLOT_INTERVAL_SECS, TimerMode::Repeating),
            current_slot: 0,
        }
    }
}

/// Finds the nearest valid target for each entity with `CurrentTarget`. Runs in `GameSet::Ai`.
///
/// Works for both units (with `Movement`) and static entities like fortresses (no `Movement`).
/// - Entities without a target evaluate every frame (so newly spawned units react instantly).
/// - Entities with a valid target re-evaluate on their stagger slot (once per
///   [`RETARGET_INTERVAL_SECS`] cycle, spread across [`RETARGET_SLOTS`] time intervals).
/// - Backtrack limit only applies to mobile entities (those with `Movement`).
pub fn find_target(
    time: Res<Time>,
    mut retarget_timer: ResMut<RetargetTimer>,
    mut seekers: Query<(
        Entity,
        &Team,
        &GlobalTransform,
        &Collider,
        &mut CurrentTarget,
        Option<&Movement>,
    )>,
    all_targets: Query<(Entity, &Team, &GlobalTransform, &Collider), With<Target>>,
) {
    retarget_timer.timer.tick(time.delta());
    let slot_advanced = retarget_timer.timer.just_finished();
    if slot_advanced {
        retarget_timer.current_slot = (retarget_timer.current_slot + 1) % RETARGET_SLOTS;
    }

    for (entity, team, transform, seeker_collider, mut current_target, movement) in &mut seekers {
        let has_valid_target = current_target.0.is_some_and(|e| all_targets.get(e).is_ok());

        if has_valid_target {
            if !slot_advanced {
                continue;
            }
            let entity_slot = entity.index().index() % RETARGET_SLOTS;
            if entity_slot != retarget_timer.current_slot {
                continue;
            }
        }

        let my_pos = transform.translation().xy();
        let opposing_team = team.opposing();

        // Find nearest enemy target (backtrack filter only for mobile entities)
        let mut nearest: Option<(Entity, f32)> = None;
        for (candidate, candidate_team, candidate_pos, candidate_collider) in &all_targets {
            if candidate == entity || *candidate_team != opposing_team {
                continue;
            }
            let candidate_xy = candidate_pos.translation().xy();

            // Backtrack filter: only applies to moving entities (units)
            if movement.is_some() {
                let behind = match team {
                    Team::Player => my_pos.x - candidate_xy.x,
                    Team::Enemy => candidate_xy.x - my_pos.x,
                };
                if behind > BACKTRACK_DISTANCE {
                    continue;
                }
            }

            let dist = surface_distance(seeker_collider, my_pos, candidate_collider, candidate_xy);
            if nearest.is_none_or(|(_, d)| dist < d) {
                nearest = Some((candidate, dist));
            }
        }

        current_target.0 = nearest.map(|(e, _)| e);
    }
}

// === Plugin ===

fn reset_retarget_timer(mut commands: Commands) {
    commands.insert_resource(RetargetTimer::default());
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<RetargetTimer>();
    app.register_type::<RetargetTimer>();
    app.add_systems(OnEnter(GameState::InGame), reset_retarget_timer);
    app.add_systems(
        Update,
        find_target.in_set(GameSet::Ai).run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn create_ai_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<RetargetTimer>();
        app.add_systems(Update, find_target);
        app
    }

    /// Set the retarget timer so the NEXT `app.update()` will fire the slot
    /// that `entity` belongs to. Sets `current_slot` to entity's slot - 1
    /// and nearly expires the timer so the next tick advances into the entity's slot.
    fn set_retarget_for_entity(app: &mut App, entity: Entity) {
        let entity_slot = entity.index().index() % RETARGET_SLOTS;
        let prev_slot = if entity_slot == 0 {
            RETARGET_SLOTS - 1
        } else {
            entity_slot - 1
        };
        let mut timer = app.world_mut().resource_mut::<RetargetTimer>();
        timer.current_slot = prev_slot;
        crate::testing::nearly_expire_timer(&mut timer.timer);
    }

    #[test]
    fn unit_targets_nearest_enemy() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let _far_enemy =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 500.0, 100.0);
        let near_enemy =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 200.0, 100.0);

        app.update();

        let current_target = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(current_target.0, Some(near_enemy));
    }

    #[test]
    fn unit_targets_fortress_when_no_enemies() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let fortress =
            crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 5000.0, 320.0);

        app.update();

        let current_target = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(current_target.0, Some(fortress));
    }

    #[test]
    fn unit_retargets_when_target_despawned() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let enemy1 = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 300.0, 100.0);
        let enemy2 = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 500.0, 100.0);

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
    fn unit_switches_to_closer_target_on_retarget() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let enemy_far = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 300.0, 100.0);

        // First update gives a target (no target yet → evaluates immediately)
        app.update();
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, Some(enemy_far));

        // Spawn a closer enemy
        let enemy_near =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 150.0, 100.0);

        // Set timer to fire on the player's slot next update
        set_retarget_for_entity(&mut app, player);

        app.update();

        // Should have switched to the closer enemy
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, Some(enemy_near));
    }

    #[test]
    fn unit_respects_backtrack_limit() {
        let mut app = create_ai_test_app();

        // Player unit at x=500, enemy far behind at x=100 (400px behind > 128px limit)
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 500.0, 100.0);
        let _behind_enemy =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 100.0, 100.0);

        app.update();

        // Should NOT target the enemy behind (too far)
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, None);
    }

    #[test]
    fn unit_targets_building() {
        let mut app = create_ai_test_app();

        // Enemy unit should target a player building
        let enemy = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 500.0, 100.0);
        let building =
            crate::testing::spawn_test_target(app.world_mut(), Team::Player, 300.0, 100.0);

        app.update();

        let ct = app.world().get::<CurrentTarget>(enemy).unwrap();
        assert_eq!(ct.0, Some(building));
    }

    #[test]
    fn fortress_targets_nearest_enemy() {
        let mut app = create_ai_test_app();

        // Spawn a fortress-like entity (no Unit, no Movement — static)
        let fortress = app
            .world_mut()
            .spawn((
                Team::Player,
                Target,
                CurrentTarget(None),
                Transform::from_xyz(64.0, 320.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(64.0, 320.0, 0.0)),
                Collider::rectangle(128.0, 128.0),
            ))
            .id();

        // Spawn two enemy targets
        let near_enemy =
            crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 200.0, 320.0);
        let _far_enemy =
            crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 500.0, 320.0);

        app.update();

        let ct = app.world().get::<CurrentTarget>(fortress).unwrap();
        assert_eq!(ct.0, Some(near_enemy));
    }

    #[test]
    fn static_entity_has_no_backtrack_limit() {
        let mut app = create_ai_test_app();

        // Fortress at x=500 with enemy "behind" at x=100 (would be filtered for units)
        let fortress = app
            .world_mut()
            .spawn((
                Team::Player,
                Target,
                CurrentTarget(None),
                Transform::from_xyz(500.0, 320.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(500.0, 320.0, 0.0)),
                Collider::rectangle(128.0, 128.0),
            ))
            .id();

        let behind_enemy =
            crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 100.0, 320.0);

        app.update();

        // Static entity (no Movement) should target regardless of direction
        let ct = app.world().get::<CurrentTarget>(fortress).unwrap();
        assert_eq!(ct.0, Some(behind_enemy));
    }
}

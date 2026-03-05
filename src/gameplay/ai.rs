//! AI: target selection for all combat entities (units, fortresses, turrets).

use avian2d::prelude::*;
use bevy::prelude::*;

use super::battlefield::CELL_SIZE;
use super::spatial_hash::SpatialHash;
use super::{CurrentTarget, Movement, Target, Team};
use crate::screens::GameState;
use crate::third_party::surface_distance;
use crate::{GameSet, gameplay_running};

/// Maximum distance (pixels) a mobile entity will backtrack to chase a target behind it.
/// 2 cells = 128 pixels.
const BACKTRACK_DISTANCE: f32 = 2.0 * super::battlefield::CELL_SIZE;

/// Initial search radius for nearby targets. 8 cells = 512px.
/// Covers most practical targeting scenarios (units near enemies).
const INITIAL_SEARCH_RADIUS: f32 = 8.0 * CELL_SIZE;

/// Maximum half-extent of any entity collider (fortress = 128px, half = 64px).
/// Entities whose center is just outside the search radius may still have
/// their surface within range, so we pad the query by this amount.
const MAX_ENTITY_HALF_EXTENT: f32 = 64.0;

/// Diagonal of the full battlefield — used as fallback search radius.
/// Guarantees finding all targets regardless of position.
const BATTLEFIELD_DIAGONAL: f32 = 5300.0; // > sqrt(5248^2 + 640^2) ≈ 5287

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

/// Spatial hash for target lookups. Populated with all `With<Target>` entities
/// each frame. Queried by `find_target` to find nearby candidates.
#[derive(Resource, Debug)]
pub struct TargetSpatialHash(SpatialHash);

impl std::ops::Deref for TargetSpatialHash {
    type Target = SpatialHash;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for TargetSpatialHash {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Rebuild the target spatial hash with all targetable entities.
/// Runs every frame before `find_target`.
fn rebuild_target_grid(
    mut grid: ResMut<TargetSpatialHash>,
    targets: Query<(Entity, &GlobalTransform), With<Target>>,
) {
    grid.clear();
    for (entity, transform) in &targets {
        grid.insert(entity, transform.translation().xy());
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
    grid: Res<TargetSpatialHash>,
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

        // Two-pass spatial search: nearby first, full battlefield fallback
        let nearest = find_nearest_target(
            &grid,
            entity,
            my_pos,
            seeker_collider,
            opposing_team,
            movement.is_some(),
            *team,
            &all_targets,
        );

        current_target.0 = nearest;
    }
}

/// Search the spatial grid for the nearest valid target.
///
/// Two-pass strategy:
/// 1. Search within `INITIAL_SEARCH_RADIUS` (catches most cases)
/// 2. If nothing found, search the full battlefield
///
/// Within each pass, uses center-distance as a cheap pre-filter before
/// calling `surface_distance` (GJK) on close candidates.
#[allow(clippy::too_many_arguments)]
fn find_nearest_target(
    grid: &TargetSpatialHash,
    seeker_entity: Entity,
    seeker_pos: Vec2,
    seeker_collider: &Collider,
    opposing_team: Team,
    is_mobile: bool,
    seeker_team: Team,
    all_targets: &Query<(Entity, &Team, &GlobalTransform, &Collider), With<Target>>,
) -> Option<Entity> {
    // First pass: nearby targets
    let result = search_radius(
        grid,
        INITIAL_SEARCH_RADIUS + MAX_ENTITY_HALF_EXTENT,
        seeker_entity,
        seeker_pos,
        seeker_collider,
        opposing_team,
        is_mobile,
        seeker_team,
        all_targets,
    );

    if result.is_some() {
        return result;
    }

    // Fallback: full battlefield
    search_radius(
        grid,
        BATTLEFIELD_DIAGONAL,
        seeker_entity,
        seeker_pos,
        seeker_collider,
        opposing_team,
        is_mobile,
        seeker_team,
        all_targets,
    )
}

#[allow(clippy::too_many_arguments)]
fn search_radius(
    grid: &TargetSpatialHash,
    radius: f32,
    seeker_entity: Entity,
    seeker_pos: Vec2,
    seeker_collider: &Collider,
    opposing_team: Team,
    is_mobile: bool,
    seeker_team: Team,
    all_targets: &Query<(Entity, &Team, &GlobalTransform, &Collider), With<Target>>,
) -> Option<Entity> {
    let candidates = grid.query_neighbors(seeker_pos, radius);

    // Phase 1: Filter and compute center distances (cheap)
    let mut valid_candidates: Vec<(Entity, Vec2, &Collider, f32)> = Vec::new();
    for candidate_entity in candidates {
        let Ok((cand_entity, cand_team, cand_transform, cand_collider)) =
            all_targets.get(candidate_entity)
        else {
            continue;
        };

        if cand_entity == seeker_entity || *cand_team != opposing_team {
            continue;
        }

        let cand_pos = cand_transform.translation().xy();

        // Backtrack filter (mobile entities only)
        if is_mobile {
            let behind = match seeker_team {
                Team::Player => seeker_pos.x - cand_pos.x,
                Team::Enemy => cand_pos.x - seeker_pos.x,
            };
            if behind > BACKTRACK_DISTANCE {
                continue;
            }
        }

        let center_dist = seeker_pos.distance(cand_pos);
        valid_candidates.push((cand_entity, cand_pos, cand_collider, center_dist));
    }

    if valid_candidates.is_empty() {
        return None;
    }

    // Phase 2: Find nearest by surface distance
    // Use center-distance to skip GJK for obviously-distant candidates.
    let min_center_dist = valid_candidates
        .iter()
        .map(|(_, _, _, d)| *d)
        .fold(f32::MAX, f32::min);

    // Only compute surface_distance for candidates whose center is close
    // enough that they could beat the current best surface distance.
    // Cutoff: min_center_dist + 2 * MAX_ENTITY_HALF_EXTENT covers the
    // worst case where both entities have maximum collider extent.
    let center_cutoff = 2.0f32.mul_add(MAX_ENTITY_HALF_EXTENT, min_center_dist);

    let mut nearest: Option<(Entity, f32)> = None;
    for (cand_entity, cand_pos, cand_collider, center_dist) in &valid_candidates {
        if *center_dist > center_cutoff {
            if let Some((_, best_surf)) = nearest {
                // Tighten cutoff as we find better candidates
                if *center_dist > 2.0f32.mul_add(MAX_ENTITY_HALF_EXTENT, best_surf) {
                    continue;
                }
            } else {
                continue;
            }
        }

        let surf_dist = surface_distance(seeker_collider, seeker_pos, cand_collider, *cand_pos);
        if nearest.is_none_or(|(_, d)| surf_dist < d) {
            nearest = Some((*cand_entity, surf_dist));
        }
    }

    nearest.map(|(e, _)| e)
}

// === Plugin ===

fn reset_retarget_timer(mut commands: Commands) {
    commands.insert_resource(RetargetTimer::default());
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<RetargetTimer>();
    app.insert_resource(TargetSpatialHash(SpatialHash::new(CELL_SIZE)));
    app.register_type::<RetargetTimer>();
    app.add_systems(OnEnter(GameState::InGame), reset_retarget_timer);
    app.add_systems(
        Update,
        (rebuild_target_grid, find_target)
            .chain_ignore_deferred()
            .in_set(GameSet::Ai)
            .run_if(gameplay_running),
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
        app.insert_resource(TargetSpatialHash(SpatialHash::new(
            crate::gameplay::battlefield::CELL_SIZE,
        )));
        app.add_systems(
            Update,
            (rebuild_target_grid, find_target).chain_ignore_deferred(),
        );
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

    #[test]
    fn targets_enemy_across_large_distance() {
        // Tests the fallback search (enemy far away, beyond initial radius)
        let mut app = create_ai_test_app();
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let far_enemy =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 4000.0, 100.0);
        app.update();
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, Some(far_enemy));
    }

    #[test]
    fn prefers_nearby_over_distant() {
        // Nearby enemy should be chosen even with a distant enemy in the grid
        let mut app = create_ai_test_app();
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let _far = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 3000.0, 100.0);
        let near = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 200.0, 100.0);
        app.update();
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, Some(near));
    }

    #[test]
    fn no_targets_gives_none() {
        // Seeker with no enemies at all
        let mut app = create_ai_test_app();
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        // Only spawn friendly targets
        let _friendly =
            crate::testing::spawn_test_target(app.world_mut(), Team::Player, 200.0, 100.0);
        app.update();
        let ct = app.world().get::<CurrentTarget>(player).unwrap();
        assert_eq!(ct.0, None);
    }
}

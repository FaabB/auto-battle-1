//! AI: target selection for all combat entities (units, fortresses, turrets).

use std::collections::HashMap;

use bevy::prelude::*;

use super::battlefield::CELL_SIZE;
use super::spatial_hash::SpatialHash;
use super::units::Unit;
use super::{
    CombatStats, EngagementLeash, EntityExtent, LEASH_DISTANCE, Movement, Target, TargetingState,
    Team, extent_distance,
};
use crate::screens::GameState;
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

/// Attack range hysteresis — prevents Attacking ↔ Engaging oscillation.
/// Exit Attacking when surface distance > range + `ATTACK_HYSTERESIS`.
const ATTACK_HYSTERESIS: f32 = 8.0;

/// Minimum detection radius (1 cell). Applied when `range * 2.0` is smaller.
const MIN_DETECTION_RADIUS: f32 = 64.0;

/// Maximum number of units that can simultaneously engage/attack a single unit target.
/// Only applies to unit targets (not buildings/fortresses).
const MAX_ENGAGERS_PER_UNIT_TARGET: u32 = 12;

/// Compute detection radius for a unit based on its attack range.
/// Longer-range units detect enemies from further away.
fn detection_radius(range: f32) -> f32 {
    (range * 2.0).max(MIN_DETECTION_RADIUS)
}

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

/// Finds the nearest valid target for each entity with `TargetingState`. Runs in `GameSet::Ai`.
///
/// State machine behavior:
/// - `Moving`: check detection radius → `Seeking` if enemy nearby
/// - `Seeking`: find best target → `Engaging` + leash, or back to `Moving` (mobile) / stay `Seeking` (static)
/// - `Engaging`/`Attacking`: retarget on stagger slot, switch target or disengage
///
/// Backtrack limit only applies to mobile entities (those with `Movement`).
#[allow(clippy::too_many_lines)]
pub fn find_target(
    time: Res<Time>,
    mut retarget_timer: ResMut<RetargetTimer>,
    grid: Res<TargetSpatialHash>,
    mut seekers: Query<(
        Entity,
        &Team,
        &GlobalTransform,
        &EntityExtent,
        &CombatStats,
        &mut TargetingState,
        Option<&Movement>,
    )>,
    all_targets: Query<(Entity, &Team, &GlobalTransform, &EntityExtent), With<Target>>,
    unit_markers: Query<(), With<Unit>>,
    mut commands: Commands,
) {
    retarget_timer.timer.tick(time.delta());
    let slot_advanced = retarget_timer.timer.just_finished();
    if slot_advanced {
        retarget_timer.current_slot = (retarget_timer.current_slot + 1) % RETARGET_SLOTS;
    }

    // Count current engagers per unit target (for cap check)
    let mut engager_counts: HashMap<Entity, u32> = HashMap::new();
    for (_, _, _, _, _, state, _) in &seekers {
        if let Some(target) = state.target_entity() {
            *engager_counts.entry(target).or_default() += 1;
        }
    }

    for (entity, team, transform, seeker_extent, stats, mut targeting_state, movement) in
        &mut seekers
    {
        let my_pos = transform.translation().xy();
        let opposing_team = team.opposing();
        let is_mobile = movement.is_some();

        match *targeting_state {
            TargetingState::Moving => {
                // Only mobile entities can be Moving. Check detection radius.
                let detect_r = detection_radius(stats.range);
                let has_nearby_enemy = has_enemy_in_radius(
                    &grid,
                    entity,
                    my_pos,
                    detect_r,
                    opposing_team,
                    &all_targets,
                );
                if has_nearby_enemy {
                    *targeting_state = TargetingState::Seeking;
                }
            }
            TargetingState::Seeking => {
                // Find best target
                let nearest = find_nearest_target(
                    &grid,
                    entity,
                    my_pos,
                    seeker_extent,
                    opposing_team,
                    is_mobile,
                    *team,
                    &all_targets,
                    &unit_markers,
                    &engager_counts,
                    None,
                );
                if let Some(target_entity) = nearest {
                    *targeting_state = TargetingState::Engaging(target_entity);
                    if is_mobile {
                        commands.entity(entity).insert(EngagementLeash {
                            origin: my_pos,
                            max_distance: LEASH_DISTANCE,
                        });
                    }
                } else if is_mobile {
                    // No enemies found — go back to marching
                    *targeting_state = TargetingState::Moving;
                }
                // Static entities stay Seeking if no target found
            }
            TargetingState::Engaging(_) | TargetingState::Attacking(_) => {
                // Retarget check on stagger slot
                let has_valid_target = targeting_state
                    .target_entity()
                    .is_some_and(|e| all_targets.get(e).is_ok());

                if has_valid_target {
                    if !slot_advanced {
                        continue;
                    }
                    let entity_slot = entity.index().index() % RETARGET_SLOTS;
                    if entity_slot != retarget_timer.current_slot {
                        continue;
                    }
                }

                let current_target = targeting_state.target_entity();
                let nearest = find_nearest_target(
                    &grid,
                    entity,
                    my_pos,
                    seeker_extent,
                    opposing_team,
                    is_mobile,
                    *team,
                    &all_targets,
                    &unit_markers,
                    &engager_counts,
                    current_target,
                );
                if let Some(target_entity) = nearest {
                    let old_target = targeting_state.target_entity();
                    if old_target != Some(target_entity) {
                        *targeting_state = TargetingState::Engaging(target_entity);
                        if is_mobile {
                            commands.entity(entity).insert(EngagementLeash {
                                origin: my_pos,
                                max_distance: LEASH_DISTANCE,
                            });
                        }
                    }
                } else {
                    *targeting_state = if is_mobile {
                        TargetingState::Moving
                    } else {
                        TargetingState::Seeking
                    };
                    commands.entity(entity).remove::<EngagementLeash>();
                }
            }
        }
    }
}

/// Quick check: is any opposing entity within detection radius?
/// Used for Moving → Seeking gate. Does NOT find the best target.
fn has_enemy_in_radius(
    grid: &TargetSpatialHash,
    seeker_entity: Entity,
    seeker_pos: Vec2,
    radius: f32,
    opposing_team: Team,
    all_targets: &Query<(Entity, &Team, &GlobalTransform, &EntityExtent), With<Target>>,
) -> bool {
    for candidate in grid.query_neighbors(seeker_pos, radius + MAX_ENTITY_HALF_EXTENT) {
        let Ok((cand_entity, cand_team, _, _)) = all_targets.get(candidate) else {
            continue;
        };
        if cand_entity != seeker_entity && *cand_team == opposing_team {
            return true;
        }
    }
    false
}

/// Search the spatial grid for the nearest valid target.
///
/// Two-pass strategy:
/// 1. Search within `INITIAL_SEARCH_RADIUS` (catches most cases)
/// 2. If nothing found, search the full battlefield
///
/// Within each pass, uses center-distance as a cheap pre-filter before
/// calling `extent_distance` on close candidates.
#[allow(clippy::too_many_arguments)]
fn find_nearest_target(
    grid: &TargetSpatialHash,
    seeker_entity: Entity,
    seeker_pos: Vec2,
    seeker_extent: &EntityExtent,
    opposing_team: Team,
    is_mobile: bool,
    seeker_team: Team,
    all_targets: &Query<(Entity, &Team, &GlobalTransform, &EntityExtent), With<Target>>,
    unit_markers: &Query<(), With<Unit>>,
    engager_counts: &HashMap<Entity, u32>,
    current_target: Option<Entity>,
) -> Option<Entity> {
    // First pass: nearby targets
    let result = search_radius(
        grid,
        INITIAL_SEARCH_RADIUS + MAX_ENTITY_HALF_EXTENT,
        seeker_entity,
        seeker_pos,
        seeker_extent,
        opposing_team,
        is_mobile,
        seeker_team,
        all_targets,
        unit_markers,
        engager_counts,
        current_target,
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
        seeker_extent,
        opposing_team,
        is_mobile,
        seeker_team,
        all_targets,
        unit_markers,
        engager_counts,
        current_target,
    )
}

#[allow(clippy::too_many_arguments)]
fn search_radius(
    grid: &TargetSpatialHash,
    radius: f32,
    seeker_entity: Entity,
    seeker_pos: Vec2,
    seeker_extent: &EntityExtent,
    opposing_team: Team,
    is_mobile: bool,
    seeker_team: Team,
    all_targets: &Query<(Entity, &Team, &GlobalTransform, &EntityExtent), With<Target>>,
    unit_markers: &Query<(), With<Unit>>,
    engager_counts: &HashMap<Entity, u32>,
    current_target: Option<Entity>,
) -> Option<Entity> {
    let candidates = grid.query_neighbors(seeker_pos, radius);

    // Phase 1: Filter and compute center distances (cheap)
    let mut valid_candidates: Vec<(Entity, Vec2, &EntityExtent, f32)> = Vec::new();
    for candidate_entity in candidates {
        let Ok((cand_entity, cand_team, cand_transform, cand_extent)) =
            all_targets.get(candidate_entity)
        else {
            continue;
        };

        if cand_entity == seeker_entity || *cand_team != opposing_team {
            continue;
        }

        // Skip unit targets at engager cap (exclude current target from count)
        if unit_markers.get(cand_entity).is_ok() {
            let count = engager_counts.get(&cand_entity).copied().unwrap_or(0);
            let effective_count = if current_target == Some(cand_entity) {
                count.saturating_sub(1) // Don't count self
            } else {
                count
            };
            if effective_count >= MAX_ENGAGERS_PER_UNIT_TARGET {
                continue;
            }
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
        valid_candidates.push((cand_entity, cand_pos, cand_extent, center_dist));
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
    for (cand_entity, cand_pos, cand_extent, center_dist) in &valid_candidates {
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

        let surf_dist = extent_distance(seeker_extent, seeker_pos, cand_extent, *cand_pos);
        if nearest.is_none_or(|(_, d)| surf_dist < d) {
            nearest = Some((*cand_entity, surf_dist));
        }
    }

    nearest.map(|(e, _)| e)
}

/// Per-engager data for cap enforcement sorting.
struct EngagerInfo {
    entity: Entity,
    distance: f32,
    is_attacking: bool,
    is_mobile: bool,
}

/// Evict excess engagers from unit targets. Keeps closest N, kicks farthest to Moving/Seeking.
/// Attacking units get priority (never evicted). Only applies to unit targets.
fn enforce_engager_cap(
    mut units: Query<(
        Entity,
        &GlobalTransform,
        &mut TargetingState,
        Option<&Movement>,
    )>,
    unit_targets: Query<&GlobalTransform, With<Unit>>,
    mut commands: Commands,
) {
    // Group engagers by target
    let mut engagers_by_target: HashMap<Entity, Vec<EngagerInfo>> = HashMap::new();

    for (entity, transform, state, movement) in &units {
        let (TargetingState::Engaging(target_entity) | TargetingState::Attacking(target_entity)) =
            *state
        else {
            continue;
        };
        // Only cap unit targets
        let Ok(target_gt) = unit_targets.get(target_entity) else {
            continue;
        };

        let target_pos = target_gt.translation().xy();
        engagers_by_target
            .entry(target_entity)
            .or_default()
            .push(EngagerInfo {
                entity,
                distance: transform.translation().xy().distance(target_pos),
                is_attacking: matches!(*state, TargetingState::Attacking(_)),
                is_mobile: movement.is_some(),
            });
    }

    // Enforce cap per target
    for (_target, mut engagers) in engagers_by_target {
        if engagers.len() <= MAX_ENGAGERS_PER_UNIT_TARGET as usize {
            continue;
        }

        // Sort: attacking first (never evicted), then by distance (closest first)
        engagers.sort_by(|a, b| {
            b.is_attacking.cmp(&a.is_attacking).then(
                a.distance
                    .partial_cmp(&b.distance)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
        });

        // Evict excess (from the end — farthest non-attacking)
        for engager in engagers.iter().skip(MAX_ENGAGERS_PER_UNIT_TARGET as usize) {
            if let Ok((_, _, mut state, _)) = units.get_mut(engager.entity) {
                *state = if engager.is_mobile {
                    TargetingState::Moving
                } else {
                    TargetingState::Seeking
                };
                commands.entity(engager.entity).remove::<EngagementLeash>();
            }
        }
    }
}

/// Verify targeting state against range and leash constraints.
/// Transitions:
/// - Engaging → Attacking (in attack range)
/// - Engaging → Moving (leash exceeded, mobile only)
/// - Attacking → Engaging (pushed out of range + hysteresis, mobile)
/// - Attacking → Seeking (target out of range, static)
fn verify_targets(
    mut units: Query<(
        Entity,
        &GlobalTransform,
        &EntityExtent,
        &CombatStats,
        &mut TargetingState,
        Option<&Movement>,
        Option<&EngagementLeash>,
    )>,
    targets: Query<(&GlobalTransform, &EntityExtent)>,
    mut commands: Commands,
) {
    for (entity, transform, extent, stats, mut state, movement, leash) in &mut units {
        let my_pos = transform.translation().xy();
        let is_mobile = movement.is_some();

        match *state {
            TargetingState::Engaging(target_entity) => {
                let Ok((target_pos, target_extent)) = targets.get(target_entity) else {
                    continue; // Target gone — death observer handles this
                };
                let target_xy = target_pos.translation().xy();
                let distance = extent_distance(extent, my_pos, target_extent, target_xy);

                // Check attack range → Attacking
                if distance <= stats.range {
                    *state = TargetingState::Attacking(target_entity);
                    continue;
                }

                // Check leash (mobile only)
                if is_mobile {
                    if let Some(leash) = leash {
                        if my_pos.distance(leash.origin) > leash.max_distance {
                            *state = TargetingState::Moving;
                            commands.entity(entity).remove::<EngagementLeash>();
                        }
                    }
                }
            }
            TargetingState::Attacking(target_entity) => {
                let Ok((target_pos, target_extent)) = targets.get(target_entity) else {
                    continue; // Death observer handles
                };
                let target_xy = target_pos.translation().xy();
                let distance = extent_distance(extent, my_pos, target_extent, target_xy);

                if distance > stats.range + ATTACK_HYSTERESIS {
                    if is_mobile {
                        *state = TargetingState::Engaging(target_entity);
                    } else {
                        *state = TargetingState::Seeking;
                    }
                }
            }
            _ => {} // Moving/Seeking handled by find_target
        }
    }
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
        (
            rebuild_target_grid,
            find_target,
            enforce_engager_cap,
            verify_targets,
        )
            .chain_ignore_deferred()
            .in_set(GameSet::Ai)
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian2d::prelude::Collider;
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
            (
                rebuild_target_grid,
                find_target,
                enforce_engager_cap,
                verify_targets,
            )
                .chain_ignore_deferred(),
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

    /// Spawn a fortress-like static entity (no Movement) with CombatStats.
    fn spawn_test_fortress(world: &mut World, team: Team, x: f32, y: f32) -> Entity {
        world
            .spawn((
                team,
                Target,
                TargetingState::Seeking,
                CombatStats {
                    damage: 20.0,
                    attack_speed: 1.0,
                    range: 100.0,
                },
                Transform::from_xyz(x, y, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
                EntityExtent::Rect(64.0, 64.0),
                Collider::rectangle(128.0, 128.0),
            ))
            .id()
    }

    // === Target selection tests (units start as Seeking to test find logic) ===

    #[test]
    fn seeking_unit_targets_nearest_enemy() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        // Override to Seeking to test target selection directly
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Seeking);
        let _far_enemy =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 500.0, 100.0);
        let near_enemy =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 200.0, 100.0);

        app.update();

        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(ct.target_entity(), Some(near_enemy));
    }

    #[test]
    fn seeking_unit_with_no_enemies_returns_to_moving() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Seeking);
        // Only friendly targets
        let _friendly =
            crate::testing::spawn_test_target(app.world_mut(), Team::Player, 200.0, 100.0);

        app.update();

        let ct = app.world().get::<TargetingState>(player).unwrap();
        // Mobile unit with no enemies goes back to Moving
        assert_eq!(*ct, TargetingState::Moving);
    }

    #[test]
    fn seeking_unit_gets_engagement_leash() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Seeking);
        let _enemy = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 200.0, 100.0);

        app.update();

        let leash = app.world().get::<EngagementLeash>(player);
        assert!(
            leash.is_some(),
            "Mobile unit should get EngagementLeash on Engaging"
        );
        let leash = leash.unwrap();
        assert!((leash.max_distance - LEASH_DISTANCE).abs() < f32::EPSILON);
    }

    #[test]
    fn unit_retargets_when_target_despawned() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Seeking);
        let enemy1 = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 200.0, 100.0);
        let enemy2 = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 300.0, 100.0);

        // First update: Seeking → Engaging(enemy1)
        app.update();
        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(ct.target_entity(), Some(enemy1));

        // Despawn enemy1
        app.world_mut().despawn(enemy1);

        // Next update: target is invalid, re-evaluates immediately → Engaging(enemy2)
        app.update();
        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(ct.target_entity(), Some(enemy2));
    }

    #[test]
    fn unit_switches_to_closer_target_on_retarget() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Seeking);
        let _enemy_far =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 300.0, 100.0);

        // First update: Seeking → Engaging(enemy_far)
        app.update();

        // Spawn a closer enemy
        let enemy_near =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 150.0, 100.0);

        // Set timer to fire on the player's slot next update
        set_retarget_for_entity(&mut app, player);
        app.update();

        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(ct.target_entity(), Some(enemy_near));
    }

    #[test]
    fn seeking_unit_respects_backtrack_limit() {
        let mut app = create_ai_test_app();

        // Player unit at x=500, enemy far behind at x=100 (400px behind > 128px limit)
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 500.0, 100.0);
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Seeking);
        let _behind_enemy =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 100.0, 100.0);

        app.update();

        // No valid target → back to Moving
        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(*ct, TargetingState::Moving);
    }

    #[test]
    fn seeking_unit_targets_building() {
        let mut app = create_ai_test_app();

        let enemy = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 500.0, 100.0);
        app.world_mut()
            .entity_mut(enemy)
            .insert(TargetingState::Seeking);
        let building =
            crate::testing::spawn_test_target(app.world_mut(), Team::Player, 300.0, 100.0);

        app.update();

        let ct = app.world().get::<TargetingState>(enemy).unwrap();
        assert_eq!(ct.target_entity(), Some(building));
    }

    // === Moving → detection tests ===

    #[test]
    fn moving_unit_detects_nearby_enemy() {
        let mut app = create_ai_test_app();

        // Player unit at 100, enemy at 200 (100px apart, within detection radius 128)
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let _enemy = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 200.0, 100.0);

        // Frame 1: Moving → detect enemy → Seeking
        app.update();
        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(*ct, TargetingState::Seeking);

        // Frame 2: Seeking → find target → Engaging
        app.update();
        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert!(
            ct.target_entity().is_some(),
            "Should be Engaging after detection + search"
        );
    }

    #[test]
    fn moving_unit_ignores_distant_enemy() {
        let mut app = create_ai_test_app();

        // Enemy far away (4000px), outside detection radius
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let _far_enemy =
            crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 4000.0, 100.0);

        app.update();

        // Stays Moving — enemy is too far to detect
        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(*ct, TargetingState::Moving);
    }

    #[test]
    fn moving_unit_no_enemies_stays_moving() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        let _friendly =
            crate::testing::spawn_test_target(app.world_mut(), Team::Player, 200.0, 100.0);

        app.update();

        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(*ct, TargetingState::Moving);
    }

    // === Fortress (static entity) tests ===

    #[test]
    fn fortress_targets_nearest_enemy() {
        let mut app = create_ai_test_app();

        let fortress = spawn_test_fortress(app.world_mut(), Team::Player, 64.0, 320.0);

        let near_enemy =
            crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 200.0, 320.0);
        let _far_enemy =
            crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 500.0, 320.0);

        app.update();

        let ct = app.world().get::<TargetingState>(fortress).unwrap();
        assert_eq!(ct.target_entity(), Some(near_enemy));
    }

    #[test]
    fn static_entity_has_no_backtrack_limit() {
        let mut app = create_ai_test_app();

        let fortress = spawn_test_fortress(app.world_mut(), Team::Player, 500.0, 320.0);

        let behind_enemy =
            crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 100.0, 320.0);

        app.update();

        let ct = app.world().get::<TargetingState>(fortress).unwrap();
        assert_eq!(ct.target_entity(), Some(behind_enemy));
    }

    #[test]
    fn static_entity_stays_seeking_with_no_enemies() {
        let mut app = create_ai_test_app();

        let fortress = spawn_test_fortress(app.world_mut(), Team::Player, 64.0, 320.0);

        app.update();

        let ct = app.world().get::<TargetingState>(fortress).unwrap();
        assert_eq!(*ct, TargetingState::Seeking);
    }

    // === verify_targets tests ===

    #[test]
    fn engaging_transitions_to_attacking_in_range() {
        let mut app = create_ai_test_app();

        let target = crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 110.0, 100.0);
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        // Place unit very close to target (within attack range of 5.0)
        // Unit extent = circle(6), target extent = circle(5). Surface dist = 11 - 6 - 5 = 0.
        // Actually entities at same Y, 10px apart: center dist = 10, surface = 10 - 6 - 5 = 0 (overlap)
        // That's ≤ range (5.0) → should transition to Attacking
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Engaging(target));

        app.update();

        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(*ct, TargetingState::Attacking(target));
    }

    #[test]
    fn engaging_transitions_to_moving_on_leash_exceeded() {
        let mut app = create_ai_test_app();

        let target = crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 500.0, 100.0);
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 400.0, 100.0);
        app.world_mut().entity_mut(player).insert((
            TargetingState::Engaging(target),
            EngagementLeash {
                origin: Vec2::new(100.0, 100.0), // Leash origin far behind
                max_distance: LEASH_DISTANCE,
            },
        ));

        // Unit at 400, leash origin at 100 → distance = 300 > LEASH_DISTANCE (192)
        app.update();

        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(*ct, TargetingState::Moving);
        assert!(
            app.world().get::<EngagementLeash>(player).is_none(),
            "Leash should be removed"
        );
    }

    #[test]
    fn attacking_transitions_to_engaging_on_range_exceeded() {
        let mut app = create_ai_test_app();

        // Target far away — surface distance > range + hysteresis
        let target = crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 200.0, 100.0);
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Attacking(target));

        // Surface distance: 100 - 6 - 5 = 89. Range = 5.0 + hysteresis 8.0 = 13.0. 89 > 13 → Engaging
        app.update();

        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(*ct, TargetingState::Engaging(target));
    }

    #[test]
    fn attacking_stays_within_hysteresis() {
        let mut app = create_ai_test_app();

        // Place target just barely outside attack range but within hysteresis
        let stats = crate::gameplay::units::unit_stats(crate::gameplay::units::UnitType::Soldier);
        // Unit circle(6) at 100, target circle(5) at x. Surface dist = (x-100) - 6 - 5 = x - 111
        // Want surface dist > range (5.0) but ≤ range + hysteresis (13.0)
        // x - 111 = 10 → x = 121
        let target = crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 121.0, 100.0);
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Attacking(target));

        app.update();

        let ct = app.world().get::<TargetingState>(player).unwrap();
        // Surface dist = 10, which is > range (5) but ≤ range + hysteresis (13) → stays Attacking
        assert_eq!(
            *ct,
            TargetingState::Attacking(target),
            "Should stay Attacking within hysteresis (surface_dist={}, range={}, hyst={})",
            10.0,
            stats.attack_range,
            ATTACK_HYSTERESIS
        );
    }

    #[test]
    fn static_attacking_transitions_to_seeking_on_range_exceeded() {
        let mut app = create_ai_test_app();

        let target = crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 500.0, 320.0);
        let fortress = spawn_test_fortress(app.world_mut(), Team::Player, 64.0, 320.0);
        app.world_mut()
            .entity_mut(fortress)
            .insert(TargetingState::Attacking(target));

        // Surface distance from fortress (Rect 64x64 at 64,320) to target (Circle 5 at 500,320):
        // Rect surface at x=128. Point-to-rect = 500-128 = 372. Surface = 372 - 5 = 367.
        // Range = 100 + hysteresis 8 = 108. 367 > 108 → Seeking
        app.update();

        let ct = app.world().get::<TargetingState>(fortress).unwrap();
        assert_eq!(*ct, TargetingState::Seeking);
    }

    // === Seeking fallback tests (from original suite) ===

    #[test]
    fn seeking_targets_enemy_across_large_distance() {
        // Static entity (fortress) can find far targets via fallback search
        let mut app = create_ai_test_app();

        let fortress = spawn_test_fortress(app.world_mut(), Team::Player, 100.0, 320.0);
        let far_enemy =
            crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 4000.0, 320.0);

        app.update();

        let ct = app.world().get::<TargetingState>(fortress).unwrap();
        assert_eq!(ct.target_entity(), Some(far_enemy));
    }

    #[test]
    fn seeking_prefers_nearby_over_distant() {
        let mut app = create_ai_test_app();

        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Seeking);
        let _far = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 3000.0, 100.0);
        let near = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 200.0, 100.0);

        app.update();

        let ct = app.world().get::<TargetingState>(player).unwrap();
        assert_eq!(ct.target_entity(), Some(near));
    }

    // === Death observer integration ===

    #[test]
    fn mobile_unit_death_transitions_to_moving() {
        let mut app = create_ai_test_app();

        let target = crate::testing::spawn_test_target(app.world_mut(), Team::Enemy, 200.0, 100.0);
        let player = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        app.world_mut()
            .entity_mut(player)
            .insert(TargetingState::Engaging(target));

        // Engaging with no valid target → retargets; since target still exists, stays engaging
        // Actually we need to test the death observer separately — this test is in death.rs
        // Instead test: engaging unit whose target gets lost reverts via find_target
        app.world_mut().despawn(target);
        app.update();

        let ct = app.world().get::<TargetingState>(player).unwrap();
        // No enemies left → Mobile unit: Moving
        assert_eq!(*ct, TargetingState::Moving);
    }

    // === Engager cap tests ===

    #[test]
    fn engager_cap_evicts_excess_units() {
        let mut app = create_ai_test_app();

        // Spawn 1 enemy target
        let _enemy = crate::testing::spawn_test_unit(app.world_mut(), Team::Enemy, 500.0, 100.0);

        // Spawn 20 player units all Seeking → they should all try to engage the enemy
        let mut player_units = Vec::new();
        for i in 0..20 {
            let unit = crate::testing::spawn_test_unit(
                app.world_mut(),
                Team::Player,
                100.0 + i as f32 * 10.0,
                100.0,
            );
            app.world_mut()
                .entity_mut(unit)
                .insert(TargetingState::Seeking);
            player_units.push(unit);
        }

        // Run a few frames to let find_target + enforce_engager_cap settle
        for _ in 0..3 {
            app.update();
        }

        // Count how many are Engaging/Attacking
        let engaged_count = player_units
            .iter()
            .filter(|&&unit| {
                let state = app.world().get::<TargetingState>(unit).unwrap();
                matches!(
                    *state,
                    TargetingState::Engaging(_) | TargetingState::Attacking(_)
                )
            })
            .count();

        assert!(
            engaged_count <= MAX_ENGAGERS_PER_UNIT_TARGET as usize,
            "Expected at most {} engagers, got {}",
            MAX_ENGAGERS_PER_UNIT_TARGET,
            engaged_count
        );
    }
}

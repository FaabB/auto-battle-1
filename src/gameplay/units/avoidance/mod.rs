//! ORCA local avoidance for unit-to-unit collision prevention.

pub mod orca;

use std::collections::HashMap;

use bevy::prelude::*;

use self::orca::AgentSnapshot;
use super::{Movement, UNIT_RADIUS, Unit};
use crate::gameplay::spatial_hash::SpatialHash;
use crate::gameplay::{TargetingState, Team};

// === Constants ===

/// Inflated ORCA avoidance radius (2× visual radius).
/// Gives ORCA more margin for planning without affecting visual overlap detection.
pub const AVOIDANCE_RADIUS: f32 = UNIT_RADIUS * 2.0;

/// Default ORCA time horizon in seconds.
const DEFAULT_TIME_HORIZON: f32 = 5.0;
/// Maximum neighbors to consider per agent.
const DEFAULT_MAX_NEIGHBORS: u32 = 10;
/// Velocity smoothing blend factor (0.0 = keep old, 1.0 = fully ORCA).
const DEFAULT_VELOCITY_SMOOTHING: f32 = 1.0;
/// Number of iterations for `resolve_overlaps`. Multiple passes improve convergence
/// in dense groups where a single pass can't fully separate all overlapping pairs.
const OVERLAP_ITERATIONS: u32 = 3;
/// ORCA responsibility for engaging/attacking units (dodge less).
const ENGAGING_RESPONSIBILITY: f32 = 0.25;
/// ORCA responsibility for moving/seeking units (yield more).
const MOVING_RESPONSIBILITY: f32 = 0.75;
/// Boids separation neighbor detection radius (4× unit radius).
const SEPARATION_RADIUS: f32 = UNIT_RADIUS * 4.0;
/// Boids separation force strength.
const SEPARATION_STRENGTH: f32 = 30.0;

// === Components ===

/// The velocity the unit wants to move at (from flow field / movement logic).
/// Written by `unit_movement`, read by `compute_avoidance`.
#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component)]
pub struct PreferredVelocity(pub Vec2);

/// The final velocity after ORCA adjustment.
/// Written by `compute_avoidance`, read by `apply_movement`.
/// Replaces `AdjustedVelocity` from avian2d for unit movement.
#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component)]
pub struct AdjustedVelocity(pub Vec2);

/// Per-unit ORCA parameters.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct AvoidanceAgent {
    /// Avoidance radius (typically matches collider radius).
    pub radius: f32,
    /// How much of the avoidance adjustment this agent absorbs (0.0–1.0).
    /// 0.5 = symmetric (both agents dodge equally). 1.0 = this agent takes full responsibility.
    pub responsibility: f32,
}

impl Default for AvoidanceAgent {
    fn default() -> Self {
        Self {
            radius: AVOIDANCE_RADIUS,
            responsibility: 0.5,
        }
    }
}

// === Resources ===

/// Global ORCA tuning parameters.
#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct AvoidanceConfig {
    /// How far ahead (seconds) agents predict collisions with each other.
    pub time_horizon: f32,
    /// Max neighbors to consider per agent. Caps ORCA constraint count.
    pub max_neighbors: u32,
    /// Search radius for neighbors (pixels). Should be >= `max_speed * time_horizon`.
    pub neighbor_distance: f32,
    /// Blend factor for velocity smoothing (0.0 = old velocity, 1.0 = raw ORCA result).
    pub velocity_smoothing: f32,
    /// Shorter time horizon for stationary (attacking) neighbors.
    pub static_time_horizon: f32,
}

impl Default for AvoidanceConfig {
    fn default() -> Self {
        Self {
            time_horizon: DEFAULT_TIME_HORIZON,
            neighbor_distance: DEFAULT_TIME_HORIZON.mul_add(50.0, AVOIDANCE_RADIUS * 2.0),
            max_neighbors: DEFAULT_MAX_NEIGHBORS,
            velocity_smoothing: DEFAULT_VELOCITY_SMOOTHING,
            static_time_horizon: 0.5,
        }
    }
}

/// Spatial hash for ORCA avoidance neighbor lookups.
/// Populated with `With<Unit>` entities each frame.
#[derive(Resource, Debug)]
pub struct AvoidanceSpatialHash(pub SpatialHash);

impl std::ops::Deref for AvoidanceSpatialHash {
    type Target = SpatialHash;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for AvoidanceSpatialHash {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// === Systems ===

/// Rebuild the spatial hash with all unit positions. Runs every frame.
pub fn rebuild_spatial_hash(
    mut hash: ResMut<AvoidanceSpatialHash>,
    agents: Query<(Entity, &GlobalTransform), With<Unit>>,
) {
    hash.clear();
    for (entity, transform) in &agents {
        hash.insert(entity, transform.translation().xy());
    }
}

/// Boids-style reactive repulsion between same-team units.
///
/// - Same-team only — opposing units are invisible
/// - Tangential push for units sharing a target (perpendicular to target→unit line)
/// - Radial push for units with different targets
/// - Skips stationary (attacking) units
/// - Blended into `PreferredVelocity`, clamped to max speed
pub fn apply_separation(
    hash: Res<AvoidanceSpatialHash>,
    mut units: Query<
        (
            Entity,
            &GlobalTransform,
            &mut PreferredVelocity,
            &Movement,
            &TargetingState,
            &Team,
        ),
        With<Unit>,
    >,
    targets: Query<&GlobalTransform>,
) {
    // Snapshot: can't read neighbor data while iterating query mutably
    let snapshots: Vec<(Entity, Vec2, Vec2, f32, Option<Entity>, Team)> = units
        .iter()
        .map(|(e, gt, pv, mv, ts, team)| {
            (
                e,
                gt.translation().xy(),
                pv.0,
                mv.speed,
                ts.target_entity(),
                *team,
            )
        })
        .collect();

    let index_map: HashMap<Entity, usize> = snapshots
        .iter()
        .enumerate()
        .map(|(i, (e, ..))| (*e, i))
        .collect();

    let results: Vec<(Entity, Vec2)> = snapshots
        .iter()
        .map(|(entity, pos, preferred, max_speed, target, team)| {
            // Skip stationary units
            if preferred.length_squared() < f32::EPSILON {
                return (*entity, *preferred);
            }

            let mut separation = Vec2::ZERO;
            let neighbors = hash.query_neighbors(*pos, SEPARATION_RADIUS);

            for neighbor_entity in neighbors {
                if neighbor_entity == *entity {
                    continue;
                }
                let Some(&idx) = index_map.get(&neighbor_entity) else {
                    continue;
                };
                let (_, n_pos, _, _, n_target, n_team) = &snapshots[idx];

                // Same-team only
                if n_team != team {
                    continue;
                }

                let diff = *pos - *n_pos;
                let dist_sq = diff.length_squared();
                let sep_radius_sq = SEPARATION_RADIUS * SEPARATION_RADIUS;
                if dist_sq >= sep_radius_sq || dist_sq < f32::EPSILON {
                    continue;
                }

                let dist = dist_sq.sqrt();
                let strength = SEPARATION_STRENGTH * (1.0 - dist / SEPARATION_RADIUS);

                // Tangential push for shared targets
                let radial = diff / dist;
                let push_dir = if *target == *n_target && target.is_some() {
                    let target_entity = target.unwrap();
                    targets.get(target_entity).map_or(radial, |target_gt| {
                        let to_unit = *pos - target_gt.translation().xy();
                        if to_unit.length_squared() > f32::EPSILON {
                            // Perpendicular to target→unit line
                            let tangent = Vec2::new(-to_unit.y, to_unit.x).normalize();
                            // Pick tangent that points away from neighbor
                            if tangent.dot(radial) >= 0.0 {
                                tangent
                            } else {
                                -tangent
                            }
                        } else {
                            radial
                        }
                    })
                } else {
                    radial
                };

                separation += push_dir * strength;
            }

            let mut new_preferred = *preferred + separation;
            // Clamp to max speed
            if new_preferred.length_squared() > max_speed * max_speed {
                new_preferred = new_preferred.normalize() * max_speed;
            }
            (*entity, new_preferred)
        })
        .collect();

    // Write results
    for (entity, new_preferred) in results {
        if let Ok((_, _, mut preferred, _, _, _)) = units.get_mut(entity) {
            preferred.0 = new_preferred;
        }
    }
}

/// Compute ORCA-adjusted velocities for all units.
///
/// Reads `PreferredVelocity` (desired direction from flow field / movement) and
/// `AdjustedVelocity` (current velocity from last frame's ORCA output).
/// Writes the ORCA result to `AdjustedVelocity`.
pub fn compute_avoidance(
    config: Res<AvoidanceConfig>,
    hash: Res<AvoidanceSpatialHash>,
    mut agents: Query<
        (
            Entity,
            &GlobalTransform,
            &mut AdjustedVelocity,
            &PreferredVelocity,
            &AvoidanceAgent,
            &Movement,
            &TargetingState,
            &Team,
        ),
        With<Unit>,
    >,
) {
    // Phase 1: Snapshot all agent data (immutable read via .iter())
    let snapshots: Vec<(Entity, AgentSnapshot, Team)> = agents
        .iter()
        .map(
            |(
                entity,
                transform,
                adjusted,
                preferred,
                avoidance,
                movement,
                targeting_state,
                team,
            )| {
                let responsibility = match *targeting_state {
                    TargetingState::Engaging(_) | TargetingState::Attacking(_) => {
                        ENGAGING_RESPONSIBILITY
                    }
                    TargetingState::Moving | TargetingState::Seeking => MOVING_RESPONSIBILITY,
                };
                (
                    entity,
                    AgentSnapshot {
                        position: transform.translation().xy(),
                        velocity: adjusted.0,
                        preferred: preferred.0,
                        radius: avoidance.radius,
                        max_speed: movement.speed,
                        responsibility,
                    },
                    *team,
                )
            },
        )
        .collect();

    // Build entity -> snapshot index lookup for neighbor access
    let index_map: HashMap<Entity, usize> = snapshots
        .iter()
        .enumerate()
        .map(|(i, (e, ..))| (*e, i))
        .collect();

    // Phase 2: Compute ORCA velocity for each agent
    let results: Vec<(Entity, Vec2)> = snapshots
        .iter()
        .map(|(entity, agent, agent_team)| {
            // Skip ORCA for stationary agents with zero preferred velocity
            if agent.preferred.length_squared() < f32::EPSILON {
                return (*entity, Vec2::ZERO);
            }

            // Gather neighbor snapshots and compute ORCA lines
            let mut lines = Vec::new();
            let candidates = hash.query_neighbors(agent.position, config.neighbor_distance);
            let mut neighbor_count = 0u32;

            for candidate_entity in candidates {
                if candidate_entity == *entity {
                    continue;
                }
                if neighbor_count >= config.max_neighbors {
                    break;
                }
                if let Some(&idx) = index_map.get(&candidate_entity) {
                    let (_, neighbor, neighbor_team) = &snapshots[idx];
                    // Same-team ORCA only — don't avoid enemies, you want to fight them
                    if neighbor_team != agent_team {
                        continue;
                    }
                    // Use shorter time horizon for stationary neighbors
                    let time_horizon = if neighbor.preferred.length_squared() < f32::EPSILON {
                        config.static_time_horizon
                    } else {
                        config.time_horizon
                    };
                    let line = orca::compute_orca_line(agent, neighbor, time_horizon);
                    lines.push(line);
                    neighbor_count += 1;
                }
            }

            // No neighbors nearby — use preferred velocity directly
            if lines.is_empty() {
                return (*entity, agent.preferred);
            }

            let orca_vel =
                orca::compute_avoiding_velocity(agent.preferred, agent.max_speed, &lines);

            // Velocity smoothing: blend ORCA result with current velocity
            let smoothed = agent.velocity.lerp(orca_vel, config.velocity_smoothing);
            (*entity, smoothed)
        })
        .collect();

    // Phase 3: Write results
    for (entity, new_velocity) in results {
        if let Ok((_, _, mut adjusted, _, _, _, _, _)) = agents.get_mut(entity) {
            adjusted.0 = new_velocity;
        }
    }
}

/// Apply ORCA-adjusted velocity to transform directly.
/// Replaces avian2d physics integration for unit movement.
pub fn apply_movement(
    time: Res<Time>,
    mut units: Query<(&AdjustedVelocity, &mut Transform), With<Unit>>,
) {
    let dt = time.delta_secs();
    for (velocity, mut transform) in &mut units {
        transform.translation.x += velocity.0.x * dt;
        transform.translation.y += velocity.0.y * dt;
    }
}

/// Hard positional overlap correction. Runs after `apply_movement` as a safety net.
///
/// Uses `AvoidanceSpatialHash` for O(n × k) neighbor lookup per iteration.
/// Runs `OVERLAP_ITERATIONS` passes for better convergence in dense groups.
///
/// Overlap rules:
/// - Moving unit vs moving: split equally
/// - Moving unit vs stationary (attacking): only the moving unit gets pushed
/// - Both stationary: split equally to unstick
///
/// Uses visual `UNIT_RADIUS` (not inflated `AVOIDANCE_RADIUS`) so units can stand side-by-side.
/// Applies to ALL teams (cross-team overlap resolution).
pub fn resolve_overlaps(
    hash: Res<AvoidanceSpatialHash>,
    mut units: Query<(Entity, &mut Transform, &AdjustedVelocity), With<Unit>>,
) {
    let min_dist = UNIT_RADIUS * 2.0;
    let min_dist_sq = min_dist * min_dist;

    for _ in 0..OVERLAP_ITERATIONS {
        // Snapshot positions for consistent reads within this iteration
        let snapshots: Vec<(Entity, Vec2, bool)> = units
            .iter()
            .map(|(e, t, v)| {
                let is_stationary = v.0.length_squared() < f32::EPSILON;
                (e, t.translation.xy(), is_stationary)
            })
            .collect();

        let index_map: HashMap<Entity, usize> = snapshots
            .iter()
            .enumerate()
            .map(|(i, (e, ..))| (*e, i))
            .collect();

        // Collect corrections (entity → displacement) to apply after iteration
        let mut corrections: HashMap<Entity, Vec2> = HashMap::new();

        for &(entity_a, pos_a, stationary_a) in &snapshots {
            // Use spatial hash for neighbor lookup instead of all-pairs
            let neighbors = hash.query_neighbors(pos_a, min_dist);

            for neighbor_entity in neighbors {
                // Only process each pair once (a < b by entity index)
                if neighbor_entity.index() <= entity_a.index() {
                    continue;
                }

                let Some(&idx_b) = index_map.get(&neighbor_entity) else {
                    continue;
                };
                let (entity_b, pos_b, stationary_b) = snapshots[idx_b];

                let diff = pos_b - pos_a;
                let dist_sq = diff.length_squared();

                if dist_sq >= min_dist_sq || dist_sq < f32::EPSILON * f32::EPSILON {
                    continue;
                }

                let dist = dist_sq.sqrt();
                let overlap = min_dist - dist;
                let direction = diff / dist;

                match (stationary_a, stationary_b) {
                    (false, true) => {
                        // Only push the moving unit (a) away
                        *corrections.entry(entity_a).or_default() -= direction * overlap;
                    }
                    (true, false) => {
                        // Only push the moving unit (b) away
                        *corrections.entry(entity_b).or_default() += direction * overlap;
                    }
                    _ => {
                        // Both moving or both stationary: split equally
                        let half_overlap = overlap * 0.5;
                        *corrections.entry(entity_a).or_default() -= direction * half_overlap;
                        *corrections.entry(entity_b).or_default() += direction * half_overlap;
                    }
                }
            }
        }

        // Apply accumulated corrections
        for (entity, correction) in corrections {
            if let Ok((_, mut transform, _)) = units.get_mut(entity) {
                transform.translation.x += correction.x;
                transform.translation.y += correction.y;
            }
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    fn create_avoidance_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AvoidanceConfig>();
        app.insert_resource(AvoidanceSpatialHash(SpatialHash::new(
            AvoidanceConfig::default().neighbor_distance,
        )));
        app.add_systems(
            Update,
            (rebuild_spatial_hash, compute_avoidance).chain_ignore_deferred(),
        );
        app.update(); // Initialize time
        app
    }

    fn spawn_avoidance_unit(
        world: &mut World,
        x: f32,
        y: f32,
        preferred: Vec2,
        current_vel: Vec2,
    ) -> Entity {
        world
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(x, y, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
                PreferredVelocity(preferred),
                AvoidanceAgent::default(),
                AdjustedVelocity(current_vel),
                TargetingState::Moving,
                Team::Player,
            ))
            .id()
    }

    #[test]
    fn lone_unit_keeps_preferred_velocity() {
        let mut app = create_avoidance_test_app();
        let unit = spawn_avoidance_unit(
            app.world_mut(),
            100.0,
            100.0,
            Vec2::new(50.0, 0.0),
            Vec2::new(50.0, 0.0),
        );
        app.update();
        let vel = app.world().get::<AdjustedVelocity>(unit).unwrap();
        assert!(
            (vel.0 - Vec2::new(50.0, 0.0)).length() < 1.0,
            "Lone unit should keep preferred, got {:?}",
            vel.0
        );
    }

    #[test]
    fn head_on_units_steer_apart() {
        let mut app = create_avoidance_test_app();
        let a = spawn_avoidance_unit(
            app.world_mut(),
            100.0,
            100.0,
            Vec2::new(50.0, 0.0),
            Vec2::new(50.0, 0.0),
        );
        let b = spawn_avoidance_unit(
            app.world_mut(),
            130.0,
            100.0,
            Vec2::new(-50.0, 0.0),
            Vec2::new(-50.0, 0.0),
        );
        app.update();
        let vel_a = app.world().get::<AdjustedVelocity>(a).unwrap();
        let vel_b = app.world().get::<AdjustedVelocity>(b).unwrap();
        // Both should have some lateral (y) component to avoid each other
        assert!(
            vel_a.0.y.abs() > 0.1 || vel_b.0.y.abs() > 0.1,
            "Head-on units should steer laterally: a={:?}, b={:?}",
            vel_a.0,
            vel_b.0
        );
    }

    #[test]
    fn zero_preferred_stays_zero() {
        let mut app = create_avoidance_test_app();
        let unit = spawn_avoidance_unit(app.world_mut(), 100.0, 100.0, Vec2::ZERO, Vec2::ZERO);
        app.update();
        let vel = app.world().get::<AdjustedVelocity>(unit).unwrap();
        assert!(vel.0.length() < f32::EPSILON);
    }

    #[test]
    fn distant_units_no_avoidance() {
        let mut app = create_avoidance_test_app();
        let a = spawn_avoidance_unit(
            app.world_mut(),
            0.0,
            0.0,
            Vec2::new(50.0, 0.0),
            Vec2::new(50.0, 0.0),
        );
        let _b = spawn_avoidance_unit(
            app.world_mut(),
            1000.0,
            1000.0,
            Vec2::new(-50.0, 0.0),
            Vec2::new(-50.0, 0.0),
        );
        app.update();
        let vel = app.world().get::<AdjustedVelocity>(a).unwrap();
        assert!(
            (vel.0 - Vec2::new(50.0, 0.0)).length() < 1.0,
            "Distant agents should not affect each other, got {:?}",
            vel.0
        );
    }

    #[test]
    fn opposing_team_units_no_avoidance() {
        let mut app = create_avoidance_test_app();
        let a = {
            let world = app.world_mut();
            world
                .spawn((
                    Unit,
                    Movement { speed: 50.0 },
                    Transform::from_xyz(100.0, 100.0, 0.0),
                    GlobalTransform::from(Transform::from_xyz(100.0, 100.0, 0.0)),
                    PreferredVelocity(Vec2::new(50.0, 0.0)),
                    AvoidanceAgent::default(),
                    AdjustedVelocity(Vec2::new(50.0, 0.0)),
                    TargetingState::Moving,
                    Team::Player,
                ))
                .id()
        };
        let _b = {
            let world = app.world_mut();
            world
                .spawn((
                    Unit,
                    Movement { speed: 50.0 },
                    Transform::from_xyz(130.0, 100.0, 0.0),
                    GlobalTransform::from(Transform::from_xyz(130.0, 100.0, 0.0)),
                    PreferredVelocity(Vec2::new(-50.0, 0.0)),
                    AvoidanceAgent::default(),
                    AdjustedVelocity(Vec2::new(-50.0, 0.0)),
                    TargetingState::Moving,
                    Team::Enemy,
                ))
                .id()
        };
        app.update();
        let vel = app.world().get::<AdjustedVelocity>(a).unwrap();
        // Should NOT steer laterally — enemy is invisible to ORCA
        assert!(
            vel.0.y.abs() < 1.0,
            "Opposing team units should not trigger avoidance, got {:?}",
            vel.0
        );
    }

    #[test]
    fn resolve_overlaps_pushes_overlapping_units_apart() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(AvoidanceSpatialHash(SpatialHash::new(
            AvoidanceConfig::default().neighbor_distance,
        )));
        app.add_systems(
            Update,
            (rebuild_spatial_hash, resolve_overlaps).chain_ignore_deferred(),
        );
        app.update(); // Initialize time

        // Two units overlapping at nearly the same position (5px apart, min_dist = 2*UNIT_RADIUS = 12)
        let a = app
            .world_mut()
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(100.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(100.0, 100.0, 0.0)),
                AdjustedVelocity(Vec2::new(50.0, 0.0)), // moving
            ))
            .id();
        let b = app
            .world_mut()
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(105.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(105.0, 100.0, 0.0)),
                AdjustedVelocity(Vec2::new(50.0, 0.0)), // moving
            ))
            .id();

        app.update();

        let pos_a = app.world().get::<Transform>(a).unwrap().translation.xy();
        let pos_b = app.world().get::<Transform>(b).unwrap().translation.xy();
        let dist = pos_a.distance(pos_b);

        assert!(
            dist > 5.0,
            "Overlapping units should be pushed apart, distance went from 5.0 to {dist}"
        );
    }

    #[test]
    fn resolve_overlaps_only_pushes_moving_unit_vs_stationary() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(AvoidanceSpatialHash(SpatialHash::new(
            AvoidanceConfig::default().neighbor_distance,
        )));
        app.add_systems(
            Update,
            (rebuild_spatial_hash, resolve_overlaps).chain_ignore_deferred(),
        );
        app.update();

        // Stationary unit (attacking, vel=ZERO)
        let stationary = app
            .world_mut()
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(100.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(100.0, 100.0, 0.0)),
                AdjustedVelocity(Vec2::ZERO),
            ))
            .id();
        // Moving unit overlapping the stationary one
        let moving = app
            .world_mut()
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(105.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(105.0, 100.0, 0.0)),
                AdjustedVelocity(Vec2::new(50.0, 0.0)),
            ))
            .id();

        app.update();

        let stat_pos = app
            .world()
            .get::<Transform>(stationary)
            .unwrap()
            .translation
            .xy();
        let move_pos = app
            .world()
            .get::<Transform>(moving)
            .unwrap()
            .translation
            .xy();

        // Stationary unit should not have moved
        assert!(
            (stat_pos - Vec2::new(100.0, 100.0)).length() < f32::EPSILON,
            "Stationary unit should not move, got {stat_pos:?}"
        );
        // Moving unit should have been pushed away
        assert!(
            move_pos.x > 105.0,
            "Moving unit should be pushed away from stationary, got {move_pos:?}"
        );
    }

    #[test]
    fn apply_separation_modifies_preferred_velocity() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(AvoidanceSpatialHash(SpatialHash::new(
            AvoidanceConfig::default().neighbor_distance,
        )));
        app.add_systems(
            Update,
            (rebuild_spatial_hash, apply_separation).chain_ignore_deferred(),
        );
        app.update();

        let preferred = Vec2::new(50.0, 0.0);
        // Two same-team units close together (within SEPARATION_RADIUS = 4 * UNIT_RADIUS = 24)
        let a = app
            .world_mut()
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(100.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(100.0, 100.0, 0.0)),
                PreferredVelocity(preferred),
                TargetingState::Moving,
                Team::Player,
            ))
            .id();
        let _b = app
            .world_mut()
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(110.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(110.0, 100.0, 0.0)),
                PreferredVelocity(preferred),
                TargetingState::Moving,
                Team::Player,
            ))
            .id();

        app.update();

        let new_pv = app.world().get::<PreferredVelocity>(a).unwrap().0;
        // Separation should have modified the preferred velocity (radial push away from neighbor)
        assert!(
            (new_pv - preferred).length() > 0.1,
            "Separation should modify preferred velocity, got {new_pv:?} (original: {preferred:?})"
        );
    }

    #[test]
    fn apply_separation_ignores_opposing_team() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(AvoidanceSpatialHash(SpatialHash::new(
            AvoidanceConfig::default().neighbor_distance,
        )));
        app.add_systems(
            Update,
            (rebuild_spatial_hash, apply_separation).chain_ignore_deferred(),
        );
        app.update();

        let preferred = Vec2::new(50.0, 0.0);
        let a = app
            .world_mut()
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(100.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(100.0, 100.0, 0.0)),
                PreferredVelocity(preferred),
                TargetingState::Moving,
                Team::Player,
            ))
            .id();
        let _b = app
            .world_mut()
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(110.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(110.0, 100.0, 0.0)),
                PreferredVelocity(Vec2::new(-50.0, 0.0)),
                TargetingState::Moving,
                Team::Enemy,
            ))
            .id();

        app.update();

        let new_pv = app.world().get::<PreferredVelocity>(a).unwrap().0;
        // Opposing team should not affect separation
        assert!(
            (new_pv - preferred).length() < f32::EPSILON,
            "Opposing team should not trigger separation, got {new_pv:?}"
        );
    }
}

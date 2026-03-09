//! Unit avoidance: boids separation + overlap resolution.
//!
//! No ORCA — separation + `resolve_overlaps` proved sufficient and avoids
//! the oscillation ORCA caused with small unit counts.

use std::collections::HashMap;

use bevy::prelude::*;

use super::{Movement, UNIT_RADIUS, Unit};
use crate::gameplay::spatial_hash::SpatialHash;
use crate::gameplay::{TargetingState, Team};

// === Constants ===

/// Number of iterations for `resolve_overlaps`. Multiple passes improve convergence
/// in dense groups where a single pass can't fully separate all overlapping pairs.
const OVERLAP_ITERATIONS: u32 = 3;
/// Boids separation neighbor detection radius (4× unit radius).
pub const SEPARATION_RADIUS: f32 = UNIT_RADIUS * 4.0;
/// Boids separation force strength.
const SEPARATION_STRENGTH: f32 = 30.0;

// === Components ===

/// The velocity the unit wants to move at (from flow field / movement logic).
/// Written by `unit_movement`, read by `apply_separation` and `apply_movement`.
#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component)]
pub struct PreferredVelocity(pub Vec2);

/// The final velocity after avoidance adjustment.
/// Written by `finalize_velocity`, read by `apply_movement` and `resolve_overlaps`.
#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component)]
pub struct AdjustedVelocity(pub Vec2);

// === Resources ===

/// Spatial hash for avoidance neighbor lookups.
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

/// Snapshot for separation computation.
struct SeparationSnapshot {
    entity: Entity,
    pos: Vec2,
    preferred: Vec2,
    max_speed: f32,
    target: Option<Entity>,
    team: Team,
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
    let snapshots: Vec<SeparationSnapshot> = units
        .iter()
        .map(|(e, gt, pv, mv, ts, team)| SeparationSnapshot {
            entity: e,
            pos: gt.translation().xy(),
            preferred: pv.0,
            max_speed: mv.speed,
            target: ts.target_entity(),
            team: *team,
        })
        .collect();

    let index_map: HashMap<Entity, usize> = snapshots
        .iter()
        .enumerate()
        .map(|(i, s)| (s.entity, i))
        .collect();

    let sep_radius_sq = SEPARATION_RADIUS * SEPARATION_RADIUS;

    let results: Vec<(Entity, Vec2)> = snapshots
        .iter()
        .map(|snap| {
            // Skip stationary units
            if snap.preferred.length_squared() < f32::EPSILON {
                return (snap.entity, snap.preferred);
            }

            let mut separation = Vec2::ZERO;
            let neighbors = hash.query_neighbors(snap.pos, SEPARATION_RADIUS);

            for neighbor_entity in neighbors {
                if neighbor_entity == snap.entity {
                    continue;
                }
                let Some(&idx) = index_map.get(&neighbor_entity) else {
                    continue;
                };
                let neighbor = &snapshots[idx];

                // Same-team only
                if neighbor.team != snap.team {
                    continue;
                }

                let diff = snap.pos - neighbor.pos;
                let dist_sq = diff.length_squared();
                if dist_sq >= sep_radius_sq || dist_sq < f32::EPSILON {
                    continue;
                }

                let dist = dist_sq.sqrt();
                let strength = SEPARATION_STRENGTH * (1.0 - dist / SEPARATION_RADIUS);

                // Tangential push for shared targets
                let radial = diff / dist;
                let push_dir = if snap.target == neighbor.target && snap.target.is_some() {
                    let target_entity = snap.target.unwrap();
                    targets.get(target_entity).map_or(radial, |target_gt| {
                        let to_unit = snap.pos - target_gt.translation().xy();
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

            let mut new_preferred = snap.preferred + separation;
            // Clamp to max speed
            if new_preferred.length_squared() > snap.max_speed * snap.max_speed {
                new_preferred = new_preferred.normalize() * snap.max_speed;
            }
            (snap.entity, new_preferred)
        })
        .collect();

    // Write results
    for (entity, new_preferred) in results {
        if let Ok((_, _, mut preferred, _, _, _)) = units.get_mut(entity) {
            preferred.0 = new_preferred;
        }
    }
}

/// Copy `PreferredVelocity` (after separation) to `AdjustedVelocity`.
/// Bridges the separation output to the movement/overlap pipeline.
pub fn finalize_velocity(
    mut units: Query<(&PreferredVelocity, &mut AdjustedVelocity), With<Unit>>,
) {
    for (preferred, mut adjusted) in &mut units {
        adjusted.0 = preferred.0;
    }
}

/// Apply adjusted velocity to transform directly.
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
/// Uses visual `UNIT_RADIUS` so units can stand side-by-side.
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
mod tests {
    use super::*;

    fn create_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(AvoidanceSpatialHash(SpatialHash::new(SEPARATION_RADIUS)));
        app.update(); // Initialize time
        app
    }

    fn spawn_unit(world: &mut World, x: f32, y: f32, preferred: Vec2, team: Team) -> Entity {
        world
            .spawn((
                Unit,
                Movement { speed: 50.0 },
                Transform::from_xyz(x, y, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, y, 0.0)),
                PreferredVelocity(preferred),
                AdjustedVelocity::default(),
                TargetingState::Moving,
                team,
            ))
            .id()
    }

    // === finalize_velocity tests ===

    #[test]
    fn finalize_copies_preferred_to_adjusted() {
        let mut app = create_test_app();
        app.add_systems(Update, finalize_velocity);

        let unit = spawn_unit(
            app.world_mut(),
            100.0,
            100.0,
            Vec2::new(50.0, 0.0),
            Team::Player,
        );

        app.update();

        let vel = app.world().get::<AdjustedVelocity>(unit).unwrap();
        assert!(
            (vel.0 - Vec2::new(50.0, 0.0)).length() < f32::EPSILON,
            "Should copy preferred to adjusted, got {:?}",
            vel.0
        );
    }

    #[test]
    fn zero_preferred_stays_zero() {
        let mut app = create_test_app();
        app.add_systems(Update, finalize_velocity);

        let unit = spawn_unit(app.world_mut(), 100.0, 100.0, Vec2::ZERO, Team::Player);

        app.update();

        let vel = app.world().get::<AdjustedVelocity>(unit).unwrap();
        assert!(vel.0.length() < f32::EPSILON);
    }

    // === resolve_overlaps tests ===

    #[test]
    fn resolve_overlaps_pushes_overlapping_units_apart() {
        let mut app = create_test_app();
        app.add_systems(
            Update,
            (rebuild_spatial_hash, resolve_overlaps).chain_ignore_deferred(),
        );

        // Two units 5px apart (min_dist = 2*UNIT_RADIUS = 12)
        let a = app
            .world_mut()
            .spawn((
                Unit,
                Transform::from_xyz(100.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(100.0, 100.0, 0.0)),
                AdjustedVelocity(Vec2::new(50.0, 0.0)),
            ))
            .id();
        let b = app
            .world_mut()
            .spawn((
                Unit,
                Transform::from_xyz(105.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(105.0, 100.0, 0.0)),
                AdjustedVelocity(Vec2::new(50.0, 0.0)),
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
        let mut app = create_test_app();
        app.add_systems(
            Update,
            (rebuild_spatial_hash, resolve_overlaps).chain_ignore_deferred(),
        );

        let stationary = app
            .world_mut()
            .spawn((
                Unit,
                Transform::from_xyz(100.0, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(100.0, 100.0, 0.0)),
                AdjustedVelocity(Vec2::ZERO),
            ))
            .id();
        let moving = app
            .world_mut()
            .spawn((
                Unit,
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

        assert!(
            (stat_pos - Vec2::new(100.0, 100.0)).length() < f32::EPSILON,
            "Stationary unit should not move, got {stat_pos:?}"
        );
        assert!(
            move_pos.x > 105.0,
            "Moving unit should be pushed away from stationary, got {move_pos:?}"
        );
    }

    // === apply_separation tests ===

    #[test]
    fn apply_separation_modifies_preferred_velocity() {
        let mut app = create_test_app();
        app.add_systems(
            Update,
            (rebuild_spatial_hash, apply_separation).chain_ignore_deferred(),
        );

        let preferred = Vec2::new(50.0, 0.0);
        let a = spawn_unit(app.world_mut(), 100.0, 100.0, preferred, Team::Player);
        let _b = spawn_unit(app.world_mut(), 110.0, 100.0, preferred, Team::Player);

        app.update();

        let new_pv = app.world().get::<PreferredVelocity>(a).unwrap().0;
        assert!(
            (new_pv - preferred).length() > 0.1,
            "Separation should modify preferred velocity, got {new_pv:?} (original: {preferred:?})"
        );
    }

    #[test]
    fn apply_separation_ignores_opposing_team() {
        let mut app = create_test_app();
        app.add_systems(
            Update,
            (rebuild_spatial_hash, apply_separation).chain_ignore_deferred(),
        );

        let preferred = Vec2::new(50.0, 0.0);
        let a = spawn_unit(app.world_mut(), 100.0, 100.0, preferred, Team::Player);
        let _b = spawn_unit(
            app.world_mut(),
            110.0,
            100.0,
            Vec2::new(-50.0, 0.0),
            Team::Enemy,
        );

        app.update();

        let new_pv = app.world().get::<PreferredVelocity>(a).unwrap().0;
        assert!(
            (new_pv - preferred).length() < f32::EPSILON,
            "Opposing team should not trigger separation, got {new_pv:?}"
        );
    }
}

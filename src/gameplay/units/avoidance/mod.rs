//! ORCA local avoidance for unit-to-unit collision prevention.

pub mod orca;
pub mod spatial_hash;

use std::collections::HashMap;

use avian2d::prelude::*;
use bevy::prelude::*;

use self::orca::AgentSnapshot;
use self::spatial_hash::SpatialHash;
use super::{Movement, UNIT_RADIUS, Unit};

// === Constants ===

/// Default ORCA time horizon in seconds.
const DEFAULT_TIME_HORIZON: f32 = 3.0;
/// Maximum neighbors to consider per agent.
const DEFAULT_MAX_NEIGHBORS: u32 = 10;
/// Velocity smoothing blend factor (0.0 = keep old, 1.0 = fully ORCA).
const DEFAULT_VELOCITY_SMOOTHING: f32 = 0.85;

// === Components ===

/// The velocity the unit wants to move at (from pathfinding/movement logic).
/// Written by `unit_movement`, read by `compute_avoidance`.
#[derive(Component, Debug, Clone, Copy, Reflect, Default)]
#[reflect(Component)]
pub struct PreferredVelocity(pub Vec2);

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
            radius: UNIT_RADIUS,
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
}

impl Default for AvoidanceConfig {
    fn default() -> Self {
        Self {
            time_horizon: DEFAULT_TIME_HORIZON,
            neighbor_distance: DEFAULT_TIME_HORIZON * 50.0, // max_speed * time_horizon
            max_neighbors: DEFAULT_MAX_NEIGHBORS,
            velocity_smoothing: DEFAULT_VELOCITY_SMOOTHING,
        }
    }
}

// === Systems ===

/// Rebuild the spatial hash with all unit positions. Runs every frame.
pub fn rebuild_spatial_hash(
    mut hash: ResMut<SpatialHash>,
    agents: Query<(Entity, &GlobalTransform), With<Unit>>,
) {
    hash.clear();
    for (entity, transform) in &agents {
        hash.insert(entity, transform.translation().xy());
    }
}

/// Compute ORCA-adjusted velocities for all units.
///
/// Reads `PreferredVelocity` (desired direction from pathfinding) and
/// `LinearVelocity` (current velocity from last frame's ORCA output).
/// Writes the ORCA result to `LinearVelocity`.
pub fn compute_avoidance(
    config: Res<AvoidanceConfig>,
    hash: Res<SpatialHash>,
    mut agents: Query<
        (
            Entity,
            &GlobalTransform,
            &mut LinearVelocity,
            &PreferredVelocity,
            &AvoidanceAgent,
            &Movement,
        ),
        With<Unit>,
    >,
) {
    // Phase 1: Snapshot all agent data (immutable read via .iter())
    let snapshots: Vec<(Entity, AgentSnapshot)> = agents
        .iter()
        .map(
            |(entity, transform, velocity, preferred, avoidance, movement)| {
                (
                    entity,
                    AgentSnapshot {
                        position: transform.translation().xy(),
                        velocity: velocity.0,
                        preferred: preferred.0,
                        radius: avoidance.radius,
                        max_speed: movement.speed,
                        responsibility: avoidance.responsibility,
                    },
                )
            },
        )
        .collect();

    // Build entity -> snapshot index lookup for neighbor access
    let index_map: HashMap<Entity, usize> = snapshots
        .iter()
        .enumerate()
        .map(|(i, (e, _))| (*e, i))
        .collect();

    // Phase 2: Compute ORCA velocity for each agent
    let results: Vec<(Entity, Vec2)> = snapshots
        .iter()
        .map(|(entity, agent)| {
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
                    let neighbor = &snapshots[idx].1;
                    if let Some(line) =
                        orca::compute_orca_line(agent, neighbor, config.time_horizon)
                    {
                        lines.push(line);
                        neighbor_count += 1;
                    }
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
        if let Ok((_, _, mut linear_vel, _, _, _)) = agents.get_mut(entity) {
            linear_vel.0 = new_velocity;
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
        app.insert_resource(SpatialHash::new(
            AvoidanceConfig::default().neighbor_distance,
        ));
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
                LinearVelocity(current_vel),
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
        let vel = app.world().get::<LinearVelocity>(unit).unwrap();
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
        let vel_a = app.world().get::<LinearVelocity>(a).unwrap();
        let vel_b = app.world().get::<LinearVelocity>(b).unwrap();
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
        let vel = app.world().get::<LinearVelocity>(unit).unwrap();
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
        let vel = app.world().get::<LinearVelocity>(a).unwrap();
        assert!(
            (vel.0 - Vec2::new(50.0, 0.0)).length() < 1.0,
            "Distant agents should not affect each other, got {:?}",
            vel.0
        );
    }
}

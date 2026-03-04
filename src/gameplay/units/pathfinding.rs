//! `NavMesh` pathfinding for units — computes waypoint paths around obstacles.

use bevy::prelude::*;
use vleue_navigator::prelude::*;

use super::Unit;
use crate::gameplay::CurrentTarget;

/// Seconds between periodic path recomputations for units that already have a path.
/// Picks up navmesh changes from building placement/destruction.
const PATH_REFRESH_INTERVAL_SECS: f32 = 0.5;

/// Step size in pixels when searching for a navigable point near an off-mesh target.
const SNAP_STEP_SIZE: f32 = 8.0;

/// Maximum search distance = `SNAP_STEP_SIZE` * `SNAP_MAX_STEPS` = 160px.
/// Covers the largest obstacle (fortress: 64px half-width + 6px `agent_radius` = 70px).
const SNAP_MAX_STEPS: u32 = 20;

/// Timer controlling periodic path refresh for all units.
/// Exposed as a resource so tests can manipulate it.
#[derive(Resource, Debug, Reflect)]
#[reflect(Resource)]
pub struct PathRefreshTimer(pub Timer);

impl Default for PathRefreshTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            PATH_REFRESH_INTERVAL_SECS,
            TimerMode::Repeating,
        ))
    }
}

/// Waypoint path for a unit, computed from the `NavMesh`.
/// When present and non-empty, the movement system follows waypoints
/// instead of heading straight for the target.
#[derive(Component, Debug, Clone, Reflect, Default)]
#[reflect(Component)]
pub struct NavPath {
    /// World-space waypoints from navmesh pathfinding.
    pub waypoints: Vec<Vec2>,
    /// Index of the next waypoint to steer toward.
    pub current_index: usize,
    /// The target entity this path was computed for.
    /// Used to detect when the target changes.
    target: Option<Entity>,
}

impl NavPath {
    /// Replace the path with new waypoints for a new target.
    pub fn set(&mut self, waypoints: Vec<Vec2>, target: Option<Entity>) {
        self.waypoints = waypoints;
        self.current_index = 0;
        self.target = target;
    }

    /// Clear the path (no waypoints).
    pub fn clear(&mut self) {
        self.waypoints.clear();
        self.current_index = 0;
        self.target = None;
    }

    /// Get the current waypoint, if any remain.
    #[must_use]
    pub fn current_waypoint(&self) -> Option<Vec2> {
        self.waypoints.get(self.current_index).copied()
    }

    /// Advance to the next waypoint. Returns true if there are more waypoints.
    pub fn advance(&mut self) -> bool {
        self.current_index += 1;
        self.current_index < self.waypoints.len()
    }

    /// Whether a non-empty path has been fully consumed (all waypoints visited).
    /// Used to trigger immediate re-pathing when the unit hasn't reached its target yet.
    #[must_use]
    pub fn is_path_consumed(&self) -> bool {
        !self.waypoints.is_empty() && self.current_index >= self.waypoints.len()
    }

    /// Whether this path needs recomputation for the given target.
    #[must_use]
    pub fn needs_recompute(&self, target: Option<Entity>) -> bool {
        self.target != target
    }
}

/// Find the nearest navigable point to `target` by walking toward `from`.
///
/// Returns `target` unchanged if it's already on the mesh.
/// When the target is inside a carved obstacle (e.g., a fortress or building with
/// `NavObstacle`), steps along the direction from `target` toward `from` until
/// an on-mesh point is found.
///
/// Returns `None` if no navigable point is found within the search distance.
fn snap_to_mesh(navmesh: &NavMesh, target: Vec2, from: Vec2) -> Option<Vec2> {
    if navmesh.is_in_mesh(target) {
        return Some(target);
    }

    let dir = (from - target).normalize_or_zero();
    if dir == Vec2::ZERO {
        return None;
    }

    #[allow(clippy::cast_precision_loss)] // step is at most 20
    for step in 1..=SNAP_MAX_STEPS {
        let candidate = target + dir * (SNAP_STEP_SIZE * step as f32);
        if navmesh.is_in_mesh(candidate) {
            return Some(candidate);
        }
    }

    None
}

/// Computes navmesh paths for units whose target changed or whose path needs refreshing.
/// Runs in `GameSet::Ai` after `find_target`.
pub(super) fn compute_paths(
    time: Res<Time>,
    mut refresh_timer: ResMut<PathRefreshTimer>,
    mut units: Query<(&CurrentTarget, &GlobalTransform, &mut NavPath), With<Unit>>,
    targets: Query<&GlobalTransform>,
    navmeshes: Option<Res<Assets<NavMesh>>>,
    navmesh_query: Option<Single<(&ManagedNavMesh, &NavMeshStatus)>>,
) {
    let Some(navmeshes) = navmeshes else {
        return;
    };
    let Some(inner) = navmesh_query else {
        return;
    };
    let (managed, status) = *inner;
    if *status != NavMeshStatus::Built {
        return;
    }
    let Some(navmesh) = navmeshes.get(managed) else {
        return;
    };

    refresh_timer.0.tick(time.delta());
    let refresh_due = refresh_timer.0.just_finished();

    for (current_target, transform, mut nav_path) in &mut units {
        let target_changed = nav_path.needs_recompute(current_target.0);

        // Recompute if: target changed, periodic refresh due, or path fully consumed
        let path_consumed = nav_path.is_path_consumed();
        if !target_changed && !refresh_due && !path_consumed {
            continue;
        }

        let Some(target_entity) = current_target.0 else {
            nav_path.clear();
            continue;
        };

        let Ok(target_transform) = targets.get(target_entity) else {
            nav_path.clear();
            continue;
        };

        let from = transform.translation().xy();
        let to = target_transform.translation().xy();

        // Snap off-mesh destinations to nearest navigable point. Targets like
        // fortresses and buildings are NavObstacles — their centers are carved
        // out of the navmesh. Walking toward the unit finds the obstacle's
        // nearest mesh edge on the correct approach side.
        let destination = snap_to_mesh(navmesh, to, from).unwrap_or(to);

        if let Some(path) = navmesh.path(from, destination) {
            nav_path.set(path.path, current_target.0);
        } else {
            // No valid path — store empty waypoints, unit stops until next refresh
            nav_path.set(Vec::new(), current_target.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use polyanya::Trimesh;

    /// Build a rectangular navmesh covering (0,0) to (200,200) using two triangles.
    fn build_test_navmesh() -> NavMesh {
        let mesh: polyanya::Mesh = Trimesh {
            vertices: vec![
                Vec2::new(0.0, 0.0),
                Vec2::new(200.0, 0.0),
                Vec2::new(200.0, 200.0),
                Vec2::new(0.0, 200.0),
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
        }
        .try_into()
        .expect("valid trimesh");
        NavMesh::from_polyanya_mesh(mesh)
    }

    #[test]
    fn nav_path_default_is_empty() {
        let path = NavPath::default();
        assert!(path.waypoints.is_empty());
        assert_eq!(path.current_index, 0);
        assert!(path.target.is_none());
        assert!(path.current_waypoint().is_none());
    }

    #[test]
    fn nav_path_set_replaces_waypoints() {
        let mut path = NavPath::default();
        let entity = Entity::from_bits(42);
        path.set(vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 4.0)], Some(entity));

        assert_eq!(path.waypoints.len(), 2);
        assert_eq!(path.current_index, 0);
        assert_eq!(path.target, Some(entity));
        assert_eq!(path.current_waypoint(), Some(Vec2::new(1.0, 2.0)));
    }

    #[test]
    fn nav_path_advance_increments_index() {
        let mut path = NavPath::default();
        path.set(vec![Vec2::new(1.0, 2.0), Vec2::new(3.0, 4.0)], None);

        assert!(path.advance()); // Advance to index 1
        assert_eq!(path.current_waypoint(), Some(Vec2::new(3.0, 4.0)));

        assert!(!path.advance()); // No more waypoints
        assert!(path.current_waypoint().is_none());
    }

    #[test]
    fn nav_path_clear_resets_everything() {
        let mut path = NavPath::default();
        let entity = Entity::from_bits(42);
        path.set(vec![Vec2::new(1.0, 2.0)], Some(entity));
        path.clear();

        assert!(path.waypoints.is_empty());
        assert_eq!(path.current_index, 0);
        assert!(path.target.is_none());
    }

    #[test]
    fn nav_path_needs_recompute_detects_target_change() {
        let mut path = NavPath::default();
        let entity_a = Entity::from_bits(42);
        let entity_b = Entity::from_bits(99);

        path.set(vec![Vec2::ZERO], Some(entity_a));

        assert!(!path.needs_recompute(Some(entity_a))); // Same target
        assert!(path.needs_recompute(Some(entity_b))); // Different target
        assert!(path.needs_recompute(None)); // No target
    }

    #[test]
    fn nav_path_is_path_consumed() {
        let mut path = NavPath::default();

        // Empty path is not "consumed" (was never set)
        assert!(!path.is_path_consumed());

        // Set a path with one waypoint
        path.set(vec![Vec2::new(1.0, 2.0)], None);
        assert!(!path.is_path_consumed()); // At index 0, not consumed

        // Advance past the last waypoint
        path.advance();
        assert!(path.is_path_consumed()); // All consumed

        // Clear resets
        path.clear();
        assert!(!path.is_path_consumed());
    }

    #[test]
    fn snap_to_mesh_returns_target_when_on_mesh() {
        let navmesh = build_test_navmesh();
        let target = Vec2::new(100.0, 100.0);
        let from = Vec2::new(150.0, 100.0);

        let result = snap_to_mesh(&navmesh, target, from);
        assert_eq!(result, Some(target));
    }

    #[test]
    fn snap_to_mesh_finds_nearest_navigable_point() {
        let navmesh = build_test_navmesh();
        // Target off-mesh to the left, unit inside the mesh
        let target = Vec2::new(-50.0, 100.0);
        let from = Vec2::new(150.0, 100.0);

        let result = snap_to_mesh(&navmesh, target, from);
        assert!(result.is_some(), "Should find an on-mesh point");
        let snapped = result.unwrap();
        assert!(
            navmesh.is_in_mesh(snapped),
            "Snapped point should be on mesh"
        );
        assert!(
            snapped.x > target.x,
            "Snapped point should be closer to mesh than target"
        );
    }

    #[test]
    fn snap_to_mesh_returns_none_when_unreachable() {
        let navmesh = build_test_navmesh();
        // Both points far off-mesh, beyond search distance
        let target = Vec2::new(-500.0, 100.0);
        let from = Vec2::new(-400.0, 100.0);

        let result = snap_to_mesh(&navmesh, target, from);
        assert!(result.is_none());
    }

    #[test]
    fn snap_to_mesh_returns_none_for_coincident_points() {
        let navmesh = build_test_navmesh();
        // from == target produces zero direction
        let point = Vec2::new(-50.0, 100.0);

        let result = snap_to_mesh(&navmesh, point, point);
        assert!(result.is_none());
    }
}

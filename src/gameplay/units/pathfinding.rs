//! `NavMesh` pathfinding for units — computes waypoint paths around obstacles.

use bevy::prelude::*;
use vleue_navigator::prelude::*;

use super::Unit;
use crate::gameplay::CurrentTarget;

/// Seconds between periodic path recomputations for units that already have a path.
/// Picks up navmesh changes from building placement/destruction.
const PATH_REFRESH_INTERVAL_SECS: f32 = 0.5;

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

    /// Whether this path needs recomputation for the given target.
    #[must_use]
    pub fn needs_recompute(&self, target: Option<Entity>) -> bool {
        self.target != target
    }
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

        // Skip recomputation if target hasn't changed and no periodic refresh
        if !target_changed && !refresh_due {
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

        if let Some(path) = navmesh.path(from, to) {
            nav_path.set(path.path, current_target.0);
        } else {
            // No valid path — clear waypoints, movement falls back to direct
            nav_path.set(Vec::new(), current_target.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

//! `vleue_navigator` navmesh configuration for pathfinding.

use avian2d::prelude::*;
use bevy::prelude::*;
use vleue_navigator::prelude::*;

use crate::screens::GameState;

/// Marker: this entity's `Collider` is a navmesh obstacle.
/// Add to buildings, fortresses — anything units must path around.
/// Do NOT add to units (dynamic), projectiles (kinematic), or non-blocking entities.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct NavObstacle;

/// Strip `NavObstacle` markers before `DespawnOnExit` batch-despawns entities.
/// Prevents `NavmeshUpdaterPlugin` from detecting obstacle removals during
/// shutdown and triggering late async navmesh rebuild tasks.
fn strip_nav_obstacles_before_despawn(
    mut commands: Commands,
    obstacles: Query<Entity, With<NavObstacle>>,
) {
    for entity in &obstacles {
        commands.entity(entity).remove::<NavObstacle>();
    }
}

pub(super) fn plugin(app: &mut App) {
    app.register_type::<NavObstacle>();
    app.add_plugins((
        VleueNavigatorPlugin,
        NavmeshUpdaterPlugin::<Collider, NavObstacle>::default(),
    ));

    // Strip NavObstacle markers before DespawnOnExit to prevent late navmesh rebuilds.
    app.add_systems(
        OnExit(GameState::InGame),
        strip_nav_obstacles_before_despawn,
    );
}

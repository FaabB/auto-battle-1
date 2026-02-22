//! `vleue_navigator` navmesh configuration for pathfinding.

use avian2d::prelude::*;
use bevy::prelude::*;
use vleue_navigator::prelude::*;

/// Marker: this entity's `Collider` is a navmesh obstacle.
/// Add to buildings, fortresses — anything units must path around.
/// Do NOT add to units (dynamic), projectiles (kinematic), or non-blocking entities.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct NavObstacle;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<NavObstacle>();
    app.add_plugins((
        VleueNavigatorPlugin,
        NavmeshUpdaterPlugin::<Collider, NavObstacle>::default(),
    ));
}

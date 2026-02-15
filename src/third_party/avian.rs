//! Avian2d physics configuration for top-down gameplay.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::gameplay::battlefield::CELL_SIZE;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(PhysicsPlugins::default().with_length_unit(CELL_SIZE));
    app.insert_resource(Gravity::ZERO);
    // PhysicsDebugPlugin (wireframe colliders) requires the render pipeline.
    // Add `app.add_plugins(avian2d::prelude::PhysicsDebugPlugin)` in main.rs
    // when you need visual debugging.
}

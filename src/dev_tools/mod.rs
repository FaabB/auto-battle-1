//! Development tools — only included with `cargo run --features dev`.
//!
//! Debug overlays, inspector setup, and diagnostic tools go here.
//! This module is stripped from release builds.

use bevy::prelude::*;
use vleue_navigator::prelude::NavMeshesDebug;

use crate::gameplay::units::Unit;
use crate::gameplay::units::pathfinding::NavPath;

/// Marker resource: when present, the world inspector is shown.
#[derive(Resource)]
struct ShowWorldInspector;

pub fn plugin(app: &mut App) {
    // Inspector requires EguiPlugin which needs the render backend.
    // Skip in headless test apps that use MinimalPlugins.
    if app.is_plugin_added::<bevy::render::RenderPlugin>() {
        app.add_plugins(bevy_inspector_egui::bevy_egui::EguiPlugin::default());
        app.add_plugins(
            bevy_inspector_egui::quick::WorldInspectorPlugin::default()
                .run_if(resource_exists::<ShowWorldInspector>),
        );
        app.add_systems(Update, toggle_world_inspector);
    }

    // Navmesh + path debug overlays start OFF. Press F3 to toggle.
    app.add_systems(Update, toggle_navmesh_debug);
    app.add_systems(
        Update,
        debug_draw_unit_paths
            .run_if(crate::gameplay_running.and(resource_exists::<NavMeshesDebug>)),
    );
}

/// Toggle world inspector with F4.
fn toggle_world_inspector(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    existing: Option<Res<ShowWorldInspector>>,
) {
    if input.just_pressed(KeyCode::F4) {
        if existing.is_some() {
            commands.remove_resource::<ShowWorldInspector>();
        } else {
            commands.insert_resource(ShowWorldInspector);
        }
    }
}

/// Toggle navmesh debug overlay and path gizmos with F3.
fn toggle_navmesh_debug(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    existing: Option<Res<NavMeshesDebug>>,
) {
    if input.just_pressed(KeyCode::F3) {
        if existing.is_some() {
            commands.remove_resource::<NavMeshesDebug>();
        } else {
            commands.insert_resource(NavMeshesDebug(Color::srgba(1.0, 0.0, 0.0, 0.15)));
        }
    }
}

/// Draw yellow lines showing each unit's remaining navmesh path.
fn debug_draw_unit_paths(
    units: Query<(&GlobalTransform, &NavPath), With<Unit>>,
    mut gizmos: Gizmos,
) {
    for (transform, nav_path) in &units {
        if nav_path.waypoints.is_empty() {
            continue;
        }
        let mut points = vec![transform.translation().xy()];
        for &wp in &nav_path.waypoints[nav_path.current_index..] {
            points.push(wp);
        }
        if points.len() >= 2 {
            gizmos.linestrip_2d(points, Color::srgb(1.0, 1.0, 0.0));
        }
    }
}

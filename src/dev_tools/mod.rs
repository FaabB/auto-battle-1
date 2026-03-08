//! Development tools — only included with `cargo run --features dev`.
//!
//! Debug overlays, inspector setup, and diagnostic tools go here.
//! This module is stripped from release builds.

use bevy::prelude::*;

use avian2d::prelude::LinearVelocity;

use crate::gameplay::Team;
use crate::gameplay::flow_field::GoalRegistry;
use crate::gameplay::units::Unit;
use crate::gameplay::units::avoidance::PreferredVelocity;

/// Marker resource: when present, the world inspector is shown.
#[derive(Resource)]
struct ShowWorldInspector;

/// Marker resource: when present, flow field debug arrows are drawn.
#[derive(Resource, Debug)]
struct FlowFieldDebug {
    /// Which team's flow field to display.
    show_team: Team,
}

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

    // Flow field + avoidance debug overlays. Press F3 to toggle flow field.
    app.add_systems(Update, toggle_flow_field_debug);
    app.add_systems(
        Update,
        (debug_draw_flow_field, debug_draw_avoidance)
            .run_if(crate::gameplay_running.and(resource_exists::<FlowFieldDebug>)),
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

/// Toggle flow field debug overlay with F3. Cycles: player → enemy → off.
fn toggle_flow_field_debug(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    existing: Option<Res<FlowFieldDebug>>,
) {
    if input.just_pressed(KeyCode::F3) {
        if let Some(debug) = existing {
            match debug.show_team {
                Team::Player => {
                    commands.insert_resource(FlowFieldDebug {
                        show_team: Team::Enemy,
                    });
                }
                Team::Enemy => {
                    commands.remove_resource::<FlowFieldDebug>();
                }
            }
        } else {
            commands.insert_resource(FlowFieldDebug {
                show_team: Team::Player,
            });
        }
    }
}

/// Draw flow field direction arrows.
#[allow(clippy::cast_precision_loss)]
fn debug_draw_flow_field(
    debug: Res<FlowFieldDebug>,
    registry: Option<Res<GoalRegistry>>,
    mut gizmos: Gizmos,
) {
    let Some(registry) = registry else { return };

    let flow_field = match debug.show_team {
        // Player units go toward enemy fortress
        Team::Player => &registry.enemy_fortress.flow_field,
        // Enemy units go toward player fortress
        Team::Enemy => &registry.player_fortress.flow_field,
    };

    let color = match debug.show_team {
        Team::Player => Color::srgba(0.0, 0.5, 1.0, 0.6),
        Team::Enemy => Color::srgba(1.0, 0.3, 0.3, 0.6),
    };

    // Draw an arrow for every cell
    for row in 0..flow_field.rows {
        for col in 0..flow_field.cols {
            let idx = flow_field.index(col, row);
            let dir = flow_field.directions[idx];
            if dir == Vec2::ZERO {
                continue;
            }
            let center = Vec2::new(
                (col as f32 + 0.5) * flow_field.cell_size,
                (row as f32 + 0.5) * flow_field.cell_size,
            );
            gizmos.arrow_2d(center, center + dir * 20.0, color);
        }
    }
}

/// Draw ORCA debug visualization: green = preferred velocity, cyan = actual (ORCA-adjusted).
fn debug_draw_avoidance(
    units: Query<(&GlobalTransform, &LinearVelocity, &PreferredVelocity), With<Unit>>,
    mut gizmos: Gizmos,
) {
    let scale = 0.5; // Scale arrows to be visible but not overwhelming
    for (transform, velocity, preferred) in &units {
        let pos = transform.translation().xy();

        // Green arrow: preferred velocity (where flow field wants to go)
        if preferred.0.length_squared() > f32::EPSILON {
            gizmos.arrow_2d(pos, pos + preferred.0 * scale, Color::srgb(0.0, 1.0, 0.0));
        }

        // Cyan arrow: actual velocity (ORCA-adjusted)
        if velocity.0.length_squared() > f32::EPSILON {
            gizmos.arrow_2d(pos, pos + velocity.0 * scale, Color::srgb(0.0, 1.0, 1.0));
        }
    }
}

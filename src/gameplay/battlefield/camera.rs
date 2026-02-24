//! Camera setup and panning for the battlefield.

use bevy::camera::ScalingMode;
use bevy::prelude::*;

use super::{BATTLEFIELD_HEIGHT, BATTLEFIELD_WIDTH, BuildZone};
use crate::gameplay::hud::bottom_bar::BOTTOM_BAR_HEIGHT;

/// Camera panning speed in pixels per second.
const CAMERA_PAN_SPEED: f32 = 500.0;

/// Computes how many world units the bottom bar covers at the current window size.
fn bar_world_height(window_height: f32) -> f32 {
    BOTTOM_BAR_HEIGHT / window_height * BATTLEFIELD_HEIGHT
}

pub(super) fn setup_camera_for_battlefield(
    mut camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
    build_zone: Single<&Transform, (With<BuildZone>, Without<Camera2d>)>,
    windows: Single<&Window>,
) {
    let (transform, projection) = &mut *camera;

    // Position camera so the visible area above the bar is centered on the battlefield.
    let bar_world = bar_world_height(windows.height());
    transform.translation.x = build_zone.translation.x;
    transform.translation.y = BATTLEFIELD_HEIGHT / 2.0 - bar_world / 2.0;

    // Set projection scaling so the full battlefield height fits the window.
    if let Projection::Orthographic(ref mut ortho) = **projection {
        ortho.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: BATTLEFIELD_HEIGHT,
        };
    }
}

pub(super) fn camera_pan(
    keyboard: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut camera: Single<&mut Transform, With<Camera2d>>,
    windows: Single<&Window>,
) {
    // X-axis panning
    let mut x_direction = 0.0;
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        x_direction += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        x_direction -= 1.0;
    }
    camera.translation.x += x_direction * CAMERA_PAN_SPEED * time.delta_secs();

    // Y-axis panning
    let mut y_direction = 0.0;
    if keyboard.pressed(KeyCode::KeyW) || keyboard.pressed(KeyCode::ArrowUp) {
        y_direction += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || keyboard.pressed(KeyCode::ArrowDown) {
        y_direction -= 1.0;
    }
    camera.translation.y += y_direction * CAMERA_PAN_SPEED * time.delta_secs();

    // X clamping: FixedVertical(BATTLEFIELD_HEIGHT) visible width depends on aspect ratio.
    let aspect_ratio = windows.width() / windows.height();
    let visible_width = BATTLEFIELD_HEIGHT * aspect_ratio;
    let half_visible_x = visible_width / 2.0;
    let min_x = half_visible_x;
    let max_x = BATTLEFIELD_WIDTH - half_visible_x;
    camera.translation.x = camera.translation.x.clamp(min_x, max_x);

    // Y clamping: allow panning down so the bottom of the battlefield is visible
    // above the opaque bottom bar.
    let half_visible_y = BATTLEFIELD_HEIGHT / 2.0;
    let bar_world = bar_world_height(windows.height());
    let min_y = half_visible_y - bar_world; // Pan down: bottom of battlefield above bar
    let max_y = half_visible_y; // Pan up: top of battlefield at top of window
    camera.translation.y = camera.translation.y.clamp(min_y, max_y);
}

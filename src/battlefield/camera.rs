//! Camera setup and panning for the battlefield.

use bevy::camera::ScalingMode;
use bevy::prelude::*;

use super::{BATTLEFIELD_HEIGHT, BATTLEFIELD_WIDTH, BuildZone};

/// Camera panning speed in pixels per second.
const CAMERA_PAN_SPEED: f32 = 500.0;

pub(super) fn setup_camera_for_battlefield(
    mut camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
    build_zone: Single<&Transform, (With<BuildZone>, Without<Camera2d>)>,
) {
    let (transform, projection) = &mut *camera;

    // Position camera centered on the building zone (X and Y) by reading the zone entity
    transform.translation.x = build_zone.translation.x;
    transform.translation.y = build_zone.translation.y;

    // Set projection scaling so the full battlefield height fits the viewport
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
    let mut direction = 0.0;
    if keyboard.pressed(KeyCode::KeyD) || keyboard.pressed(KeyCode::ArrowRight) {
        direction += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyA) || keyboard.pressed(KeyCode::ArrowLeft) {
        direction -= 1.0;
    }

    // Always apply -- multiplying by 0.0 direction is a no-op.
    // Avoids `clippy::float_cmp` from `direction != 0.0` under pedantic lints.
    camera.translation.x += direction * CAMERA_PAN_SPEED * time.delta_secs();

    // Clamp camera to battlefield bounds.
    // FixedVertical scaling: visible width depends on window aspect ratio.
    let aspect_ratio = windows.width() / windows.height();
    let visible_width = BATTLEFIELD_HEIGHT * aspect_ratio;
    let half_visible = visible_width / 2.0;

    let min_x = half_visible; // Can't see past left edge (x=0)
    let max_x = BATTLEFIELD_WIDTH - half_visible; // Can't see past right edge

    camera.translation.x = camera.translation.x.clamp(min_x, max_x);
}

//! Reusable UI widget constructors.
//!
//! Each function returns an `impl Bundle` containing the widget's styling.
//! Callers add layout (`Node`) and lifecycle (`DespawnOnExit`) separately.

use bevy::prelude::*;

use super::palette;

/// Large header text (64px, white).
pub fn header(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: 64.0,
            ..default()
        },
        TextColor(palette::HEADER_TEXT),
    )
}

/// Medium label text (32px, gray).
pub fn label(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: 32.0,
            ..default()
        },
        TextColor(palette::BODY_TEXT),
    )
}

/// Full-screen semi-transparent overlay.
pub fn overlay() -> impl Bundle {
    (
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            position_type: PositionType::Absolute,
            ..default()
        },
        BackgroundColor(palette::OVERLAY_BACKGROUND),
    )
}

//! Color constants for consistent UI theming.

use bevy::prelude::*;

/// Header/title text color (white).
pub const HEADER_TEXT: Color = Color::WHITE;

/// Body/subtitle text color (light gray).
pub const BODY_TEXT: Color = Color::srgb(0.7, 0.7, 0.7);

/// Semi-transparent dark overlay for pause/modal screens.
pub const OVERLAY_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);

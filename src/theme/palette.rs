//! Color constants and font size tokens for consistent UI theming.

#![allow(dead_code)] // Constants populated ahead of use across multiple phases.

use bevy::prelude::*;

// === Text Colors ===

/// Header/title text color (white).
pub const HEADER_TEXT: Color = Color::WHITE;

/// Body/subtitle text color (light gray).
pub const BODY_TEXT: Color = Color::srgb(0.7, 0.7, 0.7);

/// Gold/currency display text color (yellow-gold).
pub const GOLD_TEXT: Color = Color::srgb(1.0, 0.85, 0.0);

/// Button label text color.
pub const BUTTON_TEXT: Color = Color::srgb(0.925, 0.925, 0.925);

// === UI Backgrounds ===

/// Semi-transparent dark overlay for pause/modal screens.
pub const OVERLAY_BACKGROUND: Color = Color::srgba(0.0, 0.0, 0.0, 0.7);

/// Panel background (dark blue-gray, nearly opaque).
pub const PANEL_BACKGROUND: Color = Color::srgba(0.1, 0.1, 0.15, 0.95);

/// Panel border (light blue-gray, semi-transparent).
pub const PANEL_BORDER: Color = Color::srgba(0.5, 0.5, 0.6, 0.8);

// === Button Colors ===

pub const BUTTON_BACKGROUND: Color = Color::srgb(0.275, 0.4, 0.75);
pub const BUTTON_HOVERED_BACKGROUND: Color = Color::srgb(0.384, 0.6, 0.82);
pub const BUTTON_PRESSED_BACKGROUND: Color = Color::srgb(0.239, 0.286, 0.6);

// === Bottom Bar ===

pub const BOTTOM_BAR_BACKGROUND: Color = Color::srgb(0.1, 0.1, 0.15);

// === Shop Card Colors ===

pub const CARD_BACKGROUND: Color = Color::srgb(0.2, 0.2, 0.3);
pub const CARD_SELECTED: Color = Color::srgb(0.3, 0.5, 0.3);
pub const CARD_EMPTY: Color = Color::srgb(0.15, 0.15, 0.15);
pub const CARD_HOVER: Color = Color::srgb(0.3, 0.3, 0.4);
pub const REROLL_BACKGROUND: Color = Color::srgb(0.4, 0.25, 0.1);

// === Battlefield Colors ===

pub const GRID_CELL: Color = Color::srgb(0.3, 0.3, 0.4);
pub const GRID_CURSOR: Color = Color::srgba(1.0, 1.0, 1.0, 0.2);
pub const PLAYER_FORTRESS: Color = Color::srgb(0.2, 0.3, 0.8);
pub const ENEMY_FORTRESS: Color = Color::srgb(0.8, 0.2, 0.2);
pub const BUILD_ZONE: Color = Color::srgb(0.25, 0.25, 0.35);
pub const COMBAT_ZONE: Color = Color::srgb(0.15, 0.15, 0.2);
pub const BACKGROUND: Color = Color::srgb(0.1, 0.1, 0.12);

// === Entity Colors ===

pub const PLAYER_UNIT: Color = Color::srgb(0.2, 0.8, 0.2);
pub const ENEMY_UNIT: Color = Color::srgb(0.8, 0.2, 0.2);
pub const PROJECTILE: Color = Color::srgb(1.0, 1.0, 0.3);
pub const BARRACKS: Color = Color::srgb(0.15, 0.2, 0.6);
pub const FARM: Color = Color::srgb(0.2, 0.6, 0.1);

// === Health/Progress Bar Colors ===

pub const HEALTH_BAR_BG: Color = Color::srgb(0.8, 0.1, 0.1);
pub const HEALTH_BAR_FILL: Color = Color::srgb(0.1, 0.9, 0.1);
pub const PRODUCTION_BAR_BG: Color = Color::srgb(0.2, 0.2, 0.4);
pub const PRODUCTION_BAR_FILL: Color = Color::srgb(0.3, 0.5, 0.9);

// === Font Size Tokens ===

pub const FONT_SIZE_TITLE: f32 = 72.0;
pub const FONT_SIZE_HEADER: f32 = 64.0;
pub const FONT_SIZE_LABEL: f32 = 32.0;
pub const FONT_SIZE_HUD: f32 = 28.0;
pub const FONT_SIZE_PROMPT: f32 = 24.0;
pub const FONT_SIZE_BODY: f32 = 16.0;
pub const FONT_SIZE_SMALL: f32 = 14.0;

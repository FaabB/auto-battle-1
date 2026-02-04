//! Cleanup marker components for state-scoped entity management.

use bevy::prelude::*;

/// Marker for entities that should be cleaned up when leaving the loading state.
#[derive(Component)]
pub struct CleanupLoading;

/// Marker for entities that should be cleaned up when leaving the main menu.
#[derive(Component)]
pub struct CleanupMainMenu;

/// Marker for entities that should be cleaned up when leaving the game.
#[derive(Component)]
pub struct CleanupInGame;

/// Marker for entities that should be cleaned up when leaving pause.
#[derive(Component)]
pub struct CleanupPaused;

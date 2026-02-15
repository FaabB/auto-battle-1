//! Health bar rendering: spawns and updates visual health indicators.

use bevy::prelude::*;

use crate::gameplay::Health;
use crate::{GameSet, gameplay_running};

// === Constants ===

/// Health bar colors.
const HEALTH_BAR_BG_COLOR: Color = Color::srgb(0.8, 0.1, 0.1);
const HEALTH_BAR_FILL_COLOR: Color = Color::srgb(0.1, 0.9, 0.1);

/// Default health bar width for units (pixels).
pub const UNIT_HEALTH_BAR_WIDTH: f32 = 20.0;

/// Default health bar height for units (pixels).
pub const UNIT_HEALTH_BAR_HEIGHT: f32 = 3.0;

/// Default health bar Y offset for units (pixels above center).
pub const UNIT_HEALTH_BAR_Y_OFFSET: f32 = 18.0;

// === Components ===

/// Marker: red background bar (full width, shows "missing" HP).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct HealthBarBackground;

/// Marker: green foreground bar (scales with current/max HP).
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct HealthBarFill;

/// Configuration for health bar sizing. Required on all entities with `Health`.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct HealthBarConfig {
    pub width: f32,
    pub height: f32,
    pub y_offset: f32,
}

// === Systems ===

/// Spawns health bar child entities when `Health` is added to an entity with `HealthBarConfig`.
fn spawn_health_bars(
    add: On<Add, Health>,
    configs: Query<&HealthBarConfig>,
    mut commands: Commands,
) {
    let Ok(config) = configs.get(add.entity) else {
        return; // Entity has Health but no HealthBarConfig (shouldn't happen, but safe)
    };
    commands.entity(add.entity).with_children(|parent| {
        // Red background (full width, always visible)
        parent.spawn((
            Name::new("Health Bar BG"),
            Sprite::from_color(HEALTH_BAR_BG_COLOR, Vec2::new(config.width, config.height)),
            Transform::from_xyz(0.0, config.y_offset, 1.0),
            HealthBarBackground,
        ));
        // Green fill (scales with HP ratio, rendered in front of background)
        parent.spawn((
            Name::new("Health Bar Fill"),
            Sprite::from_color(
                HEALTH_BAR_FILL_COLOR,
                Vec2::new(config.width, config.height),
            ),
            Transform::from_xyz(0.0, config.y_offset, 1.1),
            HealthBarFill,
        ));
    });
}

/// Updates health bar fill width based on current/max HP.
/// Runs in `GameSet::Ui`.
fn update_health_bars(
    health_query: Query<(&Health, &Children, &HealthBarConfig)>,
    mut bar_query: Query<&mut Transform, With<HealthBarFill>>,
) {
    for (health, children, config) in &health_query {
        let ratio = (health.current / health.max).clamp(0.0, 1.0);
        for child in children.iter() {
            if let Ok(mut transform) = bar_query.get_mut(child) {
                transform.scale.x = ratio;
                // Shift left to keep bar left-aligned as it shrinks
                transform.translation.x = config.width.mul_add(-(1.0 - ratio), 0.0) / 2.0;
            }
        }
    }
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<HealthBarBackground>()
        .register_type::<HealthBarFill>()
        .register_type::<HealthBarConfig>();

    // Observer: spawn health bars immediately when Health is added
    app.add_observer(spawn_health_bars);

    // System: update health bar fill each frame (no longer needs chain)
    app.add_systems(
        Update,
        update_health_bars
            .in_set(GameSet::Ui)
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn constants_are_valid() {
        assert!(UNIT_HEALTH_BAR_WIDTH > 0.0);
        assert!(UNIT_HEALTH_BAR_HEIGHT > 0.0);
        assert!(UNIT_HEALTH_BAR_Y_OFFSET > 0.0);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::testing::assert_entity_count;

    fn create_health_bar_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_observer(spawn_health_bars);
        app.add_systems(Update, update_health_bars);
        app
    }

    /// Default health bar config for unit-sized entities in tests.
    fn unit_health_bar_config() -> HealthBarConfig {
        HealthBarConfig {
            width: UNIT_HEALTH_BAR_WIDTH,
            height: UNIT_HEALTH_BAR_HEIGHT,
            y_offset: UNIT_HEALTH_BAR_Y_OFFSET,
        }
    }

    #[test]
    fn health_bar_spawned_on_entity_with_health() {
        let mut app = create_health_bar_test_app();

        app.world_mut()
            .spawn((Health::new(100.0), unit_health_bar_config()));
        app.update(); // spawn_health_bars runs, deferred with_children queued
        app.update(); // deferred commands applied

        assert_entity_count::<With<HealthBarBackground>>(&mut app, 1);
        assert_entity_count::<With<HealthBarFill>>(&mut app, 1);
    }

    #[test]
    fn health_bar_fill_scales_with_damage() {
        let mut app = create_health_bar_test_app();

        let entity = app
            .world_mut()
            .spawn((Health::new(100.0), unit_health_bar_config()))
            .id();
        app.update(); // spawn health bars
        app.update(); // apply deferred

        // Damage to 50%
        app.world_mut().get_mut::<Health>(entity).unwrap().current = 50.0;
        app.update(); // update_health_bars

        let mut bar_query = app
            .world_mut()
            .query_filtered::<&Transform, With<HealthBarFill>>();
        let bar_transform = bar_query.single(app.world()).unwrap();
        assert!(
            (bar_transform.scale.x - 0.5).abs() < f32::EPSILON,
            "Health bar fill should be 0.5, got {}",
            bar_transform.scale.x
        );
    }

    #[test]
    fn health_bar_despawned_with_parent() {
        let mut app = create_health_bar_test_app();

        let entity = app
            .world_mut()
            .spawn((Health::new(100.0), unit_health_bar_config()))
            .id();
        app.update(); // spawn health bars
        app.update(); // apply deferred

        assert_entity_count::<With<HealthBarBackground>>(&mut app, 1);

        // Despawn parent — children should go too (recursive despawn)
        app.world_mut().despawn(entity);

        assert_entity_count::<With<HealthBarBackground>>(&mut app, 0);
        assert_entity_count::<With<HealthBarFill>>(&mut app, 0);
    }

    #[test]
    fn health_bar_uses_config_dimensions() {
        let mut app = create_health_bar_test_app();

        app.world_mut().spawn((
            Health::new(100.0),
            HealthBarConfig {
                width: 50.0,
                height: 8.0,
                y_offset: 40.0,
            },
        ));
        app.update(); // spawn health bars
        app.update(); // apply deferred

        let mut bg_query = app
            .world_mut()
            .query_filtered::<&Transform, With<HealthBarBackground>>();
        let bg_transform = bg_query.single(app.world()).unwrap();
        assert!(
            (bg_transform.translation.y - 40.0).abs() < f32::EPSILON,
            "Background y_offset should be 40.0, got {}",
            bg_transform.translation.y
        );
    }

    #[test]
    fn update_health_bar_uses_config_width() {
        let mut app = create_health_bar_test_app();

        let config = HealthBarConfig {
            width: 50.0,
            height: 8.0,
            y_offset: 40.0,
        };
        let entity = app.world_mut().spawn((Health::new(100.0), config)).id();
        app.update(); // spawn health bars
        app.update(); // apply deferred

        // Damage to 50%
        app.world_mut().get_mut::<Health>(entity).unwrap().current = 50.0;
        app.update(); // update_health_bars

        let mut bar_query = app
            .world_mut()
            .query_filtered::<&Transform, With<HealthBarFill>>();
        let bar_transform = bar_query.single(app.world()).unwrap();
        // Left-alignment offset: width * -(1 - ratio) / 2 = 50 * -0.5 / 2 = -12.5
        assert!(
            (bar_transform.translation.x - (-12.5)).abs() < f32::EPSILON,
            "Fill translation.x should be -12.5, got {}",
            bar_transform.translation.x
        );
    }
}

//! Combat systems: attack timers, projectiles, damage, death, and health bars.

use bevy::prelude::*;

use crate::gameplay::units::{CombatStats, CurrentTarget, Health, Unit};
use crate::screens::GameState;

// === Constants ===

/// Projectile travel speed (pixels per second).
const PROJECTILE_SPEED: f32 = 200.0;

/// Projectile visual radius (pixels).
const PROJECTILE_RADIUS: f32 = 3.0;

/// Projectile color (yellow).
const PROJECTILE_COLOR: Color = Color::srgb(1.0, 1.0, 0.3);

/// Health bar colors.
const HEALTH_BAR_BG_COLOR: Color = Color::srgb(0.8, 0.1, 0.1);
const HEALTH_BAR_FILL_COLOR: Color = Color::srgb(0.1, 0.9, 0.1);

// === Components ===

/// Per-unit attack cooldown timer.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct AttackTimer(pub Timer);

/// A projectile in flight toward a target.
/// Spawned by `unit_attack`, moved by `move_projectiles`.
/// Damage is applied when the projectile reaches the target.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Projectile {
    pub target: Entity,
    pub damage: f32,
    pub speed: f32,
}

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

/// Default health bar width for units (pixels).
pub const UNIT_HEALTH_BAR_WIDTH: f32 = 20.0;

/// Default health bar height for units (pixels).
pub const UNIT_HEALTH_BAR_HEIGHT: f32 = 3.0;

/// Default health bar Y offset for units (pixels above center).
pub const UNIT_HEALTH_BAR_Y_OFFSET: f32 = 18.0;

// === Systems ===

/// Ticks attack timers and spawns projectiles toward targets in range.
/// Runs in `GameSet::Combat`.
fn unit_attack(
    time: Res<Time>,
    mut attackers: Query<
        (
            &CurrentTarget,
            &CombatStats,
            &mut AttackTimer,
            &GlobalTransform,
        ),
        With<Unit>,
    >,
    positions: Query<&GlobalTransform>,
    mut commands: Commands,
) {
    for (target, stats, mut timer, attacker_pos) in &mut attackers {
        let Some(target_entity) = target.0 else {
            continue;
        };
        let Ok(target_pos) = positions.get(target_entity) else {
            continue;
        };

        // Only attack when in range
        let distance = attacker_pos
            .translation()
            .truncate()
            .distance(target_pos.translation().truncate());
        if distance > stats.range {
            continue;
        }

        // Tick and spawn projectile when timer fires
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            commands.spawn((
                Projectile {
                    target: target_entity,
                    damage: stats.damage,
                    speed: PROJECTILE_SPEED,
                },
                Sprite::from_color(PROJECTILE_COLOR, Vec2::splat(PROJECTILE_RADIUS * 2.0)),
                Transform::from_xyz(
                    attacker_pos.translation().x,
                    attacker_pos.translation().y,
                    crate::Z_UNIT + 0.5,
                ),
                DespawnOnExit(GameState::InGame),
            ));
        }
    }
}

/// Moves projectiles toward their targets. On arrival, applies damage and
/// despawns the projectile. If the target no longer exists, despawns the
/// projectile harmlessly.
/// Runs in `GameSet::Combat`.
fn move_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut projectiles: Query<(Entity, &Projectile, &mut Transform)>,
    mut healths: Query<&mut Health>,
    positions: Query<&GlobalTransform>,
) {
    for (entity, projectile, mut transform) in &mut projectiles {
        // Target gone — despawn projectile harmlessly
        let Ok(target_pos) = positions.get(projectile.target) else {
            commands.entity(entity).despawn();
            continue;
        };

        let target_translation = target_pos.translation();
        let direction = target_translation.truncate() - transform.translation.truncate();
        let distance = direction.length();
        let move_amount = projectile.speed * time.delta_secs();

        if move_amount >= distance {
            // Arrived — apply damage and despawn
            if let Ok(mut health) = healths.get_mut(projectile.target) {
                health.current -= projectile.damage;
            }
            commands.entity(entity).despawn();
        } else {
            // Move toward target
            let dir = direction / distance;
            transform.translation.x = dir.x.mul_add(move_amount, transform.translation.x);
            transform.translation.y = dir.y.mul_add(move_amount, transform.translation.y);
        }
    }
}

/// Despawns any entity whose health drops to 0 or below.
/// Runs in `GameSet::Death`. Generic — works for units, fortresses, etc.
pub fn check_death(mut commands: Commands, query: Query<(Entity, &Health)>) {
    for (entity, health) in &query {
        if health.current <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// Spawns health bar child entities on any entity that just received a `Health` component.
/// Runs in `GameSet::Ui`.
fn spawn_health_bars(
    mut commands: Commands,
    new_entities: Query<(Entity, &HealthBarConfig), Added<Health>>,
) {
    for (entity, config) in &new_entities {
        commands.entity(entity).with_children(|parent| {
            // Red background (full width, always visible)
            parent.spawn((
                Sprite::from_color(HEALTH_BAR_BG_COLOR, Vec2::new(config.width, config.height)),
                Transform::from_xyz(0.0, config.y_offset, 1.0),
                HealthBarBackground,
            ));
            // Green fill (scales with HP ratio, rendered in front of background)
            parent.spawn((
                Sprite::from_color(
                    HEALTH_BAR_FILL_COLOR,
                    Vec2::new(config.width, config.height),
                ),
                Transform::from_xyz(0.0, config.y_offset, 1.1),
                HealthBarFill,
            ));
        });
    }
}

/// Updates health bar fill width based on current/max HP.
/// Runs in `GameSet::Ui`, after `spawn_health_bars`.
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
    app.register_type::<AttackTimer>()
        .register_type::<Projectile>()
        .register_type::<HealthBarBackground>()
        .register_type::<HealthBarFill>()
        .register_type::<HealthBarConfig>();

    // Combat: unit_attack spawns projectiles, move_projectiles resolves them.
    // chain_ignore_deferred so newly spawned projectiles don't move until next frame
    // (prevents instant-hit invisible projectiles).
    app.add_systems(
        Update,
        (unit_attack, move_projectiles)
            .chain_ignore_deferred()
            .in_set(crate::GameSet::Combat)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );

    app.add_systems(
        Update,
        check_death
            .in_set(crate::GameSet::Death)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );

    app.add_systems(
        Update,
        (spawn_health_bars, update_health_bars)
            .chain_ignore_deferred()
            .in_set(crate::GameSet::Ui)
            .run_if(in_state(GameState::InGame).and(in_state(crate::menus::Menu::None))),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::assertions_on_constants)]
    #[test]
    fn constants_are_valid() {
        assert!(PROJECTILE_SPEED > 0.0);
        assert!(PROJECTILE_RADIUS > 0.0);
        assert!(UNIT_HEALTH_BAR_WIDTH > 0.0);
        assert!(UNIT_HEALTH_BAR_HEIGHT > 0.0);
        assert!(UNIT_HEALTH_BAR_Y_OFFSET > 0.0);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::gameplay::units::{
        CombatStats, CurrentTarget, Movement, SOLDIER_ATTACK_RANGE, SOLDIER_ATTACK_SPEED,
        SOLDIER_DAMAGE, SOLDIER_MOVE_SPEED, Unit,
    };
    use crate::testing::assert_entity_count;
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    // === Test App Helpers ===

    fn create_attack_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, unit_attack);
        app.update(); // Initialize time (first frame delta=0)
        app
    }

    fn create_projectile_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, move_projectiles);
        app.update(); // Initialize time
        app
    }

    fn create_death_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, check_death);
        app
    }

    fn create_health_bar_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, (spawn_health_bars, update_health_bars).chain());
        app
    }

    /// Advance virtual time and run one update.
    fn advance_and_update(app: &mut App, dt: Duration) {
        app.world_mut()
            .resource_mut::<Time<Virtual>>()
            .advance_by(dt);
        app.update();
    }

    /// Spawn a unit with attack capability at the given position.
    /// Uses a very short timer (0.001s) with elapsed set to 0.999ms so any positive
    /// wall-clock delta triggers it (MinimalPlugins uses real wall-clock delta, not advance_by).
    fn spawn_attacker(world: &mut World, x: f32, target: Option<Entity>) -> Entity {
        let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
        timer.set_elapsed(Duration::from_nanos(999_000));
        world
            .spawn((
                Unit,
                CurrentTarget(target),
                CombatStats {
                    damage: SOLDIER_DAMAGE,
                    attack_speed: SOLDIER_ATTACK_SPEED,
                    range: SOLDIER_ATTACK_RANGE,
                },
                AttackTimer(timer),
                Movement {
                    speed: SOLDIER_MOVE_SPEED,
                },
                Transform::from_xyz(x, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, 100.0, 0.0)),
            ))
            .id()
    }

    /// Spawn a target entity with Health and GlobalTransform.
    fn spawn_target(world: &mut World, x: f32, hp: f32) -> Entity {
        world
            .spawn((
                Health::new(hp),
                Transform::from_xyz(x, 100.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(x, 100.0, 0.0)),
            ))
            .id()
    }

    // === Attack + Projectile Tests ===

    #[test]
    fn unit_spawns_projectile_in_range() {
        let mut app = create_attack_test_app();

        let target = spawn_target(app.world_mut(), 120.0, 100.0);
        spawn_attacker(app.world_mut(), 100.0, Some(target)); // distance = 20 < range 30

        advance_and_update(&mut app, Duration::from_millis(100));

        assert_entity_count::<With<Projectile>>(&mut app, 1);
    }

    #[test]
    fn unit_does_not_attack_out_of_range() {
        let mut app = create_attack_test_app();

        let target = spawn_target(app.world_mut(), 500.0, 100.0);
        spawn_attacker(app.world_mut(), 100.0, Some(target)); // distance = 400 > range 30

        advance_and_update(&mut app, Duration::from_millis(100));

        assert_entity_count::<With<Projectile>>(&mut app, 0);
    }

    #[test]
    fn attack_without_target_does_nothing() {
        let mut app = create_attack_test_app();

        spawn_attacker(app.world_mut(), 100.0, None);

        advance_and_update(&mut app, Duration::from_millis(100));

        assert_entity_count::<With<Projectile>>(&mut app, 0);
    }

    #[test]
    fn projectile_deals_damage_on_arrival() {
        let mut app = create_projectile_test_app();

        let target = spawn_target(app.world_mut(), 100.01, 100.0);

        // Spawn projectile very close to target (distance = 0.01px).
        // Use very high speed so even microsecond wall-clock delta causes arrival.
        app.world_mut().spawn((
            Projectile {
                target,
                damage: 25.0,
                speed: 100_000.0,
            },
            Transform::from_xyz(100.0, 100.0, 0.0),
        ));

        app.update();

        let health = app.world().get::<Health>(target).unwrap();
        assert_eq!(health.current, 75.0);
    }

    #[test]
    fn projectile_despawns_on_arrival() {
        let mut app = create_projectile_test_app();

        let target = spawn_target(app.world_mut(), 100.01, 100.0);

        // Very close + very high speed → arrives on first frame
        app.world_mut().spawn((
            Projectile {
                target,
                damage: 10.0,
                speed: 100_000.0,
            },
            Transform::from_xyz(100.0, 100.0, 0.0),
        ));

        app.update();

        assert_entity_count::<With<Projectile>>(&mut app, 0);
    }

    #[test]
    fn projectile_despawns_when_target_missing() {
        let mut app = create_projectile_test_app();

        let target = spawn_target(app.world_mut(), 500.0, 100.0);

        app.world_mut().spawn((
            Projectile {
                target,
                damage: 10.0,
                speed: PROJECTILE_SPEED,
            },
            Transform::from_xyz(100.0, 100.0, 0.0),
        ));

        // Despawn target before projectile arrives
        app.world_mut().despawn(target);

        advance_and_update(&mut app, Duration::from_millis(50));

        assert_entity_count::<With<Projectile>>(&mut app, 0);
    }

    #[test]
    fn attack_respects_cooldown() {
        let mut app = create_attack_test_app();

        let target = spawn_target(app.world_mut(), 120.0, 100.0);

        // Spawn attacker with fresh timer (NOT nearly elapsed)
        app.world_mut().spawn((
            Unit,
            CurrentTarget(Some(target)),
            CombatStats {
                damage: SOLDIER_DAMAGE,
                attack_speed: SOLDIER_ATTACK_SPEED,
                range: SOLDIER_ATTACK_RANGE,
            },
            AttackTimer(Timer::from_seconds(
                1.0 / SOLDIER_ATTACK_SPEED,
                TimerMode::Repeating,
            )),
            Movement {
                speed: SOLDIER_MOVE_SPEED,
            },
            Transform::from_xyz(100.0, 100.0, 0.0),
            GlobalTransform::from(Transform::from_xyz(100.0, 100.0, 0.0)),
        ));

        // First few frames — timer hasn't fired yet
        advance_and_update(&mut app, Duration::from_millis(100));

        assert_entity_count::<With<Projectile>>(&mut app, 0);
    }

    // === Death System Tests ===

    #[test]
    fn entity_despawned_at_zero_hp() {
        let mut app = create_death_test_app();

        app.world_mut().spawn(Health {
            current: 0.0,
            max: 100.0,
        });
        app.update();

        assert_entity_count::<With<Health>>(&mut app, 0);
    }

    #[test]
    fn entity_despawned_at_negative_hp() {
        let mut app = create_death_test_app();

        app.world_mut().spawn(Health {
            current: -10.0,
            max: 100.0,
        });
        app.update();

        assert_entity_count::<With<Health>>(&mut app, 0);
    }

    #[test]
    fn entity_survives_above_zero_hp() {
        let mut app = create_death_test_app();

        app.world_mut().spawn(Health {
            current: 1.0,
            max: 100.0,
        });
        app.update();

        assert_entity_count::<With<Health>>(&mut app, 1);
    }

    // === Health Bar Tests ===

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

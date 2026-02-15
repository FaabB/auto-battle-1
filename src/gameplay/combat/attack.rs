//! Attack mechanics: timers, projectiles, and damage application.

use bevy::prelude::*;

use crate::gameplay::Health;
use crate::gameplay::units::{CombatStats, CurrentTarget, Unit};
use crate::screens::GameState;
use crate::{GameSet, Z_UNIT, gameplay_running};

// === Constants ===

/// Projectile travel speed (pixels per second).
const PROJECTILE_SPEED: f32 = 200.0;

/// Projectile visual radius (pixels).
const PROJECTILE_RADIUS: f32 = 3.0;

/// Projectile color (yellow).
const PROJECTILE_COLOR: Color = Color::srgb(1.0, 1.0, 0.3);

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
                Name::new("Projectile"),
                Projectile {
                    target: target_entity,
                    damage: stats.damage,
                    speed: PROJECTILE_SPEED,
                },
                Sprite::from_color(PROJECTILE_COLOR, Vec2::splat(PROJECTILE_RADIUS * 2.0)),
                Transform::from_xyz(
                    attacker_pos.translation().x,
                    attacker_pos.translation().y,
                    Z_UNIT + 0.5,
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

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<AttackTimer>()
        .register_type::<Projectile>();

    // Combat: unit_attack spawns projectiles, move_projectiles resolves them.
    // chain_ignore_deferred so newly spawned projectiles don't move until next frame
    // (prevents instant-hit invisible projectiles).
    app.add_systems(
        Update,
        (unit_attack, move_projectiles)
            .chain_ignore_deferred()
            .in_set(GameSet::Combat)
            .run_if(gameplay_running),
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
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::gameplay::units::{
        CombatStats, CurrentTarget, Movement, Unit, UnitType, unit_stats,
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
        let stats = unit_stats(UnitType::Soldier);
        let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
        timer.set_elapsed(Duration::from_nanos(999_000));
        world
            .spawn((
                Unit,
                CurrentTarget(target),
                CombatStats {
                    damage: stats.damage,
                    attack_speed: stats.attack_speed,
                    range: stats.attack_range,
                },
                AttackTimer(timer),
                Movement {
                    speed: stats.move_speed,
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
        let stats = unit_stats(UnitType::Soldier);

        // Spawn attacker with fresh timer (NOT nearly elapsed)
        app.world_mut().spawn((
            Unit,
            CurrentTarget(Some(target)),
            CombatStats {
                damage: stats.damage,
                attack_speed: stats.attack_speed,
                range: stats.attack_range,
            },
            AttackTimer(Timer::from_seconds(
                1.0 / stats.attack_speed,
                TimerMode::Repeating,
            )),
            Movement {
                speed: stats.move_speed,
            },
            Transform::from_xyz(100.0, 100.0, 0.0),
            GlobalTransform::from(Transform::from_xyz(100.0, 100.0, 0.0)),
        ));

        // First few frames — timer hasn't fired yet
        advance_and_update(&mut app, Duration::from_millis(100));

        assert_entity_count::<With<Projectile>>(&mut app, 0);
    }
}

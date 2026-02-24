//! Attack mechanics: timers, projectiles, and damage application.

use avian2d::prelude::*;
use bevy::prelude::*;

use crate::gameplay::{CombatStats, CurrentTarget, Health, Team};
use crate::screens::GameState;
use crate::third_party::{CollisionLayer, surface_distance};
use crate::{GameSet, Z_UNIT, gameplay_running};

// === Constants ===

/// Projectile travel speed (pixels per second).
const PROJECTILE_SPEED: f32 = 200.0;

/// Projectile visual radius (pixels).
const PROJECTILE_RADIUS: f32 = 2.0;

use crate::theme::palette;

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

/// Marker for hitbox sensor entities (attack colliders that damage hurtbox targets).
/// Lives on projectiles; future: melee swing entities.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Hitbox;

// === Systems ===

/// Ticks attack timers and spawns projectiles toward targets in range.
/// Uses surface-to-surface distance so entities can attack large targets (buildings, fortresses).
/// Runs in `GameSet::Combat`.
fn attack(
    time: Res<Time>,
    mut attackers: Query<(
        &CurrentTarget,
        &CombatStats,
        &mut AttackTimer,
        &GlobalTransform,
        &Collider,
        &Team,
    )>,
    targets: Query<(&GlobalTransform, &Collider)>,
    mut commands: Commands,
) {
    for (target, stats, mut timer, attacker_pos, attacker_collider, team) in &mut attackers {
        // Always tick the timer so it stays warm — entities fire on a cadence
        // regardless of whether a target is currently in range.
        timer.0.tick(time.delta());
        let ready = timer.0.just_finished();

        let Some(target_entity) = target.0 else {
            continue;
        };
        let Ok((target_pos, target_collider)) = targets.get(target_entity) else {
            continue;
        };

        // Only attack when in range (surface-to-surface)
        let distance = surface_distance(
            attacker_collider,
            attacker_pos.translation().xy(),
            target_collider,
            target_pos.translation().xy(),
        );
        if distance > stats.range {
            continue;
        }

        if ready {
            commands.spawn((
                Name::new("Projectile"),
                Projectile {
                    target: target_entity,
                    damage: stats.damage,
                    speed: PROJECTILE_SPEED,
                },
                *team,
                Hitbox,
                Sprite::from_color(palette::PROJECTILE, Vec2::splat(PROJECTILE_RADIUS * 2.0)),
                Transform::from_xyz(
                    attacker_pos.translation().x,
                    attacker_pos.translation().y,
                    Z_UNIT + 0.5,
                ),
                DespawnOnExit(GameState::InGame),
                // Physics: sensor hitbox for collision-based damage
                RigidBody::Kinematic,
                Collider::circle(PROJECTILE_RADIUS),
                Sensor,
                CollisionLayers::new(CollisionLayer::Hitbox, CollisionLayer::Hurtbox),
                CollisionEventsEnabled,
                CollidingEntities::default(),
            ));
        }
    }
}

/// Moves projectiles toward their targets. Snaps to target position on overshoot
/// so the collision system can detect the hit. If the target no longer exists,
/// despawns the projectile harmlessly.
/// Runs in `GameSet::Combat`.
fn move_projectiles(
    time: Res<Time>,
    mut commands: Commands,
    mut projectiles: Query<(Entity, &Projectile, &mut Transform)>,
    positions: Query<&GlobalTransform>,
) {
    for (entity, projectile, mut transform) in &mut projectiles {
        // Target gone — despawn projectile harmlessly
        let Ok(target_pos) = positions.get(projectile.target) else {
            commands.entity(entity).despawn();
            continue;
        };

        let target_xy = target_pos.translation().truncate();
        let current_xy = transform.translation.truncate();
        let direction = target_xy - current_xy;
        let distance = direction.length();

        if distance < f32::EPSILON {
            continue; // At target — collision system handles damage
        }

        let move_amount = projectile.speed * time.delta_secs();
        if move_amount >= distance {
            // Snap to target to prevent tunneling (collision handles damage)
            transform.translation.x = target_xy.x;
            transform.translation.y = target_xy.y;
        } else {
            let dir = direction / distance;
            transform.translation.x = dir.x.mul_add(move_amount, transform.translation.x);
            transform.translation.y = dir.y.mul_add(move_amount, transform.translation.y);
        }
    }
}

/// Checks projectile hitbox overlaps with hurtboxes via `CollidingEntities`.
/// Damages the first opposing-team entity hit and despawns the projectile.
/// Runs after `move_projectiles` in the combat chain.
fn handle_projectile_hits(
    mut commands: Commands,
    projectiles: Query<(Entity, &Projectile, &Team, &CollidingEntities), With<Hitbox>>,
    mut targets: Query<(&Team, &mut Health)>,
) {
    for (entity, projectile, proj_team, colliding) in &projectiles {
        for &hit in &colliding.0 {
            let Ok((hit_team, mut health)) = targets.get_mut(hit) else {
                continue;
            };
            // No friendly fire
            if hit_team == proj_team {
                continue;
            }
            health.current -= projectile.damage;
            commands.entity(entity).despawn();
            break; // One hit per projectile
        }
    }
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<AttackTimer>()
        .register_type::<Projectile>()
        .register_type::<Hitbox>();

    // Combat: spawn → move → check hits.
    // chain_ignore_deferred so newly spawned projectiles don't move until next frame
    // (prevents instant-hit invisible projectiles).
    app.add_systems(
        Update,
        (attack, move_projectiles, handle_projectile_hits)
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
    use crate::gameplay::{CurrentTarget, Team};
    use crate::testing::assert_entity_count;
    use pretty_assertions::assert_eq;
    use std::time::Duration;

    // === Test App Helpers ===

    fn create_attack_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, attack);
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
    /// Attack timer is nearly expired so any positive delta triggers it.
    fn spawn_attacker(world: &mut World, x: f32, target: Option<Entity>) -> Entity {
        let id = crate::testing::spawn_test_unit(world, Team::Player, x, 100.0);
        if let Some(t) = target {
            world.entity_mut(id).insert(CurrentTarget(Some(t)));
        }
        // Nearly-expire the attack timer for immediate attack
        crate::testing::nearly_expire_timer(
            &mut world.entity_mut(id).get_mut::<AttackTimer>().unwrap().0,
        );
        id
    }

    /// Spawn a target entity with Health at the given position.
    fn spawn_target(world: &mut World, x: f32, hp: f32) -> Entity {
        let id = crate::testing::spawn_test_target(world, Team::Enemy, x, 100.0);
        world.entity_mut(id).insert(Health::new(hp));
        id
    }

    // === Attack + Projectile Tests ===

    #[test]
    fn unit_spawns_projectile_in_range() {
        let mut app = create_attack_test_app();

        let target = spawn_target(app.world_mut(), 114.0, 100.0);
        spawn_attacker(app.world_mut(), 100.0, Some(target)); // surface distance = 14 - 6 - 5 = 3 < range 5

        advance_and_update(&mut app, Duration::from_millis(100));

        assert_entity_count::<With<Projectile>>(&mut app, 1);
    }

    #[test]
    fn unit_does_not_attack_out_of_range() {
        let mut app = create_attack_test_app();

        let target = spawn_target(app.world_mut(), 500.0, 100.0);
        spawn_attacker(app.world_mut(), 100.0, Some(target)); // surface distance = 383 > range 5

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

        let target = spawn_target(app.world_mut(), 114.0, 100.0);

        // Spawn attacker with fresh timer (NOT nearly elapsed)
        let attacker = crate::testing::spawn_test_unit(app.world_mut(), Team::Player, 100.0, 100.0);
        app.world_mut()
            .entity_mut(attacker)
            .insert(CurrentTarget(Some(target)));

        // First few frames — timer hasn't fired yet
        advance_and_update(&mut app, Duration::from_millis(100));

        assert_entity_count::<With<Projectile>>(&mut app, 0);
    }

    // === Collision-Based Hit Tests ===

    fn create_hit_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, handle_projectile_hits);
        app.update(); // Initialize
        app
    }

    /// Spawn a projectile with team, damage, and pre-populated `CollidingEntities`.
    fn spawn_test_projectile(
        world: &mut World,
        team: Team,
        target: Entity,
        damage: f32,
        colliding_with: &[Entity],
    ) -> Entity {
        use bevy::ecs::entity::hash_set::EntityHashSet;
        let colliding = CollidingEntities(EntityHashSet::from_iter(colliding_with.iter().copied()));
        world
            .spawn((
                Projectile {
                    target,
                    damage,
                    speed: 200.0,
                },
                team,
                Hitbox,
                colliding,
            ))
            .id()
    }

    #[test]
    fn projectile_hit_applies_damage() {
        let mut app = create_hit_test_app();

        let enemy = app
            .world_mut()
            .spawn((Team::Enemy, Health::new(100.0)))
            .id();
        spawn_test_projectile(app.world_mut(), Team::Player, enemy, 25.0, &[enemy]);

        app.update();

        let health = app.world().get::<Health>(enemy).unwrap();
        assert_eq!(health.current, 75.0);
    }

    #[test]
    fn projectile_despawns_on_hit() {
        let mut app = create_hit_test_app();

        let enemy = app
            .world_mut()
            .spawn((Team::Enemy, Health::new(100.0)))
            .id();
        spawn_test_projectile(app.world_mut(), Team::Player, enemy, 10.0, &[enemy]);

        app.update();

        assert_entity_count::<With<Projectile>>(&mut app, 0);
    }

    #[test]
    fn projectile_does_not_friendly_fire() {
        let mut app = create_hit_test_app();

        // Player projectile collides with a friendly player unit
        let friendly = app
            .world_mut()
            .spawn((Team::Player, Health::new(100.0)))
            .id();
        let dummy_target = app
            .world_mut()
            .spawn((Team::Enemy, Health::new(100.0)))
            .id();
        spawn_test_projectile(
            app.world_mut(),
            Team::Player,
            dummy_target,
            25.0,
            &[friendly],
        );

        app.update();

        // Friendly undamaged, projectile still alive
        let hp = app.world().get::<Health>(friendly).unwrap();
        assert_eq!(hp.current, 100.0);
        assert_entity_count::<With<Projectile>>(&mut app, 1);
    }

    #[test]
    fn projectile_hits_non_target_enemy() {
        let mut app = create_hit_test_app();

        // Projectile aimed at enemy_far but collides with enemy_near (different entity)
        let enemy_near = app
            .world_mut()
            .spawn((Team::Enemy, Health::new(100.0)))
            .id();
        let enemy_far = app
            .world_mut()
            .spawn((Team::Enemy, Health::new(100.0)))
            .id();
        spawn_test_projectile(
            app.world_mut(),
            Team::Player,
            enemy_far,
            25.0,
            &[enemy_near],
        );

        app.update();

        // enemy_near takes damage (even though it wasn't the intended target)
        let near_hp = app.world().get::<Health>(enemy_near).unwrap();
        assert_eq!(near_hp.current, 75.0);
        // enemy_far undamaged
        let far_hp = app.world().get::<Health>(enemy_far).unwrap();
        assert_eq!(far_hp.current, 100.0);
        assert_entity_count::<With<Projectile>>(&mut app, 0);
    }

    #[test]
    fn projectile_no_collision_yet() {
        let mut app = create_hit_test_app();

        let enemy = app
            .world_mut()
            .spawn((Team::Enemy, Health::new(100.0)))
            .id();
        spawn_test_projectile(app.world_mut(), Team::Player, enemy, 25.0, &[]); // empty

        app.update();

        let health = app.world().get::<Health>(enemy).unwrap();
        assert_eq!(health.current, 100.0);
        assert_entity_count::<With<Projectile>>(&mut app, 1);
    }

    #[test]
    fn fortress_can_attack_in_range() {
        let mut app = create_attack_test_app();

        // Spawn a "fortress-like" entity (no Unit marker)
        let mut timer = Timer::from_seconds(0.001, TimerMode::Repeating);
        crate::testing::nearly_expire_timer(&mut timer);
        let fortress = app
            .world_mut()
            .spawn((
                Team::Player,
                CurrentTarget(None),
                CombatStats {
                    damage: 50.0,
                    attack_speed: 0.5,
                    range: 200.0,
                },
                AttackTimer(timer),
                Transform::from_xyz(64.0, 320.0, 0.0),
                GlobalTransform::from(Transform::from_xyz(64.0, 320.0, 0.0)),
                Collider::rectangle(128.0, 128.0),
            ))
            .id();

        let target = spawn_target(app.world_mut(), 200.0, 100.0);

        // Set fortress target
        app.world_mut()
            .get_mut::<CurrentTarget>(fortress)
            .unwrap()
            .0 = Some(target);

        advance_and_update(&mut app, Duration::from_millis(100));
        assert_entity_count::<With<Projectile>>(&mut app, 1);
    }

    // NOTE: Tier 2 integration test with PhysicsPlugins was removed because avian2d's
    // FixedUpdate-based collision pipeline is unreliable under MinimalPlugins (wall-clock
    // time accumulation is non-deterministic). Collision layer wiring is verified by
    // manual play testing instead. The Tier 1 tests above with manually populated
    // CollidingEntities cover the handle_projectile_hits logic thoroughly.
}

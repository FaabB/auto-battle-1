//! Death detection: despawns entities at zero health.

use bevy::prelude::*;

use crate::gameplay::{EngagementLeash, Health, Movement, Target, TargetingState};
use crate::{GameSet, gameplay_running};

/// `SystemSet` for death detection. Other systems can order against this
/// (e.g., `.before(DeathCheck)`) instead of referencing the function directly.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeathCheck;

/// Despawns any entity whose health drops to 0 or below.
fn check_death(mut commands: Commands, query: Query<(Entity, &Health)>) {
    for (entity, health) in &query {
        if health.current <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

/// When a targetable entity dies (Target removed during despawn),
/// transition orphaned units: mobile → Moving, static → Seeking. Remove leash.
fn handle_target_death(
    trigger: On<Remove, Target>,
    mut seekers: Query<(Entity, &mut TargetingState, Option<&Movement>)>,
    mut commands: Commands,
) {
    let dead_entity = trigger.entity;
    for (entity, mut state, movement) in &mut seekers {
        match *state {
            TargetingState::Engaging(e) | TargetingState::Attacking(e) if e == dead_entity => {
                *state = if movement.is_some() {
                    TargetingState::Moving
                } else {
                    TargetingState::Seeking
                };
                commands.entity(entity).remove::<EngagementLeash>();
            }
            _ => {}
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_observer(handle_target_death);
    app.add_systems(
        Update,
        check_death
            .in_set(DeathCheck)
            .in_set(GameSet::Death)
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod observer_tests {
    use super::*;
    use crate::gameplay::Team;

    fn create_observer_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_observer(handle_target_death);
        app.add_systems(Update, check_death);
        app
    }

    /// Spawn a mobile seeker (has Movement) with TargetingState that can be orphaned.
    fn spawn_mobile_seeker(world: &mut World, state: TargetingState) -> Entity {
        world
            .spawn((Team::Player, state, Movement { speed: 50.0 }))
            .id()
    }

    /// Spawn a static seeker (no Movement) with TargetingState that can be orphaned.
    fn spawn_static_seeker(world: &mut World, state: TargetingState) -> Entity {
        world.spawn((Team::Player, state)).id()
    }

    /// Spawn a minimal target entity with Health so check_death can despawn it.
    fn spawn_mortal_target(world: &mut World) -> Entity {
        world
            .spawn((
                Team::Enemy,
                Target,
                Health {
                    current: 0.0,
                    max: 100.0,
                },
            ))
            .id()
    }

    #[test]
    fn orphaned_mobile_engaging_transitions_to_moving() {
        let mut app = create_observer_test_app();
        let target = spawn_mortal_target(app.world_mut());
        let seeker = spawn_mobile_seeker(app.world_mut(), TargetingState::Engaging(target));

        app.update(); // check_death despawns target → observer fires

        let state = app.world().get::<TargetingState>(seeker).unwrap();
        assert_eq!(*state, TargetingState::Moving);
    }

    #[test]
    fn orphaned_static_engaging_transitions_to_seeking() {
        let mut app = create_observer_test_app();
        let target = spawn_mortal_target(app.world_mut());
        let seeker = spawn_static_seeker(app.world_mut(), TargetingState::Engaging(target));

        app.update();

        let state = app.world().get::<TargetingState>(seeker).unwrap();
        assert_eq!(*state, TargetingState::Seeking);
    }

    #[test]
    fn orphaned_mobile_attacking_transitions_to_moving() {
        let mut app = create_observer_test_app();
        let target = spawn_mortal_target(app.world_mut());
        let seeker = spawn_mobile_seeker(app.world_mut(), TargetingState::Attacking(target));

        app.update();

        let state = app.world().get::<TargetingState>(seeker).unwrap();
        assert_eq!(*state, TargetingState::Moving);
    }

    #[test]
    fn orphaned_static_attacking_transitions_to_seeking() {
        let mut app = create_observer_test_app();
        let target = spawn_mortal_target(app.world_mut());
        let seeker = spawn_static_seeker(app.world_mut(), TargetingState::Attacking(target));

        app.update();

        let state = app.world().get::<TargetingState>(seeker).unwrap();
        assert_eq!(*state, TargetingState::Seeking);
    }

    #[test]
    fn unit_targeting_different_entity_unaffected() {
        let mut app = create_observer_test_app();
        let _target = spawn_mortal_target(app.world_mut());
        let other = app.world_mut().spawn((Team::Enemy, Target)).id();
        let seeker = spawn_mobile_seeker(app.world_mut(), TargetingState::Engaging(other));

        app.update(); // target dies, but seeker is engaging `other`

        let state = app.world().get::<TargetingState>(seeker).unwrap();
        assert_eq!(*state, TargetingState::Engaging(other));
    }

    #[test]
    fn seeking_unit_unaffected_by_death() {
        let mut app = create_observer_test_app();
        let _target = spawn_mortal_target(app.world_mut());
        let seeker = spawn_static_seeker(app.world_mut(), TargetingState::Seeking);

        app.update();

        let state = app.world().get::<TargetingState>(seeker).unwrap();
        assert_eq!(*state, TargetingState::Seeking);
    }

    #[test]
    fn multiple_orphans_all_transition() {
        let mut app = create_observer_test_app();
        let target = spawn_mortal_target(app.world_mut());
        let s1 = spawn_mobile_seeker(app.world_mut(), TargetingState::Engaging(target));
        let s2 = spawn_static_seeker(app.world_mut(), TargetingState::Attacking(target));
        let s3 = spawn_mobile_seeker(app.world_mut(), TargetingState::Engaging(target));

        app.update();

        // Mobile units → Moving, static → Seeking
        assert_eq!(
            *app.world().get::<TargetingState>(s1).unwrap(),
            TargetingState::Moving
        );
        assert_eq!(
            *app.world().get::<TargetingState>(s2).unwrap(),
            TargetingState::Seeking
        );
        assert_eq!(
            *app.world().get::<TargetingState>(s3).unwrap(),
            TargetingState::Moving
        );
    }

    #[test]
    fn death_removes_engagement_leash() {
        let mut app = create_observer_test_app();
        let target = spawn_mortal_target(app.world_mut());
        let seeker = spawn_mobile_seeker(app.world_mut(), TargetingState::Engaging(target));
        app.world_mut().entity_mut(seeker).insert(EngagementLeash {
            origin: Vec2::new(100.0, 100.0),
            max_distance: 192.0,
        });

        app.update();

        assert!(
            app.world().get::<EngagementLeash>(seeker).is_none(),
            "Leash should be removed on target death"
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::testing::assert_entity_count;

    fn create_death_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, check_death);
        app
    }

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
}

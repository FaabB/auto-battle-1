//! Death detection: despawns entities at zero health.

use bevy::prelude::*;

use crate::gameplay::Health;
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

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        check_death
            .in_set(DeathCheck)
            .in_set(GameSet::Death)
            .run_if(gameplay_running),
    );
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

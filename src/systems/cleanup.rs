//! Generic cleanup systems for state transitions.

use bevy::prelude::*;

/// Despawns all entities with the specified cleanup component.
pub fn cleanup_entities<T: Component>(mut commands: Commands, query: Query<Entity, With<T>>) {
    for entity in &query {
        commands.entity(entity).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::CleanupLoading;
    use crate::testing::*;

    #[test]
    fn cleanup_removes_marked_entities() {
        let mut app = create_test_app();

        // Spawn entities with cleanup marker
        let entity_with_marker = app.world_mut().spawn(CleanupLoading).id();
        let entity_without_marker = app.world_mut().spawn_empty().id();

        // Add and run cleanup system
        app.add_systems(Update, cleanup_entities::<CleanupLoading>);
        tick(&mut app);

        // Verify marked entity is despawned
        assert!(app.world().get_entity(entity_with_marker).is_err());
        // Verify unmarked entity still exists
        assert!(app.world().get_entity(entity_without_marker).is_ok());
    }

    #[test]
    fn cleanup_handles_empty_query() {
        let mut app = create_test_app();
        app.add_systems(Update, cleanup_entities::<CleanupLoading>);

        // Should not panic with no matching entities
        tick(&mut app);
    }
}

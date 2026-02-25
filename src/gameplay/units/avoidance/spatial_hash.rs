//! Uniform-grid spatial hash for fast neighbor queries.

use bevy::prelude::*;
use std::collections::HashMap;

/// Spatial hash for O(1) neighbor lookups. Rebuilt every frame.
#[derive(Resource, Debug)]
pub struct SpatialHash {
    cell_size: f32,
    cells: HashMap<(i32, i32), Vec<Entity>>,
}

impl SpatialHash {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            cells: HashMap::new(),
        }
    }

    /// Remove all entries. Called at the start of each frame's rebuild.
    pub fn clear(&mut self) {
        for bucket in self.cells.values_mut() {
            bucket.clear();
        }
    }

    /// Insert an entity at a world position.
    pub fn insert(&mut self, entity: Entity, position: Vec2) {
        let coords = self.cell_coords(position);
        self.cells.entry(coords).or_default().push(entity);
    }

    /// Query all entities within `radius` of `position`.
    /// Returns candidates — caller must still check actual distance.
    pub fn query_neighbors(&self, position: Vec2, radius: f32) -> Vec<Entity> {
        let min = self.cell_coords(position - Vec2::splat(radius));
        let max = self.cell_coords(position + Vec2::splat(radius));
        let mut result = Vec::new();
        for x in min.0..=max.0 {
            for y in min.1..=max.1 {
                if let Some(entities) = self.cells.get(&(x, y)) {
                    result.extend(entities);
                }
            }
        }
        result
    }

    #[allow(clippy::cast_possible_truncation)]
    fn cell_coords(&self, position: Vec2) -> (i32, i32) {
        (
            (position.x / self.cell_size).floor() as i32,
            (position.y / self.cell_size).floor() as i32,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_query_single_entity() {
        let mut hash = SpatialHash::new(50.0);
        let entity = Entity::from_bits(1);
        hash.insert(entity, Vec2::new(25.0, 25.0));

        let neighbors = hash.query_neighbors(Vec2::new(25.0, 25.0), 10.0);
        assert!(
            neighbors.contains(&entity),
            "Should find the inserted entity"
        );
    }

    #[test]
    fn query_returns_entities_within_radius() {
        let mut hash = SpatialHash::new(50.0);
        let e1 = Entity::from_bits(1);
        let e2 = Entity::from_bits(2);
        hash.insert(e1, Vec2::new(10.0, 10.0));
        hash.insert(e2, Vec2::new(40.0, 10.0));

        let neighbors = hash.query_neighbors(Vec2::new(25.0, 10.0), 20.0);
        assert!(neighbors.contains(&e1), "Should find e1 within radius");
        assert!(neighbors.contains(&e2), "Should find e2 within radius");
    }

    #[test]
    fn query_excludes_distant_entities() {
        let mut hash = SpatialHash::new(50.0);
        let near = Entity::from_bits(1);
        let far = Entity::from_bits(2);
        hash.insert(near, Vec2::new(10.0, 10.0));
        hash.insert(far, Vec2::new(500.0, 500.0));

        let neighbors = hash.query_neighbors(Vec2::new(10.0, 10.0), 30.0);
        assert!(neighbors.contains(&near), "Should find nearby entity");
        assert!(!neighbors.contains(&far), "Should not find distant entity");
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut hash = SpatialHash::new(50.0);
        hash.insert(Entity::from_bits(1), Vec2::new(10.0, 10.0));
        hash.insert(Entity::from_bits(2), Vec2::new(100.0, 100.0));
        hash.clear();

        let neighbors = hash.query_neighbors(Vec2::new(10.0, 10.0), 1000.0);
        assert!(neighbors.is_empty(), "Should find nothing after clear");
    }

    #[test]
    fn entities_on_cell_boundary_found_by_neighbors() {
        let mut hash = SpatialHash::new(50.0);
        let entity = Entity::from_bits(1);
        // Place entity exactly on a cell boundary.
        hash.insert(entity, Vec2::new(50.0, 0.0));

        let neighbors = hash.query_neighbors(Vec2::new(49.0, 0.0), 5.0);
        assert!(
            neighbors.contains(&entity),
            "Entity on boundary should be found by nearby query"
        );
    }

    #[test]
    fn large_radius_covers_many_cells() {
        let mut hash = SpatialHash::new(10.0);
        let mut entities = Vec::new();
        for i in 1..=5 {
            let e = Entity::from_bits(i);
            #[allow(clippy::cast_precision_loss)]
            hash.insert(e, Vec2::new(i as f32 * 15.0, 0.0));
            entities.push(e);
        }

        // Large radius should cover all entities.
        let neighbors = hash.query_neighbors(Vec2::new(30.0, 0.0), 50.0);
        for e in &entities {
            assert!(neighbors.contains(e), "Large radius should find entity {e}");
        }
    }
}

//! Unit components, constants, and shared rendering assets.

mod ai;
mod movement;
pub mod spawn;

use bevy::prelude::*;

use crate::screens::GameState;
use crate::{GameSet, gameplay_running};

// === Constants ===

/// Prototype soldier stats.
pub const SOLDIER_HEALTH: f32 = 100.0;
pub const SOLDIER_DAMAGE: f32 = 10.0;
pub const SOLDIER_ATTACK_SPEED: f32 = 1.0;
pub const SOLDIER_MOVE_SPEED: f32 = 50.0;
pub const SOLDIER_ATTACK_RANGE: f32 = 30.0;

/// Visual radius of a unit circle.
pub const UNIT_RADIUS: f32 = 12.0;

/// Player unit color (green).
const PLAYER_UNIT_COLOR: Color = Color::srgb(0.2, 0.8, 0.2);

/// Enemy unit color (red).
const ENEMY_UNIT_COLOR: Color = Color::srgb(0.8, 0.2, 0.2);

/// Maximum distance (pixels) a unit will backtrack to chase a target behind it.
/// 2 cells = 128 pixels.
pub const BACKTRACK_DISTANCE: f32 = 2.0 * crate::gameplay::battlefield::CELL_SIZE;

// === Components ===

/// Marker for unit entities.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Unit;

/// Combat parameters.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct CombatStats {
    pub damage: f32,
    pub attack_speed: f32,
    pub range: f32,
}

/// Movement speed.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Movement {
    pub speed: f32,
}

/// Tracks the entity this unit is currently moving toward / attacking.
/// Updated by the AI system; read by movement and (future) combat systems.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct CurrentTarget(pub Option<Entity>);

// === Resources ===

/// Shared mesh and material handles for unit circle rendering.
#[derive(Resource, Debug)]
pub struct UnitAssets {
    pub mesh: Handle<Mesh>,
    pub player_material: Handle<ColorMaterial>,
    pub enemy_material: Handle<ColorMaterial>,
}

// === Systems ===

fn setup_unit_assets(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.insert_resource(UnitAssets {
        mesh: meshes.add(Circle::new(UNIT_RADIUS)),
        player_material: materials.add(PLAYER_UNIT_COLOR),
        enemy_material: materials.add(ENEMY_UNIT_COLOR),
    });
}

// === Plugin ===

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Unit>()
        .register_type::<CombatStats>()
        .register_type::<Movement>()
        .register_type::<CurrentTarget>();

    app.add_systems(OnEnter(GameState::InGame), setup_unit_assets);

    spawn::plugin(app);

    app.add_systems(
        Update,
        ai::unit_find_target
            .in_set(GameSet::Ai)
            .run_if(gameplay_running),
    );

    app.add_systems(
        Update,
        movement::unit_movement
            .in_set(GameSet::Movement)
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn health_new_sets_current_to_max() {
        let health = crate::gameplay::Health::new(100.0);
        assert_eq!(health.current, 100.0);
        assert_eq!(health.max, 100.0);
    }

    #[test]
    fn team_variants_are_distinct() {
        use crate::gameplay::Team;
        assert_ne!(Team::Player, Team::Enemy);
    }

    #[test]
    fn soldier_stats_are_positive() {
        assert!(SOLDIER_HEALTH > 0.0);
        assert!(SOLDIER_DAMAGE > 0.0);
        assert!(SOLDIER_ATTACK_SPEED > 0.0);
        assert!(SOLDIER_MOVE_SPEED > 0.0);
        assert!(SOLDIER_ATTACK_RANGE > 0.0);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::testing::transition_to_ingame;

    #[test]
    fn unit_assets_created_on_enter_ingame() {
        let mut app = crate::testing::create_base_test_app();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();
        app.add_plugins(plugin);
        transition_to_ingame(&mut app);

        assert!(app.world().get_resource::<UnitAssets>().is_some());
    }
}

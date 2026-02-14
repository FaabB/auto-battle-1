//! Development tools — only included with `cargo run --features dev`.
//!
//! Debug overlays, test spawners, and inspector setup go here.
//! This module is stripped from release builds.

use bevy::prelude::*;

use crate::Z_UNIT;
use crate::gameplay::battlefield::{
    COMBAT_ZONE_COLS, COMBAT_ZONE_START_COL, col_to_world_x, row_to_world_y,
};
use crate::gameplay::combat::AttackTimer;
use crate::gameplay::units::{
    CombatStats, CurrentTarget, Health, Movement, SOLDIER_ATTACK_RANGE, SOLDIER_ATTACK_SPEED,
    SOLDIER_DAMAGE, SOLDIER_HEALTH, SOLDIER_MOVE_SPEED, Target, Team, Unit, UnitAssets,
};
use crate::menus::Menu;
use crate::screens::GameState;

/// Number of enemies spawned per E key press.
const ENEMIES_PER_SPAWN: u32 = 3;

/// Column where debug enemies spawn (near enemy fortress side of combat zone).
const DEBUG_SPAWN_COL: u16 = COMBAT_ZONE_START_COL + COMBAT_ZONE_COLS - 5; // col 75

fn debug_spawn_enemies(
    keyboard: Res<ButtonInput<KeyCode>>,
    unit_assets: Res<UnitAssets>,
    mut commands: Commands,
) {
    if !keyboard.just_pressed(KeyCode::KeyE) {
        return;
    }

    for i in 0..ENEMIES_PER_SPAWN {
        // Spread across rows: 2, 5, 8
        #[allow(clippy::cast_possible_truncation)]
        let row = i as u16 * 3 + 2;
        let spawn_x = col_to_world_x(DEBUG_SPAWN_COL);
        let spawn_y = row_to_world_y(row);

        commands.spawn((
            Unit,
            Team::Enemy,
            Target,
            CurrentTarget(None),
            Health::new(SOLDIER_HEALTH),
            CombatStats {
                damage: SOLDIER_DAMAGE,
                attack_speed: SOLDIER_ATTACK_SPEED,
                range: SOLDIER_ATTACK_RANGE,
            },
            Movement {
                speed: SOLDIER_MOVE_SPEED,
            },
            AttackTimer(Timer::from_seconds(
                1.0 / SOLDIER_ATTACK_SPEED,
                TimerMode::Repeating,
            )),
            Mesh2d(unit_assets.mesh.clone()),
            MeshMaterial2d(unit_assets.enemy_material.clone()),
            Transform::from_xyz(spawn_x, spawn_y, Z_UNIT),
            DespawnOnExit(GameState::InGame),
        ));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        debug_spawn_enemies
            .in_set(crate::GameSet::Input)
            .run_if(in_state(GameState::InGame).and(in_state(Menu::None))),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::units::UNIT_RADIUS;
    use crate::testing::assert_entity_count;
    use pretty_assertions::assert_eq;

    fn create_dev_tools_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<Assets<Mesh>>();
        app.init_resource::<Assets<ColorMaterial>>();

        // Create UnitAssets manually (normally done by units plugin on OnEnter)
        let mesh = app
            .world_mut()
            .resource_mut::<Assets<Mesh>>()
            .add(Circle::new(UNIT_RADIUS));
        let player_material = app
            .world_mut()
            .resource_mut::<Assets<ColorMaterial>>()
            .add(Color::srgb(0.2, 0.8, 0.2));
        let enemy_material = app
            .world_mut()
            .resource_mut::<Assets<ColorMaterial>>()
            .add(Color::srgb(0.8, 0.2, 0.2));
        app.insert_resource(UnitAssets {
            mesh,
            player_material,
            enemy_material,
        });

        app.add_systems(Update, debug_spawn_enemies);
        app
    }

    #[test]
    fn pressing_e_spawns_enemy_units() {
        let mut app = create_dev_tools_test_app();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        app.update();

        assert_entity_count::<(With<Unit>, With<Team>)>(&mut app, 3);
    }

    #[test]
    fn enemies_have_correct_components() {
        let mut app = create_dev_tools_test_app();

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        app.update();

        assert_entity_count::<(With<Unit>, With<Target>)>(&mut app, 3);
        assert_entity_count::<(With<Unit>, With<CurrentTarget>)>(&mut app, 3);
        assert_entity_count::<(With<Unit>, With<Health>)>(&mut app, 3);
        assert_entity_count::<(With<Unit>, With<CombatStats>)>(&mut app, 3);
        assert_entity_count::<(With<Unit>, With<Movement>)>(&mut app, 3);

        // All should be enemy team
        let mut query = app.world_mut().query_filtered::<&Team, With<Unit>>();
        for team in query.iter(app.world()) {
            assert_eq!(*team, Team::Enemy);
        }
    }
}

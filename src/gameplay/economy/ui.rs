//! Gold HUD display.

use bevy::prelude::*;

use super::Gold;
use crate::screens::GameState;
use crate::{GameSet, gameplay_running};

/// Marker for the gold display text entity.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
struct GoldDisplay;

fn spawn_gold_hud(mut commands: Commands) {
    commands.spawn((
        Name::new("Gold Display"),
        Text::new(format!("Gold: {}", super::STARTING_GOLD)),
        TextFont {
            font_size: 28.0,
            ..default()
        },
        TextColor(crate::theme::palette::GOLD_TEXT),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            right: Val::Px(10.0),
            ..default()
        },
        GoldDisplay,
        DespawnOnExit(GameState::InGame),
    ));
}

fn update_gold_display(gold: Res<Gold>, mut query: Single<&mut Text, With<GoldDisplay>>) {
    if gold.is_changed() {
        **query = Text::new(format!("Gold: {}", gold.0));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.register_type::<GoldDisplay>();

    app.add_systems(OnEnter(GameState::InGame), spawn_gold_hud);
    app.add_systems(
        Update,
        update_gold_display
            .in_set(GameSet::Ui)
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::GoldDisplay;
    use crate::gameplay::economy::Gold;

    #[test]
    fn gold_display_updates_on_change() {
        let mut app = crate::testing::create_test_app();
        app.init_resource::<Gold>();
        app.add_systems(Update, super::update_gold_display);

        // Spawn a GoldDisplay entity
        app.world_mut().spawn((Text::new("Gold: 0"), GoldDisplay));
        app.update();

        // Change gold
        app.world_mut().resource_mut::<Gold>().0 = 999;
        app.update();

        // Verify text updated
        let text = app
            .world_mut()
            .query_filtered::<&Text, With<GoldDisplay>>()
            .single(app.world())
            .unwrap();
        assert_eq!(**text, "Gold: 999");
    }
}

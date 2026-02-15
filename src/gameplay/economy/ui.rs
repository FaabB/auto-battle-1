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

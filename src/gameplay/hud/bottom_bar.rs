//! Bottom bar layout: Gold (left) | Cards + Reroll (center) | Timer + Minimap (right).

use bevy::prelude::*;

use super::elapsed_time::ElapsedTimeDisplay;
use crate::gameplay::GameStartTime;
use crate::gameplay::economy::STARTING_GOLD;
use crate::gameplay::economy::shop::HAND_SIZE;
use crate::gameplay::economy::shop_ui::{
    CardCostText, CardNameText, CardSlot, RerollButton, RerollCostText,
};
use crate::gameplay::economy::ui::GoldDisplay;
use crate::screens::GameState;
use crate::theme::palette;

// === Layout Constants ===

const CARD_WIDTH: f32 = 120.0;
const CARD_HEIGHT: f32 = 80.0;
const CARD_GAP: f32 = 10.0;
const BAR_PADDING: f32 = 12.0;
const MINIMAP_SIZE: f32 = 80.0;

/// Logical height of the bottom bar (padding top + tallest child + padding bottom).
/// Used by the camera to restrict its viewport to the area above the bar.
pub const BOTTOM_BAR_HEIGHT: f32 = BAR_PADDING * 2.0 + CARD_HEIGHT;

/// Spawns the full-width bottom bar on entering `InGame`.
fn spawn_bottom_bar(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    mut start: ResMut<GameStartTime>,
) {
    // Record game start time for elapsed timer
    start.0 = time.elapsed_secs();

    commands.spawn((
        Name::new("Bottom Bar"),
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(0.0),
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Auto,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            padding: UiRect::all(Val::Px(BAR_PADDING)),
            column_gap: Val::Px(BAR_PADDING),
            ..default()
        },
        BackgroundColor(palette::BOTTOM_BAR_BACKGROUND),
        DespawnOnExit(GameState::InGame),
        children![
            // === Left section: Gold ===
            (
                Name::new("Bar Left"),
                Node {
                    flex_grow: 1.0,
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![(
                    Name::new("Gold Display"),
                    GoldDisplay,
                    Node {
                        min_width: Val::Px(200.0),
                        ..default()
                    },
                    Text::new(format!("Gold: {STARTING_GOLD}")),
                    TextFont::from_font_size(palette::FONT_SIZE_HUD),
                    TextColor(palette::GOLD_TEXT),
                )],
            ),
            // === Center section: Cards + Reroll ===
            center_section(),
            // === Right section: Timer + Minimap ===
            (
                Name::new("Bar Right"),
                Node {
                    flex_grow: 1.0,
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(CARD_GAP),
                    ..default()
                },
                children![
                    // Elapsed time
                    (
                        Name::new("Elapsed Time"),
                        ElapsedTimeDisplay,
                        Text::new("00:00"),
                        TextFont::from_font_size(palette::FONT_SIZE_HUD),
                        TextColor(palette::BODY_TEXT),
                    ),
                    // Minimap placeholder
                    (
                        Name::new("Minimap Placeholder"),
                        Node {
                            width: Val::Px(MINIMAP_SIZE),
                            height: Val::Px(MINIMAP_SIZE),
                            border: UiRect::all(Val::Px(1.0)),
                            ..default()
                        },
                        BackgroundColor(palette::COMBAT_ZONE),
                        BorderColor::all(palette::PANEL_BORDER),
                    ),
                ],
            ),
        ],
    ));
}

/// Build the center section with 4 card slots + reroll button.
fn center_section() -> impl Bundle {
    (
        Name::new("Bar Center"),
        Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(CARD_GAP),
            ..default()
        },
        Children::spawn(SpawnWith(|parent: &mut ChildSpawner| {
            // 4 card slots
            for i in 0..HAND_SIZE {
                parent
                    .spawn((
                        Name::new(format!("Card Slot {i}")),
                        CardSlot(i),
                        Button,
                        Node {
                            width: Val::Px(CARD_WIDTH),
                            height: Val::Px(CARD_HEIGHT),
                            flex_direction: FlexDirection::Column,
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(4.0),
                            ..default()
                        },
                        BackgroundColor(palette::CARD_BACKGROUND),
                    ))
                    .with_children(|card| {
                        card.spawn((
                            Name::new(format!("Card {i} Name")),
                            CardNameText(i),
                            Text::new("—"),
                            TextFont::from_font_size(palette::FONT_SIZE_BODY),
                            TextColor(palette::HEADER_TEXT),
                        ));
                        card.spawn((
                            Name::new(format!("Card {i} Cost")),
                            CardCostText(i),
                            Text::new(""),
                            TextFont::from_font_size(palette::FONT_SIZE_SMALL),
                            TextColor(palette::GOLD_TEXT),
                        ));
                    });
            }

            // Reroll button
            parent
                .spawn((
                    Name::new("Reroll Button"),
                    RerollButton,
                    Button,
                    Node {
                        width: Val::Px(90.0),
                        height: Val::Px(CARD_HEIGHT),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BackgroundColor(palette::REROLL_BACKGROUND),
                ))
                .with_children(|btn| {
                    btn.spawn((
                        Name::new("Reroll Text"),
                        RerollCostText,
                        Text::new("Reroll\nFREE"),
                        TextFont::from_font_size(palette::FONT_SIZE_SMALL),
                        TextColor(palette::HEADER_TEXT),
                        TextLayout::new_with_justify(Justify::Center),
                    ));
                });
        })),
    )
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameState::InGame), spawn_bottom_bar);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::assert_entity_count;

    fn create_bottom_bar_test_app() -> App {
        let mut app = crate::testing::create_base_test_app();
        crate::testing::init_economy_resources(&mut app);
        app.init_resource::<GameStartTime>();
        app.add_plugins(super::super::plugin);
        crate::testing::transition_to_ingame(&mut app);
        app
    }

    #[test]
    fn bottom_bar_spawned_on_enter_ingame() {
        let mut app = create_bottom_bar_test_app();
        // The bottom bar root entity has the "Bottom Bar" name
        let mut query = app.world_mut().query_filtered::<&Name, With<Node>>();
        let has_bar = query
            .iter(app.world())
            .any(|name| name.as_str() == "Bottom Bar");
        assert!(has_bar, "Bottom bar should be spawned on InGame enter");
    }

    #[test]
    fn bottom_bar_has_gold_display() {
        let mut app = create_bottom_bar_test_app();
        assert_entity_count::<With<GoldDisplay>>(&mut app, 1);
    }

    #[test]
    fn bottom_bar_has_elapsed_time_display() {
        let mut app = create_bottom_bar_test_app();
        assert_entity_count::<With<ElapsedTimeDisplay>>(&mut app, 1);
    }

    #[test]
    fn bottom_bar_has_four_card_slots() {
        let mut app = create_bottom_bar_test_app();
        assert_entity_count::<With<CardSlot>>(&mut app, 4);
    }

    #[test]
    fn bottom_bar_has_reroll_button() {
        let mut app = create_bottom_bar_test_app();
        assert_entity_count::<With<RerollButton>>(&mut app, 1);
    }

    #[test]
    fn bottom_bar_height_constant_is_positive() {
        assert!(BOTTOM_BAR_HEIGHT > 0.0);
    }
}

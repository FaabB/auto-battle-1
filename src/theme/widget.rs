//! Reusable UI widget constructors.

use bevy::ecs::hierarchy::ChildSpawner;
use bevy::ecs::spawn::SpawnWith;
use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;

use super::interaction::InteractionPalette;
use super::palette;

/// Full-screen flex container that centers its children.
/// Use as root for menus and overlays.
pub fn ui_root(name: impl Into<std::borrow::Cow<'static, str>>) -> impl Bundle {
    (
        Name::new(name),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(20.0),
            ..default()
        },
    )
}

/// Large header text (title size, white).
pub fn header(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: palette::FONT_SIZE_HEADER,
            ..default()
        },
        TextColor(palette::HEADER_TEXT),
    )
}

/// Medium label text (label size, gray).
#[allow(dead_code)] // Used in future phases.
pub fn label(text: impl Into<String>) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font_size: palette::FONT_SIZE_LABEL,
            ..default()
        },
        TextColor(palette::BODY_TEXT),
    )
}

/// Clickable button with text and an observer-based action.
/// Uses the foxtrot pattern: outer wrapper + inner Button with `InteractionPalette`.
pub fn button<E, B, M, I>(text: impl Into<String>, action: I) -> impl Bundle
where
    E: EntityEvent,
    B: Bundle,
    I: IntoObserverSystem<E, B, M>,
{
    let text = text.into();
    let action = IntoObserverSystem::into_system(action);
    (
        Name::new("Button"),
        Node::default(),
        Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
            parent
                .spawn((
                    Name::new("Button Inner"),
                    Button,
                    Node {
                        width: Val::Px(300.0),
                        height: Val::Px(60.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BackgroundColor(palette::BUTTON_BACKGROUND),
                    BorderColor::all(palette::PANEL_BORDER),
                    InteractionPalette {
                        none: palette::BUTTON_BACKGROUND,
                        hovered: palette::BUTTON_HOVERED_BACKGROUND,
                        pressed: palette::BUTTON_PRESSED_BACKGROUND,
                    },
                    children![(
                        Text(text),
                        TextFont::from_font_size(palette::FONT_SIZE_LABEL),
                        TextColor(palette::BUTTON_TEXT),
                        Pickable::IGNORE,
                    )],
                ))
                .observe(action);
        })),
    )
}

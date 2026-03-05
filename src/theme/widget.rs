//! Reusable UI widget constructors.

use bevy::ecs::hierarchy::ChildSpawner;
use bevy::ecs::spawn::SpawnWith;
use bevy::ecs::system::IntoObserverSystem;
use bevy::input_focus::InputFocus;
use bevy::input_focus::InputFocusVisible;
use bevy::input_focus::tab_navigation::{NavAction, TabNavigation};
use bevy::prelude::*;

use super::interaction::InteractionPalette;
use super::palette;

/// Custom entity event fired when a button is activated (click or keyboard Enter/Space).
#[derive(EntityEvent, Clone, Debug, Reflect)]
pub struct Activate(pub Entity);

pub fn plugin(app: &mut App) {
    app.register_type::<Activate>();
    app.add_systems(
        Update,
        (
            keyboard_confirm_focused,
            arrow_key_navigation,
            update_focus_outline,
        ),
    );
}

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

/// Clickable button with text, keyboard navigation support, and an observer-based action.
/// The action observes `Activate`, which fires on both mouse click and keyboard Enter/Space.
pub fn button<B, M, I>(
    text: impl Into<String>,
    tab_index: i32,
    auto_focus: bool,
    action: I,
) -> impl Bundle
where
    B: Bundle,
    I: IntoObserverSystem<Activate, B, M>,
{
    let text = text.into();
    let action = IntoObserverSystem::into_system(action);
    (
        Name::new("Button"),
        Node::default(),
        Children::spawn(SpawnWith(move |parent: &mut ChildSpawner| {
            let mut inner = parent.spawn((
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
                Outline::default(),
                bevy::input_focus::tab_navigation::TabIndex(tab_index),
                children![(
                    Text(text),
                    TextFont::from_font_size(palette::FONT_SIZE_LABEL),
                    TextColor(palette::BUTTON_TEXT),
                    Pickable::IGNORE,
                )],
            ));
            // Bridge: Pointer<Click> → Activate
            inner.observe(|click: On<Pointer<Click>>, mut commands: Commands| {
                commands.entity(click.entity).trigger(Activate);
            });
            // User-provided action
            inner.observe(action);
            if auto_focus {
                inner.insert(bevy::input_focus::AutoFocus);
            }
        })),
    )
}

/// Fire `Activate` on the focused button when Enter or Space is pressed.
fn keyboard_confirm_focused(
    input: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    if input.just_pressed(KeyCode::Enter) || input.just_pressed(KeyCode::Space) {
        if let Some(entity) = focus.0 {
            commands.entity(entity).trigger(Activate);
        }
    }
}

/// Arrow Up/Down mapped to `TabNavigation` Previous/Next.
fn arrow_key_navigation(
    input: Res<ButtonInput<KeyCode>>,
    nav: TabNavigation,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
) {
    let action = if input.just_pressed(KeyCode::ArrowUp) {
        Some(NavAction::Previous)
    } else if input.just_pressed(KeyCode::ArrowDown) {
        Some(NavAction::Next)
    } else {
        None
    };

    if let Some(action) = action {
        if let Ok(next) = nav.navigate(&focus, action) {
            focus.0 = Some(next);
            focus_visible.0 = true;
        }
    }
}

/// Show/hide outline on the focused button entity.
fn update_focus_outline(
    focus: Res<InputFocus>,
    mut buttons: Query<(Entity, &mut Outline), With<Button>>,
) {
    for (entity, mut outline) in &mut buttons {
        if focus.0 == Some(entity) {
            outline.width = Val::Px(2.0);
            outline.offset = Val::Px(2.0);
            outline.color = palette::BUTTON_FOCUS_OUTLINE;
        } else {
            outline.color = Color::NONE;
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::hierarchy::ChildOf;
    use bevy::input_focus::InputFocus;
    use bevy::input_focus::tab_navigation::{TabGroup, TabIndex};
    use bevy::prelude::*;

    use super::palette;

    fn create_focus_test_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<ButtonInput<KeyCode>>();
        app.init_resource::<InputFocus>();
        app.init_resource::<bevy::input_focus::InputFocusVisible>();
        app
    }

    /// Spawn a tab group with N button children that have TabIndex(0..N) and Outline::default().
    /// Returns (group_entity, vec of button entities).
    fn spawn_tab_group(world: &mut World, count: usize) -> (Entity, Vec<Entity>) {
        let group = world.spawn(TabGroup::new(0)).id();
        let mut buttons = Vec::new();
        for i in 0..count {
            let btn = world
                .spawn((
                    Button,
                    TabIndex(i as i32),
                    Outline::default(),
                    ChildOf(group),
                ))
                .id();
            buttons.push(btn);
        }
        (group, buttons)
    }

    #[test]
    fn focus_outline_shown_on_focused_button() {
        let mut app = create_focus_test_app();
        app.add_systems(Update, super::update_focus_outline);

        let (_group, buttons) = spawn_tab_group(app.world_mut(), 2);
        app.world_mut()
            .insert_resource(InputFocus(Some(buttons[0])));
        app.update();

        let outline_0 = app.world().get::<Outline>(buttons[0]).unwrap();
        assert_eq!(outline_0.color, palette::BUTTON_FOCUS_OUTLINE);

        let outline_1 = app.world().get::<Outline>(buttons[1]).unwrap();
        assert_eq!(outline_1.color, Color::NONE);
    }

    #[test]
    fn focus_outline_hidden_when_no_focus() {
        let mut app = create_focus_test_app();
        app.add_systems(Update, super::update_focus_outline);

        let (_group, buttons) = spawn_tab_group(app.world_mut(), 1);
        // No focus set (default is None)
        app.update();

        let outline = app.world().get::<Outline>(buttons[0]).unwrap();
        assert_eq!(outline.color, Color::NONE);
    }

    #[test]
    fn arrow_down_cycles_focus_to_next_button() {
        let mut app = create_focus_test_app();
        app.add_systems(Update, super::arrow_key_navigation);

        let (_group, buttons) = spawn_tab_group(app.world_mut(), 2);
        app.world_mut()
            .insert_resource(InputFocus(Some(buttons[0])));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowDown);
        app.update();

        let focus = app.world().resource::<InputFocus>();
        assert_eq!(focus.0, Some(buttons[1]));
    }

    #[test]
    fn arrow_up_wraps_from_first_to_last() {
        let mut app = create_focus_test_app();
        app.add_systems(Update, super::arrow_key_navigation);

        let (_group, buttons) = spawn_tab_group(app.world_mut(), 2);
        app.world_mut()
            .insert_resource(InputFocus(Some(buttons[0])));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowUp);
        app.update();

        let focus = app.world().resource::<InputFocus>();
        assert_eq!(focus.0, Some(buttons[1]));
    }

    #[test]
    fn arrow_down_wraps_from_last_to_first() {
        let mut app = create_focus_test_app();
        app.add_systems(Update, super::arrow_key_navigation);

        let (_group, buttons) = spawn_tab_group(app.world_mut(), 2);
        app.world_mut()
            .insert_resource(InputFocus(Some(buttons[1])));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowDown);
        app.update();

        let focus = app.world().resource::<InputFocus>();
        assert_eq!(focus.0, Some(buttons[0]));
    }

    #[test]
    fn keyboard_enter_triggers_activate_on_focused_button() {
        let mut app = create_focus_test_app();
        app.add_systems(Update, super::keyboard_confirm_focused);

        // Spawn a button that sets a resource when activated
        #[derive(Resource, Default)]
        struct Activated(bool);
        app.init_resource::<Activated>();

        let btn = app.world_mut().spawn(Button).id();
        app.world_mut().entity_mut(btn).observe(
            |_: On<super::Activate>, mut activated: ResMut<Activated>| {
                activated.0 = true;
            },
        );

        app.world_mut().insert_resource(InputFocus(Some(btn)));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();

        assert!(app.world().resource::<Activated>().0);
    }

    #[test]
    fn keyboard_space_triggers_activate_on_focused_button() {
        let mut app = create_focus_test_app();
        app.add_systems(Update, super::keyboard_confirm_focused);

        #[derive(Resource, Default)]
        struct Activated(bool);
        app.init_resource::<Activated>();

        let btn = app.world_mut().spawn(Button).id();
        app.world_mut().entity_mut(btn).observe(
            |_: On<super::Activate>, mut activated: ResMut<Activated>| {
                activated.0 = true;
            },
        );

        app.world_mut().insert_resource(InputFocus(Some(btn)));
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        app.update();

        assert!(app.world().resource::<Activated>().0);
    }

    #[test]
    fn no_activate_when_no_focus() {
        let mut app = create_focus_test_app();
        app.add_systems(Update, super::keyboard_confirm_focused);

        #[derive(Resource, Default)]
        struct Activated(bool);
        app.init_resource::<Activated>();

        let btn = app.world_mut().spawn(Button).id();
        app.world_mut().entity_mut(btn).observe(
            |_: On<super::Activate>, mut activated: ResMut<Activated>| {
                activated.0 = true;
            },
        );

        // No focus set
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();

        assert!(!app.world().resource::<Activated>().0);
    }
}

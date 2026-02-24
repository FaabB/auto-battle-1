//! Button hover/press visual feedback.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui::Pressed;

/// Defines colors for none/hovered/pressed button states.
/// Add alongside `Button` and `BackgroundColor` on clickable UI elements.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
#[require(Hovered)]
pub struct InteractionPalette {
    pub none: Color,
    pub hovered: Color,
    pub pressed: Color,
}

fn apply_interaction_palette(
    mut palette_query: Query<
        (
            Has<Pressed>,
            &Hovered,
            &InteractionPalette,
            &mut BackgroundColor,
        ),
        Changed<Interaction>,
    >,
) {
    for (pressed, Hovered(hovered), palette, mut background) in &mut palette_query {
        *background = match (pressed, hovered) {
            (true, _) => palette.pressed,
            (false, true) => palette.hovered,
            (false, false) => palette.none,
        }
        .into();
    }
}

pub fn plugin(app: &mut App) {
    app.register_type::<InteractionPalette>();
    app.add_systems(Update, apply_interaction_palette);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_sets_none_color_by_default() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_systems(Update, apply_interaction_palette);

        let none_color = Color::srgb(1.0, 0.0, 0.0);
        app.world_mut().spawn((
            Button,
            BackgroundColor(Color::BLACK),
            InteractionPalette {
                none: none_color,
                hovered: Color::srgb(0.0, 1.0, 0.0),
                pressed: Color::srgb(0.0, 0.0, 1.0),
            },
            Interaction::None,
        ));
        app.update();

        let mut query = app.world_mut().query::<&BackgroundColor>();
        let bg = query.single(app.world()).unwrap();
        assert_eq!(bg.0, none_color);
    }
}

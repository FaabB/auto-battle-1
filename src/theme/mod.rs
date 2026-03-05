//! Shared UI theme: color palette, interaction feedback, and reusable widget constructors.

use bevy::input_focus::InputDispatchPlugin;
use bevy::input_focus::tab_navigation::TabNavigationPlugin;

pub mod interaction;
pub mod palette;
pub mod widget;

pub fn plugin(app: &mut bevy::prelude::App) {
    app.add_plugins((
        InputDispatchPlugin,
        TabNavigationPlugin,
        interaction::plugin,
        widget::plugin,
    ));
}

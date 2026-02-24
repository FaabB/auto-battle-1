//! Shared UI theme: color palette, interaction feedback, and reusable widget constructors.

pub mod interaction;
pub mod palette;
pub mod widget;

pub fn plugin(app: &mut bevy::prelude::App) {
    app.add_plugins(interaction::plugin);
}

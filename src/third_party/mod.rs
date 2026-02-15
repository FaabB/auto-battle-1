//! Third-party plugin isolation.

mod avian;

pub fn plugin(app: &mut bevy::prelude::App) {
    app.add_plugins(avian::plugin);
}

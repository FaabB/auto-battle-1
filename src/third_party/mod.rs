//! Third-party plugin isolation.

mod avian;

pub use avian::{CollisionLayer, surface_distance};

pub fn plugin(app: &mut bevy::prelude::App) {
    app.add_plugins(avian::plugin);
}

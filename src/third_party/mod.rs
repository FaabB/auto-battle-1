//! Third-party plugin isolation.

mod avian;

#[cfg(test)]
pub use avian::surface_distance;
pub use avian::{CollisionLayer, solid_entity_layers};

pub fn plugin(app: &mut bevy::prelude::App) {
    app.add_plugins(avian::plugin);
}

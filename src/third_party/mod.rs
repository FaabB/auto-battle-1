//! Third-party plugin isolation.

mod avian;
mod vleue_navigator;

pub use self::vleue_navigator::NavObstacle;
pub use avian::{CollisionLayer, surface_distance};

pub fn plugin(app: &mut bevy::prelude::App) {
    app.add_plugins((avian::plugin, vleue_navigator::plugin));
}

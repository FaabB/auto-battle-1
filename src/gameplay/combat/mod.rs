//! Combat systems: attack mechanics, death detection, and health bars.

mod attack;
mod death;
mod health_bar;

#[allow(unused_imports)]
// Hitbox re-exported for external use (currently only used within combat)
pub use attack::{AttackTimer, Hitbox};
pub use death::DeathCheck;
pub use health_bar::{
    HealthBarConfig, UNIT_HEALTH_BAR_HEIGHT, UNIT_HEALTH_BAR_WIDTH, UNIT_HEALTH_BAR_Y_OFFSET,
};

use bevy::prelude::*;

pub(super) fn plugin(app: &mut App) {
    attack::plugin(app);
    death::plugin(app);
    health_bar::plugin(app);
}

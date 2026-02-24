//! Gold HUD display update system.
//!
//! Spawning is handled by `gameplay/hud/bottom_bar.rs`.

use bevy::prelude::*;

use super::Gold;
use crate::{GameSet, gameplay_running};

/// Marker for the gold display text entity.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct GoldDisplay;

fn update_gold_display(gold: Res<Gold>, mut query: Single<&mut Text, With<GoldDisplay>>) {
    if gold.is_changed() {
        **query = Text::new(format!("Gold: {}", gold.0));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.register_type::<GoldDisplay>();

    app.add_systems(
        Update,
        update_gold_display
            .in_set(GameSet::Ui)
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::GoldDisplay;
    use crate::gameplay::economy::Gold;

    #[test]
    fn gold_display_updates_on_change() {
        let mut app = crate::testing::create_test_app();
        app.init_resource::<Gold>();
        app.add_systems(Update, super::update_gold_display);

        app.world_mut().spawn((Text::new("Gold: 0"), GoldDisplay));
        app.update();

        app.world_mut().resource_mut::<Gold>().0 = 999;
        app.update();

        let text = app
            .world_mut()
            .query_filtered::<&Text, With<GoldDisplay>>()
            .single(app.world())
            .unwrap();
        assert_eq!(**text, "Gold: 999");
    }
}

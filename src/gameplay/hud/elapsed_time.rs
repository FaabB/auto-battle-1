//! Elapsed game time display.

use bevy::prelude::*;

use crate::gameplay::GameStartTime;
use crate::{GameSet, gameplay_running};

/// Marker for the elapsed time text in the bottom bar.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct ElapsedTimeDisplay;

fn update_elapsed_time(
    time: Res<Time<Virtual>>,
    start: Res<GameStartTime>,
    mut query: Single<&mut Text, With<ElapsedTimeDisplay>>,
) {
    let elapsed = time.elapsed_secs() - start.0;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let total_secs = elapsed as u32;
    let minutes = total_secs / 60;
    let seconds = total_secs % 60;
    **query = Text::new(format!("{minutes:02}:{seconds:02}"));
}

pub(super) fn plugin(app: &mut App) {
    app.register_type::<ElapsedTimeDisplay>();

    app.add_systems(
        Update,
        update_elapsed_time
            .in_set(GameSet::Ui)
            .run_if(gameplay_running),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elapsed_time_formats_correctly() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<GameStartTime>();
        app.add_systems(Update, update_elapsed_time);

        app.world_mut()
            .spawn((Text::new("00:00"), ElapsedTimeDisplay));

        // Advance virtual time by running several updates.
        // GameStartTime defaults to 0.0 and Time<Virtual>.elapsed_secs()
        // increments with wall-clock delta. After a few updates,
        // elapsed will still be ~0s, so text should remain "00:00".
        app.update();

        let mut query = app
            .world_mut()
            .query_filtered::<&Text, With<ElapsedTimeDisplay>>();
        let text = query.single(app.world()).unwrap();
        assert_eq!(**text, "00:00");
    }
}

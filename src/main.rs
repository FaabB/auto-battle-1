//! Auto-battle game entry point.

use auto_battle::battlefield::BattlefieldPlugin;
use auto_battle::game::GamePlugin;
use auto_battle::prelude::*;
use auto_battle::screens::{InGamePlugin, LoadingScreenPlugin, MainMenuPlugin};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "Auto Battle".to_string(),
                        resolution: (1920, 1080).into(),
                        resizable: true,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
        )
        .add_plugins((
            GamePlugin,
            LoadingScreenPlugin,
            MainMenuPlugin,
            InGamePlugin,
            BattlefieldPlugin,
        ))
        .run();
}

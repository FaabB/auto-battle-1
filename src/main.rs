//! Auto-battle game entry point.

use auto_battle::battlefield::BattlefieldPlugin;
use auto_battle::building::BuildingPlugin;
use auto_battle::game::CoreGamePlugin;
use auto_battle::prelude::*;
use auto_battle::screens::{InGameScreenPlugin, LoadingScreenPlugin, MainMenuScreenPlugin};

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
            CoreGamePlugin,
            LoadingScreenPlugin,
            MainMenuScreenPlugin,
            InGameScreenPlugin,
            BattlefieldPlugin,
            BuildingPlugin,
        ))
        .run();
}

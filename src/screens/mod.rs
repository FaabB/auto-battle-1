//! Screen plugins for each game state.

mod in_game;
mod loading;
mod main_menu;
mod paused;

pub use in_game::InGamePlugin;
pub use loading::LoadingScreenPlugin;
pub use main_menu::MainMenuPlugin;
pub use paused::PausedPlugin;

//! Screen plugins for each game state.

mod in_game;
mod loading;
mod main_menu;

pub use in_game::InGamePlugin;
pub use loading::LoadingScreenPlugin;
pub use main_menu::MainMenuPlugin;

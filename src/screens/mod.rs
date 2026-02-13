//! Screen plugins for each game state.

mod in_game;
mod loading;
mod main_menu;

pub use in_game::InGameScreenPlugin;
pub use loading::LoadingScreenPlugin;
pub use main_menu::MainMenuScreenPlugin;

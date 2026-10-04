//! The companion app without a screen: what is shown (the app's state and what it does with keys and with time) and the logic behind
//! it (build search, hunt plan, cheapest routes...). A screen draws `app::App` and sends it keys; `mh3u-tui` is one such screen.
//! See `docs/architecture.md`.

pub mod app;
pub mod builds;
pub mod commands;
pub mod compare;
pub mod families;
pub mod files;
pub mod gains;
pub mod hunts;
pub mod input;
pub mod search;
pub mod select;
pub mod surplus;
pub mod templates;
pub mod tree;
pub mod unlocked;
pub mod upgrade_path;
pub mod worn;
pub mod zenny;

use std::path::PathBuf;

/// The game's title id (US).
pub const TITLE_ID: &str = "0005000010118300";

/// Look for a `... [Game] [0005000010118300]` folder under ~/games/wiiu.
pub fn guess_game_dir() -> Option<PathBuf> {
    let dir = dirs::home_dir()?.join("games/wiiu");
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok().map(|e| e.path())).find(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(TITLE_ID) && n.contains("[Game]"))
    })
}

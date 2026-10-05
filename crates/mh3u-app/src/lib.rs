//! The companion app without a screen: what is shown (the app's state and what it does with keys and with time) and the logic behind
//! it (build search, hunt plan, cheapest routes...). A screen draws `app::App` and sends it keys; `mh3u-tui` is one such screen.
//! See `docs/architecture.md`.

pub mod app;
pub mod builds;
pub mod changes;
pub mod commands;
pub mod compare;
pub mod config;
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

/// The game's title id (US).
pub const TITLE_ID: &str = mh3u_core::gamedata::TITLE_ID;

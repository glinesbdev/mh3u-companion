//! Parsing for Monster Hunter 3 Ultimate (Wii U) save data. All values are big-endian.

/// Bytes of a personal save snapshot under `snapshots/` (git-ignored), or returns from the calling test, which is skipped, if
/// the file isn't there.
#[cfg(test)]
macro_rules! fixture {
    ($path:expr) => {
        match std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../snapshots/", $path)) {
            Ok(bytes) => bytes,
            Err(_) => {
                eprintln!("skipped: snapshots/{} is not available", $path);
                return;
            }
        }
    };
}

pub mod arc;
pub mod armor;
pub mod barsearch;
pub mod blacksmith;
pub mod breakparts;
pub mod debug_edits;
pub mod decorations;
pub mod diff;
pub mod drops;
pub mod edit;
pub mod gamedata;
pub mod gather_spots;
pub mod gmd;
pub mod hitzones;
pub mod horn_songs;
pub mod item_extras;
pub mod items;
pub mod live;
pub mod livesave;
pub mod notes;
pub mod online;
pub mod prices;
pub mod procmem;
pub mod quest;
pub mod recipes;
pub mod rpx;
pub mod save;
pub mod shopscan;
pub mod skilltiers;
pub mod weapon_extras;
pub mod weapons;
pub mod weaponskills;
pub mod zone_names;

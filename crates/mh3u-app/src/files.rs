//! Small text files kept between sessions (wishlist, builds, templates, price ledger, unlocked pieces).

use std::{
    io,
    path::{Path, PathBuf},
};

const APP: &str = "mh3u-companion";

/// Where the app keeps its files. The lists that belong to one hunter (wishlist, builds, templates) have a file per save slot, so
/// switching hunters switches them. Slot 1 keeps the plain names (`wishlist.txt`), so files made before there were several are
/// still found; the other slots are `wishlist-2.txt`, `wishlist-3.txt`. The price ledger, the pieces seen on offer (which holds all
/// hunters) and the tracker log are shared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Files {
    config: PathBuf,
    data: PathBuf,
    pub wishlist: PathBuf,
    pub builds: PathBuf,
    pub templates: PathBuf,
    pub prices: PathBuf,
    pub unlocked: PathBuf,
    pub tracker_log: PathBuf,
    /// The items picked up while playing live; one file per hunter, like the lists, but in the data folder.
    pub gains: PathBuf,
    /// The wishlist's shopping list, written out as text by the Wishlist tab's `e`; one per hunter, in the data folder.
    pub shopping: PathBuf,
    /// What the hunter held when the app last closed (see `changes`); one per hunter, in the data folder.
    pub last_seen: PathBuf,
    /// The settings file (`config.txt`, see `crate::config`); one for all hunters, in the config folder.
    pub settings: PathBuf,
    /// What the debug commands changed for this hunter (see `mh3u_core::debug_edits`); one per hunter, in the data folder.
    pub debug_edits: PathBuf,
    /// Where `--debug-edit` copies the saves before changing the game.
    pub backups: PathBuf,
}

impl Files {
    /// The usual places: the lists in the config folder (`$XDG_CONFIG_HOME` or `~/.config`), the rest in the data folder.
    /// `None` when the system names no home folder.
    pub fn for_slot(slot: u8) -> Option<Files> {
        Some(Files::in_dirs(&dirs::config_dir()?.join(APP), &dirs::data_dir()?.join(APP), slot))
    }

    /// The same files for another save slot's hunter.
    pub fn with_slot(&self, slot: u8) -> Files {
        Files::in_dirs(&self.config, &self.data, slot)
    }

    pub fn in_dirs(config: &Path, data: &Path, slot: u8) -> Files {
        let per_slot = |stem: &str| {
            config.join(if slot == 1 {
                format!("{stem}.txt")
            } else {
                format!("{stem}-{slot}.txt")
            })
        };
        Files {
            config: config.to_path_buf(),
            data: data.to_path_buf(),
            wishlist: per_slot("wishlist"),
            builds: per_slot("builds"),
            templates: per_slot("templates"),
            prices: data.join("prices.tsv"),
            unlocked: data.join("unlocked.tsv"),
            tracker_log: data.join("tracker.log"),
            gains: data.join(if slot == 1 {
                "gains.tsv".to_string()
            } else {
                format!("gains-{slot}.tsv")
            }),
            shopping: data.join(if slot == 1 {
                "shopping-list.txt".to_string()
            } else {
                format!("shopping-list-{slot}.txt")
            }),
            last_seen: data.join(if slot == 1 {
                "last-seen.txt".to_string()
            } else {
                format!("last-seen-{slot}.txt")
            }),
            settings: config.join("config.txt"),
            debug_edits: data.join(if slot == 1 {
                "debug-edits.txt".to_string()
            } else {
                format!("debug-edits-{slot}.txt")
            }),
            backups: data.join("backups"),
        }
    }

    /// The settings file in the usual place, before any save slot is known.
    pub fn settings_path() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join(APP).join("config.txt"))
    }

    /// Whether some hunter has debug edits on record, so the program must be able to take them out again.
    pub fn any_debug_edits(&self) -> bool {
        (1..=3).any(|slot| {
            std::fs::read_to_string(&self.with_slot(slot).debug_edits).is_ok_and(|t| !mh3u_core::debug_edits::Ledger::parse(&t).is_empty())
        })
    }
}

/// Which save slot a save file is: the number in a `userN` file name.
pub fn slot_of(save: &Path) -> Option<u8> {
    let n = save.file_name()?.to_str()?.strip_prefix("user")?.parse().ok()?;
    (1..=3).contains(&n).then_some(n)
}

/// Write `text` to `path`, creating the folder first. The error is worded for the status line, e.g. `could not save wishlist: ...`.
pub fn save(path: &Path, text: &str, what: &str) -> Result<(), String> {
    write(path, text).map_err(|e| format!("could not save {what}: {e}"))
}

fn write(path: &Path, text: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_slot_has_its_own_lists_and_slot_one_keeps_the_original_names() {
        let (config, data) = (Path::new("/c/app"), Path::new("/d/app"));
        let one = Files::in_dirs(config, data, 1);
        let two = Files::in_dirs(config, data, 2);
        assert_eq!(one.wishlist, Path::new("/c/app/wishlist.txt"));
        assert_eq!(two.wishlist, Path::new("/c/app/wishlist-2.txt"));
        assert_eq!(two.builds, Path::new("/c/app/builds-2.txt"));
        assert_eq!(two.templates, Path::new("/c/app/templates-2.txt"));
        assert_eq!(one.gains, Path::new("/d/app/gains.tsv"));
        assert_eq!(two.gains, Path::new("/d/app/gains-2.tsv"));
        assert_eq!(one.shopping, Path::new("/d/app/shopping-list.txt"));
        assert_eq!(two.shopping, Path::new("/d/app/shopping-list-2.txt"));
        assert_eq!(one.prices, two.prices, "the ledger is shared");
        assert_eq!(one.unlocked, Path::new("/d/app/unlocked.tsv"));
    }

    #[test]
    fn a_save_files_slot_is_the_number_in_its_name() {
        assert_eq!(slot_of(Path::new("/x/80000001/user2")), Some(2));
        assert_eq!(slot_of(Path::new("user3")), Some(3));
        assert_eq!(slot_of(Path::new("user4")), None);
        assert_eq!(slot_of(Path::new("system")), None);
        let two = Files::in_dirs(Path::new("/c/app"), Path::new("/d/app"), 2);
        assert_eq!(two.with_slot(3), Files::in_dirs(Path::new("/c/app"), Path::new("/d/app"), 3));
    }

    #[test]
    fn saving_creates_the_folder_and_words_errors_for_the_status_line() {
        let dir = std::env::temp_dir().join(format!("mh3u-files-test-{}", std::process::id()));
        let file = dir.join("deeper/list.txt");
        save(&file, "a\n", "list").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "a\n");
        let blocked = file.join("under-a-file.txt");
        let message = save(&blocked, "x", "list").unwrap_err();
        assert!(message.starts_with("could not save list: "), "{message}");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

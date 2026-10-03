//! Small text files kept between sessions (wishlist, builds, templates, price ledger, unlocked pieces).

use std::{io, path::Path};

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

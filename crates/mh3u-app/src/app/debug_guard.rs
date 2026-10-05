//! Keeping debug edits out of online play.
//!
//! The debug commands (`--debug-edit`) are for trying things out alone. Everything they change is noted per hunter
//! (`mh3u_core::debug_edits`), and when the game is online, or Cemu is set up to go online, the program takes it all out of the game's
//! data again and refuses further edits until the game is offline. `purge` does the same by hand. This is a safeguard for people who do
//! not want edited gear in online play; it cannot stop someone who changes this open source program or plays without it.

use super::*;
use mh3u_core::debug_edits::Ledger;
use mh3u_core::online;

/// How often the emulator is looked at for signs of being online.
const CHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(2);

/// What is known about the debug edits and whether the game is online.
#[derive(Default)]
pub struct DebugGuard {
    /// What the debug commands changed for the hunter on screen.
    pub ledger: Ledger,
    /// Why the game counts as online (`None`: it does not).
    pub online: Option<String>,
    checked: Option<Instant>,
    /// Where Cemu's settings are, when not in the usual place (for tests).
    pub(super) settings: Option<PathBuf>,
}

impl DebugGuard {
    pub(super) fn load(files: Option<&Files>) -> DebugGuard {
        let ledger = files
            .and_then(|f| std::fs::read_to_string(&f.debug_edits).ok())
            .map(|t| Ledger::parse(&t))
            .unwrap_or_default();
        DebugGuard {
            ledger,
            ..DebugGuard::default()
        }
    }

    /// Why the game counts as online now, from Cemu's settings and the emulator's connections.
    pub(super) fn reason(&self, emulator: Option<u32>) -> Option<String> {
        let settings = self.settings.clone().or_else(online::cemu_settings_path)?;
        if online::cemu_online_enabled(&settings) {
            return Some("Cemu has online play turned on".to_string());
        }
        emulator
            .filter(|&pid| online::process_is_online(pid))
            .map(|_| "the game is connected to the network".to_string())
    }
}

impl App {
    pub(super) fn save_ledger(&mut self) {
        let Some(path) = self.files.as_ref().map(|f| &f.debug_edits) else {
            return;
        };
        let text = self.guard.ledger.format();
        let result = if text.is_empty() {
            std::fs::remove_file(path)
                .or_else(|e| if e.kind() == std::io::ErrorKind::NotFound { Ok(()) } else { Err(e) })
                .map_err(|e| format!("could not clear the debug edits file: {e}"))
        } else {
            crate::files::save(path, &text, "debug edits")
        };
        if let Err(e) = result {
            self.status = e;
        }
    }

    /// Take everything the debug commands added out of the game's data, and say what was done.
    pub(super) fn purge_debug_edits(&mut self) -> String {
        let (Some(live), Some(data)) = (&self.live, self.console.live_bytes.clone()) else {
            return "taking edits out needs the game running through --live".to_string();
        };
        if !self.live_connected() {
            return "taking edits out needs a hunter loaded in the game".to_string();
        }
        if self.guard.ledger.is_empty() {
            return "no debug edits are recorded for this hunter".to_string();
        }
        let (patches, report) = self.guard.ledger.purge(&data);
        let mut edited = data;
        for patch in patches {
            mh3u_core::edit::apply(&mut edited, &patch);
            live.reader.write(patch);
        }
        if let Ok(save) = Save::parse(&edited) {
            self.costs.tracker.rebase(&save);
        }
        // what could not be taken out stays on record
        if report.kept.is_empty() {
            self.guard.ledger = Ledger::default();
        }
        self.save_ledger();
        report.summary()
    }

    /// Look at the emulator now and then: when the game is online (or may go online) the edits come out and editing stops.
    pub(super) fn watch_online(&mut self) {
        if self.guard.checked.is_some_and(|t| t.elapsed() < CHECK_EVERY) {
            return;
        }
        self.guard.checked = Some(Instant::now());
        let pid = self.live.as_ref().map(|l| l.child.id());
        let reason = self.guard.reason(pid);
        let was_online = self.guard.online.is_some();
        self.guard.online = reason.clone();
        let Some(reason) = reason else {
            if was_online {
                self.status = "the game is offline again: debug edits are allowed".to_string();
            }
            return;
        };
        if !was_online && !self.guard.ledger.is_empty() && self.live_connected() {
            let done = self.purge_debug_edits();
            self.status = format!("online ({reason}): {done}; debug edits are off while online");
        } else if !was_online && self.console.enabled {
            self.status = format!("online ({reason}): debug edits are off while online");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reason_comes_from_cemus_setting_first() {
        let dir = std::env::temp_dir().join(format!("mh3u-guard-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let settings = dir.join("settings.xml");
        let mut guard = DebugGuard {
            settings: Some(settings.clone()),
            ..DebugGuard::default()
        };
        assert_eq!(guard.reason(None), None, "no settings file: not online");
        std::fs::write(&settings, "<Account><OnlineEnabled>false</OnlineEnabled></Account>").unwrap();
        assert_eq!(guard.reason(None), None);
        assert_eq!(guard.reason(Some(std::process::id())), None, "this process holds no connection");
        std::fs::write(&settings, "<Account><OnlineEnabled>true</OnlineEnabled></Account>").unwrap();
        assert_eq!(guard.reason(None).as_deref(), Some("Cemu has online play turned on"));
        guard.settings = None;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_ledger_is_read_from_the_hunters_file() {
        let dir = std::env::temp_dir().join(format!("mh3u-guard-files-{}", std::process::id()));
        let files = Files::in_dirs(&dir.join("c"), &dir.join("d"), 2);
        assert!(DebugGuard::load(Some(&files)).ledger.is_empty());
        assert!(!files.any_debug_edits());
        std::fs::create_dir_all(dir.join("d")).unwrap();
        std::fs::write(&files.debug_edits, "zenny 500\n").unwrap();
        assert!(!DebugGuard::load(Some(&files)).ledger.is_empty());
        assert!(files.any_debug_edits(), "found from any slot's Files");
        assert!(files.with_slot(1).any_debug_edits());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

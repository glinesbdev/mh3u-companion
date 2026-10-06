//! Live mode: following the running game and reloading the save when the file changes.

use super::*;

impl App {
    /// Make `save` the current data. Returns a message if the zenny changed.
    /// Which save slot holds a hunter of this name: the slot files are the `userN` files beside the one in use. When two slots share a
    /// name the current slot wins, then the lowest.
    fn slot_of_hunter(&self, name: &str) -> Option<u8> {
        let dir = self.save_path.parent()?;
        let named = |slot: u8| {
            let bytes = std::fs::read(dir.join(format!("user{slot}"))).ok()?;
            Some(Save::parse(&bytes).ok()?.hunter_name == name)
        };
        self.slot
            .filter(|&s| named(s) == Some(true))
            .or_else(|| (1..=3).find(|&s| named(s) == Some(true)))
    }

    /// Show another hunter's data: their wishlist, skills, templates and save file. Called when the game loads a hunter other than
    /// the one on screen.
    pub(super) fn switch_profile(&mut self, slot: u8) {
        let Some(dir) = self.save_path.parent().map(std::path::Path::to_path_buf) else {
            return;
        };
        self.files = self.files.as_ref().map(|f| f.with_slot(slot));
        self.slot = Some(slot);
        self.save_path = dir.join(format!("user{slot}"));
        self.modified = modified(&self.save_path);
        let (wish, builds) = read_profile(self.files.as_ref());
        self.wish = wish;
        self.builds = builds;
        self.hunts = HuntTab::default();
        #[cfg(feature = "edit")]
        {
            self.guard.ledger = DebugGuard::load(self.files.as_ref()).ledger;
        }
        // the next live data is another hunter's: comparing it with the last would look like a crafting
        self.costs.tracker.reset();
        self.load_gains();
    }

    /// Live data from the game: if it is another hunter than the one shown, switch to that hunter's slot first.
    pub(super) fn follow_hunter(&mut self, save: &Save) -> Option<String> {
        if save.hunter_name == self.save.hunter_name {
            return None;
        }
        if let Some(slot) = self.slot_of_hunter(&save.hunter_name).filter(|&s| Some(s) != self.slot) {
            self.switch_profile(slot);
            return Some(format!("the game loaded {} (slot {slot}): showing their lists", save.hunter_name));
        }
        None
    }

    pub(super) fn apply_save(&mut self, save: Save) -> Option<String> {
        let (old, new) = (self.save.zenny, save.zenny);
        self.zenny_change = next_zenny_change(self.zenny_change, old, new, Instant::now());
        self.save = save;
        self.learn_unlocked();
        self.refresh_box();
        self.refresh_equipment();
        self.refresh_pieces();
        self.refresh_families();
        self.refresh_quests();
        self.builds.stale = true;
        self.hunts.stale = true;
        if self.tab == Tab::Hunts {
            self.refresh_hunts();
        }
        if self.tab == Tab::Builds {
            self.refresh_builds();
        }
        (old != new).then(|| {
            format!(
                "zenny {} (now {})",
                signed_zenny(i64::from(new) - i64::from(old)),
                group_digits(u64::from(new))
            )
        })
    }

    pub fn set_live(&mut self, live: Live) {
        self.live = Some(live);
        self.status = "live: waiting for a hunter to be loaded in the game".into();
    }

    /// True while the debug command line is open.
    pub fn is_commanding(&self) -> bool {
        #[cfg(feature = "edit")]
        return self.console.active;
        #[cfg(not(feature = "edit"))]
        false
    }

    /// Debug editing is on (`--debug-edit`, feature `edit`).
    pub fn edit_enabled(&self) -> bool {
        #[cfg(feature = "edit")]
        return self.console.enabled;
        #[cfg(not(feature = "edit"))]
        false
    }

    /// Whether debug edits are off because the game is online.
    pub fn edits_off_online(&self) -> bool {
        #[cfg(feature = "edit")]
        return self.guard.online.is_some();
        #[cfg(not(feature = "edit"))]
        false
    }

    /// What has been typed on the debug command line.
    pub fn command_text(&self) -> &str {
        #[cfg(feature = "edit")]
        return &self.console.text;
        #[cfg(not(feature = "edit"))]
        ""
    }

    /// Without the `edit` feature there is no command line, so no key goes to one.
    #[cfg(not(feature = "edit"))]
    pub(super) fn command_key(&mut self, _code: Key) {}

    #[cfg(not(feature = "edit"))]
    pub(super) fn edit_popup_open(&self) -> bool {
        false
    }

    #[cfg(not(feature = "edit"))]
    pub(super) fn edit_popup_key(&mut self, _code: Key) {}

    pub fn live_connected(&self) -> bool {
        self.live.as_ref().is_some_and(|l| l.connected)
    }

    /// Apply whatever the live reader has found since the last call.
    pub(super) fn poll_live(&mut self) {
        let Some(live) = &mut self.live else { return };
        let (mut newest, mut status, mut fresh) = (None, None, false);
        let was_connected = live.connected;
        while let Ok(event) = live.reader.events.try_recv() {
            match event {
                LiveEvent::Connected(_) => {
                    live.connected = true;
                    fresh = true;
                    status = Some("live: connected to the game".to_string());
                }
                LiveEvent::Save(bytes) => newest = Some(bytes),
                #[cfg(feature = "edit")]
                LiveEvent::WriteFailed(why) => status = Some(format!("edit failed: {why}")),
                LiveEvent::Lost => {
                    live.connected = false;
                    fresh = true;
                    status = Some("live: hunter unloaded, showing the last save file".to_string());
                }
            }
        }
        if live.child.try_wait().ok().flatten().is_some() {
            live.connected = false;
            status = Some("Cemu has exited; live updates stopped".to_string());
        }
        // The live data is gone (hunter unloaded or Cemu closed), and anything not saved with it. Go back to the save file.
        let dropped = was_connected && !live.connected;
        if dropped {
            self.costs.tracker.reset();
        }
        if fresh || dropped {
            self.forget_gain_baseline();
        }
        if let Some(bytes) = newest {
            let parsed = Save::parse(&bytes);
            #[cfg(feature = "edit")]
            {
                self.console.live_bytes = Some(bytes);
            }
            match parsed {
                Ok(save) => {
                    let switched = self.follow_hunter(&save);
                    let picked = self.record_gains(&save);
                    self.costs.tracker.observe(&save, Instant::now());
                    let craftable_before = self.craftable_wishes();
                    let zenny = self.apply_save(save);
                    let craftable = self.newly_craftable(&craftable_before);
                    status = [switched, craftable, picked, zenny].into_iter().flatten().next().or(status);
                }
                Err(e) => status = Some(format!("live data not understood: {e:#}")),
            }
        }
        if let Some(s) = status {
            self.status = s;
        }
        if dropped {
            self.modified = None; // makes `reload_if_changed` read the file now
        }
    }

    /// Reload the save file if it changed on disk (Cemu rewrites it each time the game saves).
    pub(super) fn reload_if_changed(&mut self) {
        if self.live_connected() {
            return; // the live data is newer than the file
        }
        let now = modified(&self.save_path);
        if now == self.modified {
            return;
        }
        match std::fs::read(&self.save_path)
            .map_err(anyhow::Error::from)
            .and_then(|b| Save::parse(&b))
        {
            Ok(save) => {
                let forced = self.modified.is_none(); // set when the live data went away
                self.modified = now;
                self.status = match (self.apply_save(save), forced) {
                    (Some(zenny), true) => format!("hunter unloaded, back to the save file: {zenny}"),
                    (None, true) => "hunter unloaded, back to the save file".into(),
                    (Some(zenny), false) => zenny,
                    (None, false) => "save reloaded".into(),
                };
            }
            Err(e) => self.status = format!("reload failed: {e:#}"),
        }
    }
}

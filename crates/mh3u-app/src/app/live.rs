//! Live mode and the debug command line: following the running game, reloading the save, and editing the game.

use super::*;

/// The debug command line, available with `--debug-edit`.
#[derive(Default)]
pub struct EditConsole {
    /// Debug editing is on: `:` opens the command line.
    pub enabled: bool,
    /// The command line is open.
    pub(super) active: bool,
    pub text: String,
    /// The newest live save block, the base for edit commands.
    pub(super) live_bytes: Option<Vec<u8>>,
}

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
        self.guard.ledger = DebugGuard::load(self.files.as_ref()).ledger;
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

    /// Turn on the debug command line (`:`). `note` is shown in the status line.
    pub fn enable_edit(&mut self, note: String) {
        self.console.enabled = true;
        self.status = note;
    }

    /// Run a debug command: work out the patches, then ask the live reader to write them into the game.
    pub(super) fn run_command(&mut self, text: &str) {
        let command = match commands::parse(text) {
            Ok(c) => c,
            Err(e) => return self.status = e,
        };
        if let commands::Command::Scan { kind, names } = command {
            return self.run_scan(kind, &names);
        }
        if let commands::Command::Find { bar } = command {
            return self.run_find(&bar);
        }
        if let commands::Command::Purge = command {
            return self.status = self.purge_debug_edits();
        }
        if let Some(reason) = &self.guard.online {
            return self.status = format!("debug edits are off while online ({reason}); purge takes out the ones made before");
        }
        let connected = self.live_connected();
        let (Some(live), Some(mut data)) = (&self.live, self.console.live_bytes.clone()) else {
            return self.status = "editing needs the game running through --live".into();
        };
        if !connected {
            return self.status = "editing needs a hunter loaded in the game".into();
        }
        let before = data.clone();
        let mut patches: Vec<edit::Patch> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        let mut push = |patch: edit::Patch, data: &mut Vec<u8>| {
            edit::apply(data, &patch);
            patches.push(patch);
        };
        let result: Result<(), String> = (|| {
            match command {
                commands::Command::Zenny(op) => {
                    let wanted = match op {
                        commands::ZennyOp::Set(n) => i64::from(n),
                        commands::ZennyOp::Add(d) => i64::from(self.save.zenny) + d,
                    };
                    let amount = wanted.clamp(0, i64::from(edit::MAX_ZENNY)) as u32;
                    push(edit::set_zenny(amount), &mut data);
                    notes.push(format!("zenny set to {}", group_digits(u64::from(amount))));
                }
                commands::Command::Give { item, count } => {
                    // "give tenderizer jwl 3" is the item called that, not 3 of "tenderizer jwl"
                    let (item, count) = match count.map(|n| format!("{item} {n}")) {
                        Some(whole) if commands::has_item_named(&self.game, &whole) => (whole, None),
                        _ => (item, count),
                    };
                    let found = match commands::resolve_give(&self.game, &item) {
                        Some(commands::Target::Piece { kind, id, name }) => {
                            let copies = count.unwrap_or(1).clamp(1, 20);
                            let mut slots = Vec::new();
                            for _ in 0..copies {
                                let (slot, patch) = edit::new_piece(&data, kind, id).map_err(|e| e.to_string())?;
                                push(patch, &mut data);
                                slots.push(slot.to_string());
                            }
                            notes.push(format!("{name} x{copies} in the equipment box, slot {}", slots.join(", ")));
                            return Ok(());
                        }
                        Some(commands::Target::Item(found)) => found,
                        None => return Err(format!("no item matches '{item}'")),
                    };
                    let have = edit::box_count(&data, found.id);
                    let target = count.map_or(edit::MAX_STACK, |n| have.saturating_add(n));
                    push(edit::set_box_item(&data, found.id, target).map_err(|e| e.to_string())?, &mut data);
                    notes.push(format!(
                        "{} in the box: {have} -> {}",
                        found.describe(),
                        target.min(edit::MAX_STACK)
                    ));
                }
                commands::Command::Set { item, count } => {
                    let found = commands::resolve_item(&self.game, &item).ok_or(format!("no item matches '{item}'"))?;
                    push(edit::set_box_item(&data, found.id, count).map_err(|e| e.to_string())?, &mut data);
                    notes.push(format!("{} in the box set to {}", found.describe(), count.min(edit::MAX_STACK)));
                }
                commands::Command::Scan { .. } | commands::Command::Find { .. } | commands::Command::Purge => {}
                commands::Command::Equip { slot, offset, bytes } => {
                    if bytes.is_empty() {
                        let record = edit::equipment_record(&data, slot).map_err(|e| e.to_string())?;
                        let hex: Vec<String> = record.iter().map(|b| format!("{b:02x}")).collect();
                        notes.push(format!("slot {slot}: {}", hex.join(" ")));
                    } else {
                        push(
                            edit::poke_equipment(&data, slot, offset, &bytes).map_err(|e| e.to_string())?,
                            &mut data,
                        );
                        let record = edit::equipment_record(&data, slot).map_err(|e| e.to_string())?;
                        let hex: Vec<String> = record.iter().map(|b| format!("{b:02x}")).collect();
                        notes.push(format!("slot {slot} is now {}", hex.join(" ")));
                    }
                }
                commands::Command::Talisman { skills } => {
                    let mut pairs = Vec::new();
                    for (name, points) in &skills {
                        let (id, full) = commands::resolve_skill(&self.game, name).ok_or(format!("no skill matches '{name}'"))?;
                        notes.push(format!("{full} {points:+}"));
                        pairs.push((id, *points));
                    }
                    let (slot, patch) = edit::new_talisman(&data, &pairs).map_err(|e| e.to_string())?;
                    push(patch, &mut data);
                    notes = vec![format!("talisman in slot {slot}: {}", notes.join(", "))];
                }
                commands::Command::Stock { include_owned } => {
                    let (need, _) = self.shopping_need_with(include_owned);
                    for (id, wanted) in need {
                        let have = self.save.item_count(id);
                        if have >= wanted {
                            continue;
                        }
                        let in_box = edit::box_count(&data, id);
                        let target = in_box.saturating_add(u16::try_from(wanted - have).unwrap_or(u16::MAX));
                        push(edit::set_box_item(&data, id, target).map_err(|e| e.to_string())?, &mut data);
                        notes.push(self.game.item_name(id).unwrap_or("?").to_string());
                    }
                    if notes.is_empty() {
                        notes.push("the wishlist is already covered".into());
                    } else {
                        notes = vec![format!("stocked the box: {}", notes.join(", "))];
                    }
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                for patch in patches {
                    live.reader.write(patch);
                }
                // what we just wrote must not be mistaken for something the player did
                if let Ok(edited) = Save::parse(&data) {
                    self.costs.tracker.rebase(&edited);
                }
                self.guard.ledger.record(&before, &data);
                self.save_ledger();
                self.status = notes.join("; ");
            }
            Err(e) => self.status = e,
        }
    }

    pub fn set_live(&mut self, live: Live) {
        self.live = Some(live);
        self.status = "live: waiting for a hunter to be loaded in the game".into();
    }

    /// True while the debug command line is open.
    pub fn is_commanding(&self) -> bool {
        self.console.active
    }

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
            self.console.live_bytes = Some(bytes);
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

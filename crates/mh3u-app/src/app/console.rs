//! The debug command line (feature `edit`): work out what a command changes, then ask the live reader to write it into the game.

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
    /// Turn on the debug command line (`:`). `note` is shown in the status line.
    pub fn enable_edit(&mut self, note: String) {
        self.console.enabled = true;
        self.status = note;
    }

    /// Run a debug command: work out the patches, then ask the live reader to write them into the game.
    pub(super) fn run_command(&mut self, text: &str) {
        match commands::parse(text) {
            Ok(command) => self.run_parsed(command),
            Err(e) => self.status = e,
        }
    }

    /// [`run_command`] for a command that is already parsed (the give picker builds them without text).
    pub(super) fn run_parsed(&mut self, command: commands::Command) {
        if let commands::Command::Scan { kind, names } = command {
            return self.run_scan(kind, &names);
        }
        if let commands::Command::Find { bar } = command {
            return self.run_find(&bar);
        }
        if let commands::Command::Locate { text } = &command {
            return self.run_locate(text);
        }
        if let commands::Command::Purge = command {
            return self.status = self.purge_debug_edits();
        }
        if let Some(reason) = &self.guard.online {
            return self.status = format!("debug edits are off while online ({reason}); purge takes out the ones made before");
        }
        // a name typed after `give` is settled first, so that everything below gives something already chosen
        let command = match command {
            commands::Command::Give { item, count } => {
                // "give tenderizer jwl 3" is the item called that, not 3 of "tenderizer jwl"
                let (item, count) = match count.map(|n| format!("{item} {n}")) {
                    Some(whole) if commands::has_item_named(&self.game, &whole) => (whole, None),
                    _ => (item, count),
                };
                let what = match commands::resolve_give(&self.game, &item) {
                    Some(commands::Target::Piece { kind, id, name }) => commands::GiveWhat::Piece {
                        kind,
                        id,
                        name: name.to_string(),
                    },
                    Some(commands::Target::Item(found)) => commands::GiveWhat::Item {
                        id: found.id,
                        label: found.describe(),
                    },
                    None => return self.status = format!("no item matches '{item}'"),
                };
                commands::Command::GiveTarget { what, count }
            }
            other => other,
        };
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
                commands::Command::Give { .. } => {}
                commands::Command::GiveTarget { what, count } => match what {
                    commands::GiveWhat::Piece { kind, id, name } => {
                        let copies = count.unwrap_or(1).clamp(1, 20);
                        let mut slots = Vec::new();
                        for _ in 0..copies {
                            let (slot, patch) = edit::new_piece(&data, kind, id).map_err(|e| e.to_string())?;
                            push(patch, &mut data);
                            slots.push(slot.to_string());
                        }
                        notes.push(format!("{name} x{copies} in the equipment box, slot {}", slots.join(", ")));
                    }
                    commands::GiveWhat::Item { id, label } => {
                        let have = edit::box_count(&data, id);
                        let target = count.map_or(edit::MAX_STACK, |n| have.saturating_add(n));
                        push(edit::set_box_item(&data, id, target).map_err(|e| e.to_string())?, &mut data);
                        notes.push(format!("{label} in the box: {have} -> {}", target.min(edit::MAX_STACK)));
                    }
                },
                commands::Command::Set { item, count } => {
                    let found = commands::resolve_item(&self.game, &item).ok_or(format!("no item matches '{item}'"))?;
                    push(edit::set_box_item(&data, found.id, count).map_err(|e| e.to_string())?, &mut data);
                    notes.push(format!("{} in the box set to {}", found.describe(), count.min(edit::MAX_STACK)));
                }
                commands::Command::Scan { .. }
                | commands::Command::Find { .. }
                | commands::Command::Locate { .. }
                | commands::Command::Purge => {}
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
                commands::Command::Talisman { skills, slots } => {
                    let mut pairs = Vec::new();
                    for (name, points) in &skills {
                        let (id, full) = commands::resolve_skill(&self.game, name).ok_or(format!("no skill matches '{name}'"))?;
                        notes.push(format!("{full} {points:+}"));
                        pairs.push((id, *points));
                    }
                    let (slot, patch) = edit::new_talisman(&data, &pairs, slots).map_err(|e| e.to_string())?;
                    push(patch, &mut data);
                    let gems = slots.map_or(String::new(), |n| format!(", {n} slot{}", if n == 1 { "" } else { "s" }));
                    notes = vec![format!("talisman in slot {slot}: {}{gems}", notes.join(", "))];
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

    pub(super) fn command_key(&mut self, code: Key) {
        match code {
            Key::Esc => self.console.active = false,
            Key::Enter => {
                self.console.active = false;
                let text = std::mem::take(&mut self.console.text);
                self.run_command(&text);
            }
            Key::Backspace => {
                self.console.text.pop();
            }
            Key::Char(c) => self.console.text.push(c),
            _ => {}
        }
    }
}

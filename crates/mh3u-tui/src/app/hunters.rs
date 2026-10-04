//! Picking the hunter to show without live mode (with live mode the game's hunter is followed).

use super::*;

/// The popup that lists the save slots that hold a hunter.
pub struct HunterChoice {
    /// (save slot, hunter name, play time).
    pub slots: Vec<(u8, String, String)>,
    pub state: ListState,
}

impl App {
    /// Open the hunter list. The slots are the `userN` files beside the save in use.
    pub(super) fn open_hunter_picker(&mut self) {
        if self.live_connected() {
            self.status = "the game decides the hunter while it is live".to_string();
            return;
        }
        let Some(dir) = self.save_path.parent() else { return };
        let slots: Vec<(u8, String, String)> = (1..=3u8)
            .filter_map(|slot| {
                let bytes = std::fs::read(dir.join(format!("user{slot}"))).ok()?;
                let save = Save::parse(&bytes).ok()?;
                let played = save.play_time();
                Some((slot, save.hunter_name, played))
            })
            .filter(|(_, name, _)| !name.is_empty())
            .collect();
        if slots.is_empty() {
            self.status = "no save slot holds a hunter".to_string();
            return;
        }
        let at = slots.iter().position(|(slot, ..)| Some(*slot) == self.slot).unwrap_or(0);
        self.hunter_choice = Some(HunterChoice {
            slots,
            state: ListState::default().with_selected(Some(at)),
        });
    }

    /// The save slot of the hunter on screen, when known.
    pub fn slot_shown(&self) -> Option<u8> {
        self.slot
    }

    /// Show another hunter: their lists and their save file.
    fn pick_hunter(&mut self, slot: u8) {
        if Some(slot) == self.slot {
            return;
        }
        self.switch_profile(slot);
        match std::fs::read(&self.save_path)
            .map_err(anyhow::Error::from)
            .and_then(|b| Save::parse(&b))
        {
            Ok(save) => {
                let name = save.hunter_name.clone();
                self.apply_save(save);
                self.status = format!("showing {name} (slot {slot})");
            }
            Err(e) => self.status = format!("could not read slot {slot}: {e:#}"),
        }
    }

    /// Keys while the hunter list is open: move, Enter shows the hunter, Esc closes.
    pub(super) fn hunter_choice_key(&mut self, code: KeyCode) {
        let Some(choice) = self.hunter_choice.as_mut() else { return };
        let last = choice.slots.len().saturating_sub(1);
        let at = choice.state.selected().unwrap_or(0);
        match code {
            KeyCode::Down | KeyCode::Char('j') => choice.state.select(Some((at + 1).min(last))),
            KeyCode::Up | KeyCode::Char('k') => choice.state.select(Some(at.saturating_sub(1))),
            KeyCode::Enter => {
                let slot = choice.slots.get(at).map(|(slot, ..)| *slot);
                self.hunter_choice = None;
                if let Some(slot) = slot {
                    self.pick_hunter(slot);
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => self.hunter_choice = None,
            _ => {}
        }
    }
}

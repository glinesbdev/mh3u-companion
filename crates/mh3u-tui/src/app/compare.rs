//! The Compare tab: a few weapons side by side.

use super::*;

/// The most weapons the comparison holds (more would not fit side by side).
pub const MAX_COMPARED: usize = 4;

/// The weapons being compared, as (equipment kind, id), and the highlighted one. Kept until the app closes.
pub struct CompareTab {
    pub weapons: Vec<(u8, u16)>,
    pub state: ListState,
}

impl Default for CompareTab {
    fn default() -> CompareTab {
        CompareTab {
            weapons: Vec::new(),
            state: ListState::default().with_selected(Some(0)),
        }
    }
}

impl App {
    /// Put the weapon highlighted on the current tab into the comparison, or take it out if it is there.
    pub(super) fn toggle_compare(&mut self) {
        let Some((kind, id)) = self
            .highlighted_equipment()
            .filter(|&(kind, id)| self.game.weapon_stats(kind, id).is_some())
        else {
            self.status = "only weapons can be compared".to_string();
            return;
        };
        let name = self.game.equipment_name(kind, id).unwrap_or("?").to_string();
        if let Some(at) = self.compare.weapons.iter().position(|&w| w == (kind, id)) {
            self.compare.weapons.remove(at);
            self.status = format!("{name} taken out of the comparison");
        } else if self.compare.weapons.len() >= MAX_COMPARED {
            self.status = format!("the comparison holds {MAX_COMPARED} weapons: take one out first (x on the Compare tab)");
        } else {
            self.compare.weapons.push((kind, id));
            self.status = format!(
                "{name} added to the comparison ({} of {MAX_COMPARED}; see the Compare tab)",
                self.compare.weapons.len()
            );
        }
        let len = self.compare.weapons.len();
        let at = self.compare.state.selected().unwrap_or(0).min(len.saturating_sub(1));
        self.compare.state.select((len > 0).then_some(at));
    }

    pub(super) fn compare_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some(at) = self.compare.state.selected().filter(|&i| i < self.compare.weapons.len()) {
                    self.compare.weapons.remove(at);
                    let len = self.compare.weapons.len();
                    self.compare.state.select((len > 0).then_some(at.min(len - 1)));
                }
            }
            KeyCode::Char('c') => {
                self.compare.weapons.clear();
                self.compare.state.select(None);
            }
            // look the highlighted weapon up on the Crafting tab
            KeyCode::Enter => {
                let Some(&(kind, id)) = self.compare.state.selected().and_then(|i| self.compare.weapons.get(i)) else {
                    return true;
                };
                self.craft.search = self.game.equipment_name(kind, id).unwrap_or_default().to_string();
                self.tab = Tab::Crafting;
                self.refresh_pieces();
            }
            _ => return false,
        }
        true
    }
}

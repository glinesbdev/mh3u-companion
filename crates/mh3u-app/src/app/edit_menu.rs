//! The edit menu (feature `edit`, key `E` with debug editing on): the visual editors to choose from, and the keys of whichever popup is
//! open.

use super::*;

/// The editors, in the order the menu lists them.
pub const EDITORS: [&str; 2] = ["Give an item or gear", "Make a talisman"];

impl App {
    /// Whether the edit menu or one of the editors is open.
    pub fn edit_popup_open(&self) -> bool {
        self.edit_menu.is_some() || self.give.is_some() || self.talisman.is_some()
    }

    pub(super) fn open_edit_menu(&mut self) {
        self.edit_menu = Some(ListState::default().with_selected(Some(0)));
    }

    /// A key for the editor that is open (an editor takes the keys before the menu under it).
    pub(super) fn edit_popup_key(&mut self, code: Key) {
        if self.give.is_some() {
            return self.give_key(code);
        }
        if self.talisman.is_some() {
            return self.talisman_key(code);
        }
        let Some(menu) = &mut self.edit_menu else { return };
        let at = menu.selected().unwrap_or(0);
        match code {
            Key::Esc | Key::Char('q') => self.edit_menu = None,
            Key::Down | Key::Char('j') => menu.select(Some((at + 1).min(EDITORS.len() - 1))),
            Key::Up | Key::Char('k') => menu.select(Some(at.saturating_sub(1))),
            Key::Enter => {
                self.edit_menu = None;
                match at {
                    0 => self.open_give_picker(),
                    _ => self.open_talisman_form(),
                }
            }
            _ => {}
        }
    }
}

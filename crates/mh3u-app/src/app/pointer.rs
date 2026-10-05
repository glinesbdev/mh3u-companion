//! The mouse. A click or a wheel turn becomes the keys that do the same thing, so that every tab and popup moves the way it does from
//! the keyboard. Where things are comes from `hits`, filled in by the screen as it draws.

use super::*;
use crate::hits::{Focus, ListHit};
use crate::input::Pointer;

/// Rows the wheel moves per notch.
const WHEEL_ROWS: usize = 3;
/// A second click on the same row within this long is a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(500);

impl App {
    /// The mouse was used where the last frame showed `hits`.
    pub fn on_pointer(&mut self, pointer: Pointer) {
        match pointer {
            Pointer::Click { col, row } => self.click(col, row),
            Pointer::Scroll { down, .. } => {
                if self.confirm_quit {
                    return;
                }
                let key = if down { Key::Down } else { Key::Up };
                for _ in 0..WHEEL_ROWS {
                    self.on_key(key, Mods::default());
                }
            }
        }
    }

    /// Whether a popup is open over the tab: the tab's own tabs and lists can't be clicked then.
    fn popup_open(&self) -> bool {
        self.confirm_quit
            || self.tree.is_some()
            || self.settings.is_some()
            || self.hunter_choice.is_some()
            || self.quests.choosing.is_some()
            || self.builds.skill_picker.is_some()
            || self.builds.name_prompt.is_some()
            || self.builds.piece_picker.is_some()
            || self.show_help
    }

    fn click(&mut self, col: u16, row: u16) {
        let popup = self.popup_open();
        let typing = self.console.active || self.searching;
        let first_list = if popup { self.hits.tab_lists } else { 0 };
        let found = self
            .hits
            .lists
            .iter()
            .enumerate()
            .skip(first_list)
            .rev()
            .find(|(_, l)| l.area.contains(col, row))
            .map(|(_, l)| *l);
        if let Some(list) = found {
            if let Some(at) = list.row_at(col, row).filter(|_| !typing) {
                self.click_row(list, at, (col, row));
            }
            return;
        }
        if popup || typing {
            return;
        }
        if let Some(&(_, tab)) = self.hits.tabs.iter().find(|(a, _)| a.contains(col, row)) {
            self.go_to_tab(tab);
        }
    }

    fn go_to_tab(&mut self, tab: Tab) {
        let at = |t: Tab| Tab::ALL.iter().position(|&x| x == t).unwrap_or(0) as isize;
        self.switch_tab(at(tab) - at(self.tab));
    }

    fn click_row(&mut self, list: ListHit, at: usize, cell: (u16, u16)) {
        let now = Instant::now();
        let again = self
            .last_click
            .is_some_and(|(when, c, i)| c == cell && i == at && now.duration_since(when) < DOUBLE_CLICK);
        self.last_click = if again { None } else { Some((now, cell, at)) };
        let step = |app: &mut App, key: Key, times: usize| {
            for _ in 0..times {
                app.on_key(key, Mods::default());
            }
        };
        match (list.selected, list.focus) {
            (Some(selected), _) => {
                if at == selected {
                    if again {
                        self.on_key(Key::Enter, Mods::default());
                    }
                } else if at > selected {
                    step(self, Key::Down, at - selected);
                } else {
                    step(self, Key::Up, selected - at);
                }
            }
            // a list that did not have the keys: give them over, then go to the row from the top
            (None, Some(focus)) => {
                match focus {
                    Focus::ItemsPouch => self.inv.pouch_focus = true,
                    Focus::ItemsBox => self.inv.pouch_focus = false,
                    Focus::Builds(f) => self.builds.focus = f,
                }
                step(self, Key::Home, 1);
                step(self, Key::Down, at);
            }
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_double_click_window_is_short_enough_to_not_catch_two_separate_clicks() {
        assert!(DOUBLE_CLICK < Duration::from_secs(1));
    }
}

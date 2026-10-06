//! The give picker (feature `edit`, key `E` with debug editing on): find an item, armor piece or weapon by typing part of its name,
//! choose how many, and give it, the same as the `give` command does.

use super::*;
use crate::commands::{Command, GiveWhat};

/// The most rows listed at once; typing narrows them.
const SHOWN: usize = 300;
/// The most of one thing: a stack holds 99 items; gear is made one at a time, up to 20 copies.
const MOST_ITEMS: u16 = 99;
const MOST_GEAR: u16 = 20;

/// One thing that can be given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiveChoice {
    pub what: GiveWhat,
    pub name: String,
    /// What kind of thing it is, to tell same-named things apart: `item`, `Great Sword`, `Head armor`...
    pub kind: String,
}

impl GiveChoice {
    fn most(&self) -> u16 {
        match self.what {
            GiveWhat::Item { .. } => MOST_ITEMS,
            GiveWhat::Piece { .. } => MOST_GEAR,
        }
    }
}

/// The popup: a name being typed, the things that match, and how many to give.
pub struct GivePicker {
    pub text: String,
    pub choices: Vec<GiveChoice>,
    pub state: ListState,
    pub amount: u16,
}

impl GivePicker {
    /// The highlighted thing, if any.
    pub fn chosen(&self) -> Option<&GiveChoice> {
        self.state.selected().and_then(|i| self.choices.get(i))
    }

    /// How many of the highlighted thing will be given (the amount, held to what that thing allows).
    pub fn count(&self) -> u16 {
        self.amount.min(self.chosen().map_or(MOST_ITEMS, GiveChoice::most)).max(1)
    }
}

impl App {
    /// Everything the typed words match: every word must be in the name (loosely, as in the other searches), best first.
    fn give_matches(&self, text: &str) -> Vec<GiveChoice> {
        let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
        let mut found: Vec<(u32, GiveChoice)> = Vec::new();
        let mut consider = |name: &str, what: GiveWhat, kind: &str| {
            let lower = name.to_lowercase();
            let mut total = 0;
            for word in &words {
                match search::score(word, &lower) {
                    Some(s) => total += s,
                    None => return,
                }
            }
            found.push((
                total,
                GiveChoice {
                    what,
                    name: name.to_string(),
                    kind: kind.to_string(),
                },
            ));
        };
        for (id, name) in self.game.item_names() {
            if name.is_empty() || name.starts_with("DUMMY") || name.starts_with('(') {
                continue;
            }
            consider(
                name,
                GiveWhat::Item {
                    id,
                    label: name.to_string(),
                },
                "item",
            );
        }
        for (kind, id, name) in self.game.equipment_pieces() {
            let label = self.game.equipment_kind_label(kind).unwrap_or("gear");
            consider(
                name,
                GiveWhat::Piece {
                    kind,
                    id,
                    name: name.to_string(),
                },
                label,
            );
        }
        // best score first; the sort is stable, so equal scores stay in the game's order
        found.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        found.into_iter().take(SHOWN).map(|(_, c)| c).collect()
    }

    /// Open the picker (debug editing on).
    pub(super) fn open_give_picker(&mut self) {
        let choices = self.give_matches("");
        self.give = Some(GivePicker {
            text: String::new(),
            choices,
            state: ListState::default().with_selected(Some(0)),
            amount: 1,
        });
    }

    fn refilter_give(&mut self) {
        let Some(text) = self.give.as_ref().map(|g| g.text.clone()) else {
            return;
        };
        let choices = self.give_matches(&text);
        if let Some(picker) = &mut self.give {
            picker.state.select((!choices.is_empty()).then_some(0));
            picker.choices = choices;
        }
    }

    pub(super) fn give_key(&mut self, code: Key) {
        let Some(picker) = &mut self.give else { return };
        let last = picker.choices.len().saturating_sub(1);
        let at = picker.state.selected().unwrap_or(0);
        match code {
            Key::Esc => self.give = None,
            Key::Down => picker.state.select(Some((at + 1).min(last))),
            Key::Up => picker.state.select(Some(at.saturating_sub(1))),
            Key::PageDown => picker.state.select(Some((at + 10).min(last))),
            Key::PageUp => picker.state.select(Some(at.saturating_sub(10))),
            Key::Right => picker.amount = (picker.count() + 1).min(MOST_ITEMS),
            Key::Left => picker.amount = picker.count().saturating_sub(1).max(1),
            // 1, 10, 50, 99: the amounts that come up
            Key::Tab => {
                picker.amount = match picker.count() {
                    1..=9 => 10,
                    10..=49 => 50,
                    50..=98 => MOST_ITEMS,
                    _ => 1,
                }
            }
            Key::Backspace => {
                picker.text.pop();
                self.refilter_give();
            }
            Key::Char(c) => {
                picker.text.push(c);
                self.refilter_give();
            }
            Key::Enter => {
                let Some(choice) = picker.chosen().cloned() else { return };
                let count = picker.count();
                // the picker stays open, so several things can be given in a row; the status line says what happened
                self.run_parsed(Command::GiveTarget {
                    what: choice.what,
                    count: Some(count),
                });
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_amount_steps_through_the_usual_numbers_and_is_held_to_what_the_thing_allows() {
        let gear = GiveChoice {
            what: GiveWhat::Piece {
                kind: 1,
                id: 1,
                name: "X".into(),
            },
            name: "X".into(),
            kind: "Head".into(),
        };
        let mut picker = GivePicker {
            text: String::new(),
            choices: vec![gear],
            state: ListState::default().with_selected(Some(0)),
            amount: 50,
        };
        assert_eq!(picker.count(), 20, "gear is made one at a time, up to 20");
        picker.amount = 0;
        assert_eq!(picker.count(), 1);
    }
}

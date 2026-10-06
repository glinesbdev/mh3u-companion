//! The talisman form (feature `edit`): choose up to two skills with their points and the number of gem slots, then make the
//! talisman, the same as the `talisman` command does.

use super::*;
use crate::commands::Command;
use mh3u_core::edit::{MAX_TALISMAN_SLOTS, TALISMAN_PAIRS};

/// The rows of the form: the skills, the slots, and the button that makes it.
pub const ROW_SLOTS: usize = TALISMAN_PAIRS;
pub const ROW_MAKE: usize = TALISMAN_PAIRS + 1;
const ROWS: usize = TALISMAN_PAIRS + 2;
/// The points a skill can be given in the form.
const MOST_POINTS: i8 = 15;
const FIRST_POINTS: i8 = 10;
/// The most skills listed while finding one.
const SHOWN: usize = 200;

/// Finding a skill for a row by typing part of its name.
pub struct SkillFind {
    pub text: String,
    pub matches: Vec<(u8, String)>,
    pub state: ListState,
}

pub struct TalismanForm {
    pub skills: [Option<(u8, i8)>; TALISMAN_PAIRS],
    pub slots: u8,
    pub rows: ListState,
    pub find: Option<SkillFind>,
}

impl TalismanForm {
    fn new() -> TalismanForm {
        TalismanForm {
            skills: [None; TALISMAN_PAIRS],
            slots: MAX_TALISMAN_SLOTS,
            rows: ListState::default().with_selected(Some(0)),
            find: None,
        }
    }

    fn row(&self) -> usize {
        self.rows.selected().unwrap_or(0).min(ROWS - 1)
    }
}

impl App {
    pub(super) fn open_talisman_form(&mut self) {
        self.talisman = Some(TalismanForm::new());
    }

    /// The skills whose names hold every typed word, best match first.
    fn talisman_skills(&self, text: &str) -> Vec<(u8, String)> {
        let words: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
        let mut found: Vec<(u32, u8, String)> = Vec::new();
        'skills: for id in self.game.skill_ids() {
            let Some(name) = self.game.skill_name(id).filter(|n| !n.is_empty()) else {
                continue;
            };
            let lower = name.to_lowercase();
            let mut total = 0;
            for word in &words {
                match search::score(word, &lower) {
                    Some(s) => total += s,
                    None => continue 'skills,
                }
            }
            found.push((total, id, name.to_string()));
        }
        found.sort_by_key(|(score, ..)| std::cmp::Reverse(*score));
        found.into_iter().take(SHOWN).map(|(_, id, name)| (id, name)).collect()
    }

    fn start_skill_find(&mut self, text: String) {
        let matches = self.talisman_skills(&text);
        let state = ListState::default().with_selected((!matches.is_empty()).then_some(0));
        if let Some(form) = &mut self.talisman {
            form.find = Some(SkillFind { text, matches, state });
        }
    }

    pub(super) fn talisman_key(&mut self, code: Key) {
        let Some(form) = &mut self.talisman else { return };
        if form.find.is_some() {
            return self.skill_find_key(code);
        }
        let row = form.row();
        match code {
            Key::Esc => self.talisman = None,
            Key::Down => form.rows.select(Some((row + 1).min(ROWS - 1))),
            Key::Up => form.rows.select(Some(row.saturating_sub(1))),
            Key::Left | Key::Right => {
                let up = code == Key::Right;
                if row == ROW_SLOTS {
                    form.slots = if up {
                        (form.slots + 1).min(MAX_TALISMAN_SLOTS)
                    } else {
                        form.slots.saturating_sub(1)
                    };
                } else if let Some(Some((_, points))) = form.skills.get_mut(row) {
                    *points = if up {
                        (*points + 1).min(MOST_POINTS)
                    } else {
                        (*points - 1).max(-MOST_POINTS)
                    };
                }
            }
            Key::Backspace | Key::Delete if row < TALISMAN_PAIRS => form.skills[row] = None,
            Key::Enter if row < TALISMAN_PAIRS => self.start_skill_find(String::new()),
            Key::Enter if row == ROW_MAKE => self.make_talisman(),
            Key::Char(c) if row < TALISMAN_PAIRS => self.start_skill_find(c.to_string()),
            _ => {}
        }
    }

    fn skill_find_key(&mut self, code: Key) {
        let Some(form) = &mut self.talisman else { return };
        let row = form.row();
        let Some(find) = &mut form.find else { return };
        let last = find.matches.len().saturating_sub(1);
        let at = find.state.selected().unwrap_or(0);
        match code {
            Key::Esc => form.find = None,
            Key::Down => find.state.select(Some((at + 1).min(last))),
            Key::Up => find.state.select(Some(at.saturating_sub(1))),
            Key::PageDown => find.state.select(Some((at + 10).min(last))),
            Key::PageUp => find.state.select(Some(at.saturating_sub(10))),
            Key::Enter => {
                if let Some((id, _)) = find.state.selected().and_then(|i| find.matches.get(i)) {
                    let points = form.skills[row].map_or(FIRST_POINTS, |(_, p)| p);
                    form.skills[row] = Some((*id, points));
                    form.find = None;
                }
            }
            Key::Backspace | Key::Char(_) => {
                let mut text = find.text.clone();
                match code {
                    Key::Char(c) => text.push(c),
                    _ => {
                        text.pop();
                    }
                }
                self.start_skill_find(text);
            }
            _ => {}
        }
    }

    fn make_talisman(&mut self) {
        let Some(form) = &self.talisman else { return };
        let chosen: Vec<(u8, i8)> = form.skills.iter().flatten().copied().collect();
        let slots = form.slots;
        if chosen.is_empty() {
            return self.status = "choose a skill first (Enter on a skill row)".to_string();
        }
        if chosen.len() == 2 && chosen[0].0 == chosen[1].0 {
            return self.status = "the two skills are the same one".to_string();
        }
        let skills: Vec<(String, i8)> = chosen
            .iter()
            .map(|&(id, points)| (self.game.skill_name(id).unwrap_or("?").to_string(), points))
            .collect();
        self.run_parsed(Command::Talisman {
            skills,
            slots: Some(slots),
        });
    }
}

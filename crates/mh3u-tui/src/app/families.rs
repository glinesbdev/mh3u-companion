//! The Sets tab: armor grouped by family, and which variants of each you have.

use super::*;
use crate::families::{self, Family};

/// One row of the family list.
pub struct FamilyRow {
    /// Index into `FamiliesTab::families`.
    pub family: usize,
    pub owned: usize,
    pub total: usize,
}

/// The Sets tab: every family, the rows shown (after the search and filter) and the highlighted one.
pub struct FamiliesTab {
    pub families: Vec<Family>,
    pub rows: Vec<FamilyRow>,
    pub state: ListState,
    pub search: String,
    /// Only families you own at least one piece of.
    pub only_owned: bool,
}

impl FamiliesTab {
    pub(super) fn new(families: Vec<Family>) -> FamiliesTab {
        FamiliesTab {
            families,
            rows: Vec::new(),
            state: ListState::default().with_selected(Some(0)),
            search: String::new(),
            only_owned: false,
        }
    }

    pub fn selected(&self) -> Option<&Family> {
        self.state
            .selected()
            .and_then(|i| self.rows.get(i))
            .map(|r| &self.families[r.family])
    }
}

impl App {
    /// Group the game's armor into families (once, when the app starts).
    pub(super) fn group_families(game: &GameData) -> Vec<Family> {
        let names = [5u8, 1, 2, 3, 4].into_iter().flat_map(|kind| {
            game.piece_ids(kind)
                .filter_map(move |id| Some((kind, id, game.piece_name(kind, id)?)))
        });
        families::group(names)
    }

    /// Work out the rows again: which families match the search and filter, and how much of each you own.
    pub(super) fn refresh_families(&mut self) {
        let words: Vec<String> = self.families.search.split_whitespace().map(str::to_lowercase).collect();
        let mut rows: Vec<(u32, FamilyRow)> = Vec::new();
        for (family, f) in self.families.families.iter().enumerate() {
            let lower = f.name.to_lowercase();
            let mut score = 0;
            let mut matched = true;
            for w in &words {
                match search::score(w, &lower) {
                    Some(s) => score += s,
                    None => {
                        matched = false;
                        break;
                    }
                }
            }
            if !matched {
                continue;
            }
            let owned = f.members.iter().filter(|m| self.save.owns_equipment(m.kind, m.id)).count();
            if self.families.only_owned && owned == 0 {
                continue;
            }
            rows.push((
                score,
                FamilyRow {
                    family,
                    owned,
                    total: f.members.len(),
                },
            ));
        }
        if !words.is_empty() {
            rows.sort_by_key(|(score, r)| (std::cmp::Reverse(*score), r.family));
        }
        self.families.rows = rows.into_iter().map(|(_, r)| r).collect();
        let len = self.families.rows.len();
        let at = self.families.state.selected().unwrap_or(0).min(len.saturating_sub(1));
        self.families.state.select((len > 0).then_some(at));
    }

    pub(super) fn families_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('o') => {
                self.families.only_owned = !self.families.only_owned;
                self.refresh_families();
            }
            KeyCode::Char('x') => self.clear_search(),
            // look the family up on the Crafting tab
            KeyCode::Enter => {
                let Some(name) = self.families.selected().map(|f| f.name.clone()) else {
                    return true;
                };
                self.craft.search = name;
                self.tab = Tab::Crafting;
                self.refresh_pieces();
            }
            _ => return false,
        }
        true
    }
}

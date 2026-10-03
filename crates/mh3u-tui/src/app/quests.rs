//! The Quests tab: every quest in the game, searchable by name, monster or reward.

use super::*;
use mh3u_core::quest::Quest;

/// How the quest list is ordered.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum QuestSort {
    /// By quest id, which is the game's order.
    #[default]
    Id,
    Name,
    Stars,
    /// The quests that give the most of what the wishlist is short of first.
    Needed,
}

impl QuestSort {
    fn next(self) -> QuestSort {
        match self {
            QuestSort::Id => QuestSort::Name,
            QuestSort::Name => QuestSort::Stars,
            QuestSort::Stars => QuestSort::Needed,
            QuestSort::Needed => QuestSort::Id,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            QuestSort::Id => "game order",
            QuestSort::Name => "by name",
            QuestSort::Stars => "by stars",
            QuestSort::Needed => "wishlist first",
        }
    }
}

/// One row of the list.
pub struct QuestRow {
    /// Index into `GameData::quests`.
    pub quest: usize,
    /// How many of the items the wishlist is still short of this quest can give.
    pub needed: usize,
}

/// What a quest is searched by, lowercased once.
struct QuestText {
    title: String,
    goal: String,
    client: String,
    monsters: Vec<String>,
    items: Vec<String>,
}

pub struct QuestTab {
    pub rows: Vec<QuestRow>,
    pub state: ListState,
    pub search: String,
    pub sort: QuestSort,
    /// First visible line of the details; the drawing code keeps it inside the text.
    pub scroll: u16,
    text: Vec<QuestText>,
}

impl QuestTab {
    pub(super) fn new(game: &GameData) -> QuestTab {
        let lower = |s: &str| s.to_lowercase();
        let text = game
            .quests()
            .iter()
            .map(|q| QuestText {
                title: lower(&q.title),
                goal: lower(&q.goal),
                client: lower(&q.client),
                monsters: q.monsters.iter().filter_map(|&m| game.monster_name(m)).map(lower).collect(),
                items: q
                    .rewards
                    .iter()
                    .flatten()
                    .filter_map(|r| game.item_name(r.item))
                    .map(lower)
                    .collect(),
            })
            .collect();
        QuestTab {
            rows: Vec::new(),
            state: ListState::default().with_selected(Some(0)),
            search: String::new(),
            sort: QuestSort::default(),
            scroll: 0,
            text,
        }
    }

    pub fn selected<'a>(&self, quests: &'a [Quest]) -> Option<&'a Quest> {
        self.state.selected().and_then(|i| self.rows.get(i)).map(|r| &quests[r.quest])
    }
}

impl QuestText {
    /// The score for the query words, or `None` if some word matches nothing about the quest.
    fn matches(&self, words: &[String]) -> Option<u32> {
        let mut total = 0;
        for w in words {
            let fields = [
                search::score(w, &self.title).map(|s| 3 * s),
                search::score(w, &self.goal).map(|s| 3 * s),
                search::score(w, &self.client),
                self.monsters.iter().filter_map(|m| search::score(w, m)).max().map(|s| 2 * s),
                self.items.iter().filter_map(|i| search::score(w, i)).max().map(|s| 2 * s),
            ];
            total += fields.into_iter().flatten().max()?;
        }
        Some(total)
    }
}

impl App {
    /// Work out the rows again: which quests match the search, in the order chosen.
    pub(super) fn refresh_quests(&mut self) {
        let words: Vec<String> = self.quests.search.split_whitespace().map(str::to_lowercase).collect();
        let missing = self.missing_for_wishlist();
        let quests = self.game.quests();
        let mut rows: Vec<(u32, QuestRow)> = Vec::new();
        for (quest, q) in quests.iter().enumerate() {
            let Some(score) = self.quests.text[quest].matches(&words) else {
                continue;
            };
            let needed = q
                .rewards
                .iter()
                .flatten()
                .map(|r| r.item)
                .filter(|item| missing.contains_key(item))
                .collect::<HashSet<u16>>()
                .len();
            rows.push((score, QuestRow { quest, needed }));
        }
        if !words.is_empty() {
            rows.sort_by_key(|(score, r)| (std::cmp::Reverse(*score), r.quest));
        }
        match self.quests.sort {
            QuestSort::Id => {}
            QuestSort::Name => rows.sort_by_key(|(_, r)| quests[r.quest].title.to_lowercase()),
            QuestSort::Stars => rows.sort_by_key(|(_, r)| (quests[r.quest].stars, r.quest)),
            QuestSort::Needed => rows.sort_by_key(|(_, r)| (std::cmp::Reverse(r.needed), r.quest)),
        }
        self.quests.rows = rows.into_iter().map(|(_, r)| r).collect();
        let len = self.quests.rows.len();
        let at = self.quests.state.selected().unwrap_or(0).min(len.saturating_sub(1));
        self.quests.state.select((len > 0).then_some(at));
    }

    pub(super) fn quests_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('s') => {
                self.quests.sort = self.quests.sort.next();
                self.refresh_quests();
            }
            KeyCode::Char('x') => self.clear_search(),
            KeyCode::PageDown => self.quests.scroll = self.quests.scroll.saturating_add(10),
            KeyCode::PageUp => self.quests.scroll = self.quests.scroll.saturating_sub(10),
            _ => return false,
        }
        true
    }
}

//! The Monsters tab.

use super::*;

/// The Monsters tab: how the list is ordered, the highlighted monster and how far its drops are scrolled.
#[derive(Default)]
pub struct MonsterTab {
    pub sort: MonsterSort,
    /// The highlighted monster (by id, so the highlight stays on it when the order changes).
    pub selected: Option<u16>,
    /// First visible line of the highlighted monster's drops; the drawing code keeps it inside the text.
    pub scroll: u16,
}

impl App {
    /// The monsters that have drops, in display order, each with how many of the wishlist's missing items it drops.
    pub fn monster_view(&self) -> Vec<(u16, usize)> {
        let missing = self.missing_for_wishlist();
        let drops = self.game.drops();
        let mut view: Vec<(u16, usize)> = drops
            .monsters()
            .iter()
            .copied()
            .filter(|&m| self.game.monster_name(m).is_some())
            .map(|m| {
                let wanted: HashSet<u16> = drops
                    .lists_for(m)
                    .flat_map(|(_, _, list)| list.iter().map(|d| d.item))
                    .filter(|item| missing.contains_key(item))
                    .collect();
                (m, wanted.len())
            })
            .collect();
        match self.monsters.sort {
            MonsterSort::GameOrder => {}
            MonsterSort::Name => view.sort_by_key(|&(m, _)| self.game.monster_name(m).unwrap_or("").to_lowercase()),
            MonsterSort::Needed => view.sort_by_key(|&(m, wanted)| (std::cmp::Reverse(wanted), m)),
        }
        view
    }

    /// The highlighted monster: the remembered one if it is still listed, else the first.
    pub fn highlighted_monster(&self) -> Option<u16> {
        let view = self.monster_view();
        self.monsters
            .selected
            .filter(|m| view.iter().any(|&(v, _)| v == *m))
            .or_else(|| view.first().map(|&(m, _)| m))
    }
}

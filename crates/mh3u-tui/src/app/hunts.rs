//! The Hunt plan tab: which monsters to hunt for the materials the wishlist is short of.

use super::*;
use crate::hunts::{self, Origin, Plan, QuestOffer, RankFilter};

/// The Hunt plan tab: the plan, the ranks it may use, and the highlighted hunt.
pub struct HuntTab {
    pub filter: RankFilter,
    pub plan: Plan,
    pub state: ListState,
    /// The wishlist or the save changed since the plan was made; it is made again when the tab is next shown.
    pub(super) stale: bool,
}

impl Default for HuntTab {
    fn default() -> HuntTab {
        HuntTab {
            filter: RankFilter::All,
            plan: Plan::default(),
            state: ListState::default().with_selected(Some(0)),
            stale: true,
        }
    }
}

impl App {
    /// Plan the hunts again for what the wishlist is still missing.
    pub(super) fn refresh_hunts(&mut self) {
        self.hunts.stale = false;
        let mut missing: Vec<(u16, u32)> = self.missing_for_wishlist().into_iter().collect();
        missing.sort_unstable();
        let drops = self.game.drops();
        let quests = self.quest_offers();
        self.hunts.plan = hunts::plan(
            &missing,
            |item| drops.sources(item),
            &quests,
            self.hunts.filter,
            |m| self.game.monster_name(m).is_some(),
        );
        let len = self.hunts.plan.steps.len();
        let at = self.hunts.state.selected().unwrap_or(0).min(len.saturating_sub(1));
        self.hunts.state.select((len > 0).then_some(at));
    }

    /// What each quest gives, for the plan. Tutorials (no star rank) are left out.
    fn quest_offers(&self) -> Vec<QuestOffer> {
        self.game
            .quests()
            .iter()
            .filter(|q| q.stars > 0)
            .map(|q| QuestOffer {
                id: q.id,
                rewards: q
                    .rewards
                    .iter()
                    .enumerate()
                    .flat_map(|(second, box_)| box_.iter().map(move |r| (r.item, r.percent, r.quantity, second == 1)))
                    .collect(),
            })
            .collect()
    }

    pub(super) fn hunts_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('r') => {
                self.hunts.filter = self.hunts.filter.next();
                self.refresh_hunts();
            }
            // show the step's monster on the Monsters tab, or its quest on the Quests tab
            KeyCode::Enter => {
                let Some(origin) = self
                    .hunts
                    .state
                    .selected()
                    .and_then(|i| self.hunts.plan.steps.get(i))
                    .map(|s| s.origin)
                else {
                    return true;
                };
                match origin {
                    Origin::Monster { monster, .. } => self.show_monster(monster),
                    Origin::Quest(id) => self.show_quest(id),
                }
            }
            _ => return false,
        }
        true
    }
}

//! The Hunt plan tab: which monsters to hunt for the materials the wishlist is short of.

use super::*;
use crate::hunts::{self, Plan, RankFilter};

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
        self.hunts.plan = hunts::plan(
            &missing,
            |item| drops.sources(item),
            self.hunts.filter,
            |m| self.game.monster_name(m).is_some(),
        );
        let len = self.hunts.plan.steps.len();
        let at = self.hunts.state.selected().unwrap_or(0).min(len.saturating_sub(1));
        self.hunts.state.select((len > 0).then_some(at));
    }

    pub(super) fn hunts_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('r') => {
                self.hunts.filter = self.hunts.filter.next();
                self.refresh_hunts();
            }
            // show the hunt's monster on the Monsters tab
            KeyCode::Enter => {
                let Some(step) = self.hunts.state.selected().and_then(|i| self.hunts.plan.steps.get(i)) else {
                    return true;
                };
                self.monsters.selected = Some(step.monster);
                self.monsters.scroll = 0;
                self.tab = Tab::Monsters;
            }
            _ => return false,
        }
        true
    }
}

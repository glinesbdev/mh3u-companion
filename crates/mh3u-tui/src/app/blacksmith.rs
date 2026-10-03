//! What the blacksmith offers: the rule from `mh3u_core::blacksmith` plus the pieces seen on offer before.

use super::*;

/// Whether the blacksmith offers a piece: the rule in `mh3u_core::blacksmith` (a monster that drops its first material has been
/// hunted), once pieces seen on offer before are counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Offer {
    /// What the rule says.
    Rule(mh3u_core::blacksmith::Unlock),
    /// The rule says not yet, but the piece was seen on offer before (the blacksmith never takes a piece off the list).
    Earlier,
}

impl App {
    /// Whether the blacksmith offers a piece you can create: by the rule in `mh3u_core::blacksmith` plus the pieces seen on offer
    /// before. `None` when the game data has no create recipe for it (a weapon reached only by upgrading).
    pub fn offer(&self, kind: u8, id: u16) -> Option<Offer> {
        use mh3u_core::blacksmith::Unlock;
        let rule = self.rule(kind, id)?;
        let remembered = self.unlocked.contains(&self.save.hunter_name, (kind, id));
        Some(match rule {
            Unlock::NeedsHunt(..) | Unlock::NeedsRank(_) | Unlock::Unknown(_) if remembered => Offer::Earlier,
            rule => Offer::Rule(rule),
        })
    }

    pub(super) fn rule(&self, kind: u8, id: u16) -> Option<mh3u_core::blacksmith::Unlock> {
        let recipe = self.game.recipe(kind, id)?;
        Some(mh3u_core::blacksmith::unlock(recipe, self.game.drops(), |m| {
            self.save.times_hunted(m)
        }))
    }

    /// Whether the piece should be on the blacksmith's list now: on offer or starting gear, or a weapon you own a parent of.
    pub fn at_blacksmith(&self, kind: u8, id: u16) -> bool {
        use mh3u_core::blacksmith::Unlock;
        match self.offer(kind, id) {
            Some(Offer::Earlier | Offer::Rule(Unlock::Starter | Unlock::Hunted(_))) => true,
            Some(Offer::Rule(_)) => false,
            None => self
                .upgrade_recipe(kind, id)
                .is_some_and(|u| u.parents.iter().any(|&p| self.save.owns_equipment(kind, p))),
        }
    }

    /// Remember every piece seen on offer: those whose monster has been hunted, and those whose price was seen or learned in play
    /// (it must have been on the list to be crafted). Saved next to the ledger when something new turns up.
    pub(super) fn learn_unlocked(&mut self) {
        use mh3u_core::blacksmith::Unlock;
        let hunter = self.save.hunter_name.clone();
        let mut changed = false;
        for kind in (1..=5u8).chain(7..=19) {
            for id in self.game.piece_ids(kind) {
                if matches!(self.rule(kind, id), Some(Unlock::Hunted(_))) {
                    changed |= self.unlocked.add(&hunter, (kind, id));
                }
            }
        }
        for e in self.prices.entries() {
            if matches!(e.source, Source::Seen | Source::Learned) && e.route == Route::Create {
                changed |= self.unlocked.add(&hunter, (e.kind, e.id));
            }
        }
        if changed
            && let Some(path) = &self.unlocked_path
            && let Err(message) = crate::files::save(path, &self.unlocked.format(), "unlocked pieces")
        {
            self.status = message;
        }
    }
}

//! The wishlist: the pieces you want, the parent weapons they need, and the shopping list for them.

use super::*;

/// The wishlist: the pieces wanted, which of them were added only as a parent of another, and the highlighted row.
pub struct WishList {
    /// Wishlisted pieces as (equipment kind, piece id), in the order they were added.
    pub items: Vec<(u8, u16)>,
    /// Wishlisted pieces that were added automatically as a parent of another piece (see `remove_wish`).
    pub(super) auto_parents: HashSet<(u8, u16)>,
    pub state: ListState,
}

impl WishList {
    pub(super) fn new(items: Vec<(u8, u16)>, auto_parents: HashSet<(u8, u16)>) -> WishList {
        WishList {
            items,
            auto_parents,
            state: ListState::default().with_selected(Some(0)),
        }
    }
}

/// Parse the wishlist file: `kind id` per line, with an optional trailing `auto` for pieces added as a parent of another.
/// Lines that don't parse are ignored.
pub(super) fn parse_wishlist(text: &str) -> Vec<(u8, u16, bool)> {
    text.lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let (kind, id) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
            Some((kind, id, parts.next() == Some("auto")))
        })
        .collect()
}

pub(super) fn format_wishlist(wishlist: &[(u8, u16)], auto: &HashSet<(u8, u16)>) -> String {
    wishlist
        .iter()
        .map(|&(kind, id)| format!("{kind} {id}{}\n", if auto.contains(&(kind, id)) { " auto" } else { "" }))
        .collect()
}

impl App {
    /// Items the wishlist still needs, as item id -> how many more are missing.
    pub fn missing_for_wishlist(&self) -> HashMap<u16, u32> {
        self.shopping_need()
            .0
            .into_iter()
            .filter_map(|(item, n)| {
                let have = self.save.item_count(item);
                n.checked_sub(have).filter(|&short| short > 0).map(|short| (item, short))
            })
            .collect()
    }

    pub fn is_wished(&self, kind: u8, id: u16) -> bool {
        self.wish.items.contains(&(kind, id))
    }

    /// Add a piece to the wishlist, plus the parent weapons it needs: the upgrade chain leading to it, back to
    /// the first weapon you own or can make from scratch. Parents go in before the piece.
    pub(super) fn add_wish_with_parents(&mut self, kind: u8, id: u16) {
        let chain = self.needed_parents(kind, id);
        let mut added = 0;
        for &parent in chain.iter().chain([&id]) {
            if !self.is_wished(kind, parent) {
                self.wish.items.push((kind, parent));
                if parent != id {
                    self.wish.auto_parents.insert((kind, parent));
                }
                added += 1;
            }
        }
        if added > 1 {
            self.status = format!(
                "added {} and {} parent weapon(s)",
                self.game.equipment_name(kind, id).unwrap_or("?"),
                added - 1
            );
        }
        self.after_wishlist_change();
    }

    /// The weapons to get before `id`, first to last: the steps of the cheapest way to it (see `cheapest_path`) other than the last,
    /// leaving out any you own. Shared by adding and removing, and ignores the wishlist.
    pub(super) fn needed_parents(&self, kind: u8, id: u16) -> Vec<u16> {
        self.cheapest_path(kind, id).map(|p| p.parents_needed()).unwrap_or_default()
    }

    /// Remove a piece, and the parents that were added automatically for it unless another wishlisted piece still
    /// needs them. Parents you added yourself are kept.
    pub(super) fn remove_wish(&mut self, kind: u8, id: u16) {
        self.wish.items.retain(|&w| w != (kind, id));
        self.wish.auto_parents.remove(&(kind, id));
        let mut removed = 0;
        // nearest first, so a parent that only the next one up needed goes too
        for parent in self.needed_parents(kind, id).into_iter().rev() {
            if !self.is_wished(kind, parent) || !self.wish.auto_parents.contains(&(kind, parent)) {
                continue;
            }
            let still_needed = self
                .wish
                .items
                .iter()
                .any(|&(k, other)| k == kind && other != parent && self.needed_parents(kind, other).contains(&parent));
            if !still_needed {
                self.wish.items.retain(|&w| w != (kind, parent));
                self.wish.auto_parents.remove(&(kind, parent));
                removed += 1;
            }
        }
        if removed > 0 {
            self.status = format!(
                "removed {} and {removed} parent weapon(s)",
                self.game.equipment_name(kind, id).unwrap_or("?")
            );
        }
    }

    pub(super) fn toggle_wish(&mut self, kind: u8, id: u16) {
        if self.is_wished(kind, id) {
            self.remove_wish(kind, id);
        } else {
            self.add_wish_with_parents(kind, id);
            return;
        }
        self.after_wishlist_change();
    }

    pub(super) fn after_wishlist_change(&mut self) {
        let sel = self.wish.state.selected().unwrap_or(0).min(self.wish.items.len().saturating_sub(1));
        self.wish.state.select(Some(sel));
        self.save_wishlist();
        self.hunts.stale = true;
        self.refresh_box();
        self.refresh_quests();
    }

    pub(super) fn save_wishlist(&mut self) {
        let Some(path) = self.files.as_ref().map(|f| &f.wishlist) else {
            return;
        };
        if let Err(message) = crate::files::save(path, &format_wishlist(&self.wish.items, &self.wish.auto_parents), "wishlist") {
            self.status = message;
        }
    }

    /// What the wishlist still needs: (item id, total needed) for every wishlisted piece you don't own, and how many such
    /// pieces there are. The same totals as the Wishlist tab's shopping list.
    pub fn shopping_need(&self) -> (Vec<(u16, u32)>, usize) {
        self.shopping_need_with(false)
    }

    /// Like `shopping_need`; with `include_owned`, wishlisted pieces you already own count too.
    pub(super) fn shopping_need_with(&self, include_owned: bool) -> (Vec<(u16, u32)>, usize) {
        let mut need: Vec<(u16, u32)> = Vec::new();
        let mut unowned = 0;
        for &(kind, id) in &self.wish.items {
            if !include_owned && self.save.owns_equipment(kind, id) {
                continue;
            }
            unowned += 1;
            if let Some(plan) = self.plan(kind, id) {
                for m in plan.materials {
                    match need.iter_mut().find(|(item, _)| *item == m.id) {
                        Some((_, n)) => *n += u32::from(m.count),
                        None => need.push((m.id, u32::from(m.count))),
                    }
                }
            }
        }
        (need, unowned)
    }

    /// Total known forging cost of the wishlist's planned routes, and how many pieces have no known cost.
    pub fn wishlist_cost(&self) -> (u64, usize) {
        let (mut known, mut unknown) = (0u64, 0usize);
        for &(kind, id) in &self.wish.items {
            if self.save.owns_equipment(kind, id) {
                continue;
            }
            let route = match self.plan(kind, id).map(|p| p.via) {
                Some(Via::Create) => Route::Create,
                Some(Via::Upgrade) => Route::Upgrade,
                None => continue,
            };
            match self.cost(kind, id, route) {
                Some((c, _)) => known += u64::from(c),
                None => unknown += 1,
            }
        }
        (known, unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wishlist_round_trips_with_auto_markers() {
        let list = vec![(7, 24), (5, 8), (7, 3)];
        let auto: HashSet<(u8, u16)> = [(7, 3)].into();
        let text = format_wishlist(&list, &auto);
        assert_eq!(text, "7 24\n5 8\n7 3 auto\n");
        assert_eq!(parse_wishlist(&text), vec![(7, 24, false), (5, 8, false), (7, 3, true)]);
    }

    #[test]
    fn wishlist_reads_the_older_two_column_format() {
        assert_eq!(parse_wishlist("7 24\n5 8\n"), vec![(7, 24, false), (5, 8, false)]);
    }

    #[test]
    fn wishlist_ignores_bad_lines() {
        assert_eq!(
            parse_wishlist("7 24\nnonsense\n\n5\n300 1\n5 8 extra\n"),
            vec![(7, 24, false), (5, 8, false)]
        );
    }
}

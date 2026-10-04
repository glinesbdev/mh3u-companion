//! The wishlist: the pieces you want, the parent weapons they need, and the shopping list for them.

use super::*;

/// How the wishlist is listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WishSort {
    /// The order the pieces were added (parents come before the piece that needs them).
    #[default]
    Added,
    Name,
    Type,
    /// What is still to get first; the pieces you own or marked done last.
    ToDo,
}

impl WishSort {
    fn next(self) -> WishSort {
        match self {
            WishSort::Added => WishSort::Name,
            WishSort::Name => WishSort::Type,
            WishSort::Type => WishSort::ToDo,
            WishSort::ToDo => WishSort::Added,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WishSort::Added => "added",
            WishSort::Name => "name",
            WishSort::Type => "type",
            WishSort::ToDo => "to do first",
        }
    }
}

/// The wishlist: the pieces wanted, which of them were added only as a parent of another, which are marked done, and the highlighted row.
pub struct WishList {
    /// Wishlisted pieces as (equipment kind, piece id), in the order they were added.
    pub items: Vec<(u8, u16)>,
    /// Wishlisted pieces that were added automatically as a parent of another piece (see `remove_wish`).
    pub(super) auto_parents: HashSet<(u8, u16)>,
    /// Pieces marked done without owning them (you made another, or no longer want to): left out of the shopping list and the costs.
    pub(super) done: HashSet<(u8, u16)>,
    pub sort: WishSort,
    /// The rows shown, in order: indexes into `items`. The highlighted row is an index into this.
    pub view: Vec<usize>,
    pub state: ListState,
}

impl WishList {
    pub(super) fn new(items: Vec<(u8, u16)>, auto_parents: HashSet<(u8, u16)>, done: HashSet<(u8, u16)>) -> WishList {
        let view = (0..items.len()).collect();
        WishList {
            items,
            auto_parents,
            done,
            sort: WishSort::Added,
            view,
            state: ListState::default().with_selected(Some(0)),
        }
    }

    /// The piece on a row of the list.
    pub fn at(&self, row: usize) -> Option<(u8, u16)> {
        self.view.get(row).and_then(|&i| self.items.get(i)).copied()
    }

    /// The highlighted piece.
    pub fn selected(&self) -> Option<(u8, u16)> {
        self.state.selected().and_then(|row| self.at(row))
    }
}

/// Parse the wishlist file: `kind id` per line, with optional trailing words: `auto` for pieces added as a parent of another and
/// `done` for pieces marked done. Lines that don't parse are ignored.
pub(super) fn parse_wishlist(text: &str) -> Vec<(u8, u16, bool, bool)> {
    text.lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let (kind, id) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
            let flags: Vec<&str> = parts.collect();
            Some((kind, id, flags.contains(&"auto"), flags.contains(&"done")))
        })
        .collect()
}

pub(super) fn format_wishlist(wishlist: &[(u8, u16)], auto: &HashSet<(u8, u16)>, done: &HashSet<(u8, u16)>) -> String {
    wishlist
        .iter()
        .map(|&(kind, id)| {
            format!(
                "{kind} {id}{}{}\n",
                if auto.contains(&(kind, id)) { " auto" } else { "" },
                if done.contains(&(kind, id)) { " done" } else { "" }
            )
        })
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

    /// A wishlisted piece marked done.
    pub fn is_done(&self, kind: u8, id: u16) -> bool {
        self.wish.done.contains(&(kind, id))
    }

    /// Mark the highlighted piece done, or undo it.
    pub(super) fn toggle_done(&mut self) {
        let Some(piece) = self.wish.selected() else { return };
        if !self.wish.done.remove(&piece) {
            self.wish.done.insert(piece);
        }
        self.after_wishlist_change();
    }

    pub(super) fn cycle_wish_sort(&mut self) {
        self.wish.sort = self.wish.sort.next();
        self.refresh_wish_view();
    }

    /// Put the rows in order for the sort, keeping the same piece highlighted.
    pub(super) fn refresh_wish_view(&mut self) {
        let kept = self.wish.selected();
        let name = |i: usize| {
            let (kind, id) = self.wish.items[i];
            self.game.equipment_name(kind, id).unwrap_or("?").to_lowercase()
        };
        let mut view: Vec<usize> = (0..self.wish.items.len()).collect();
        match self.wish.sort {
            WishSort::Added => {}
            WishSort::Name => view.sort_by_key(|&i| (name(i), i)),
            WishSort::Type => view.sort_by_key(|&i| (kind_rank(self.wish.items[i].0), name(i), i)),
            WishSort::ToDo => view.sort_by_key(|&i| {
                let (kind, id) = self.wish.items[i];
                (self.save.owns_equipment(kind, id) || self.is_done(kind, id), i)
            }),
        }
        self.wish.view = view;
        let at = kept
            .and_then(|k| self.wish.view.iter().position(|&i| self.wish.items[i] == k))
            .unwrap_or_else(|| self.wish.state.selected().unwrap_or(0).min(self.wish.view.len().saturating_sub(1)));
        self.wish.state.select((!self.wish.view.is_empty()).then_some(at));
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
        self.wish.done.retain(|p| self.wish.items.contains(p));
        self.refresh_wish_view();
        self.save_wishlist();
        self.hunts.stale = true;
        self.refresh_box();
        self.refresh_quests();
    }

    pub(super) fn save_wishlist(&mut self) {
        let Some(path) = self.files.as_ref().map(|f| &f.wishlist) else {
            return;
        };
        if let Err(message) = crate::files::save(
            path,
            &format_wishlist(&self.wish.items, &self.wish.auto_parents, &self.wish.done),
            "wishlist",
        ) {
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
            if self.is_done(kind, id) || (!include_owned && self.save.owns_equipment(kind, id)) {
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

    /// The shopping list as plain text, to paste into a note: what is still missing, by name, then the fees.
    pub fn shopping_text(&self) -> String {
        let (need, pieces) = self.shopping_need();
        let mut lines: Vec<(String, u32, u32, u32)> = need
            .iter()
            .filter_map(|&(item, total)| {
                let have = self.save.item_count(item);
                let missing = total.checked_sub(have).filter(|&m| m > 0)?;
                Some((self.game.item_name(item).unwrap_or("?").to_string(), missing, have, total))
            })
            .collect();
        lines.sort();
        let mut out = format!("Shopping list for {}: {pieces} piece(s) still to make\n", self.save.hunter_name);
        if lines.is_empty() {
            out.push_str("Nothing is missing.\n");
        }
        for (name, missing, have, total) in &lines {
            out.push_str(&format!("{name} x{missing} (have {have} of {total})\n"));
        }
        let (fees, unknown) = self.wishlist_cost();
        out.push_str(&format!("Forging fees: {}z", group_digits(fees)));
        if unknown > 0 {
            out.push_str(&format!(" ({unknown} piece(s) with no known fee)"));
        }
        out.push('\n');
        out
    }

    /// Write the shopping list to its file (see `Files::shopping`) and say where.
    pub(super) fn export_shopping_list(&mut self) {
        let Some(path) = self.files.as_ref().map(|f| f.shopping.clone()) else {
            return self.status = "no data folder to write the shopping list to".into();
        };
        self.status = match crate::files::save(&path, &self.shopping_text(), "shopping list") {
            Ok(()) => format!("shopping list written to {}", path.display()),
            Err(e) => e,
        };
    }

    /// Total known forging cost of the wishlist's planned routes, and how many pieces have no known cost.
    pub fn wishlist_cost(&self) -> (u64, usize) {
        let (mut known, mut unknown) = (0u64, 0usize);
        for &(kind, id) in &self.wish.items {
            if self.save.owns_equipment(kind, id) || self.is_done(kind, id) {
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
        let done: HashSet<(u8, u16)> = [(5, 8), (7, 3)].into();
        let text = format_wishlist(&list, &auto, &done);
        assert_eq!(text, "7 24\n5 8 done\n7 3 auto done\n");
        assert_eq!(
            parse_wishlist(&text),
            vec![(7, 24, false, false), (5, 8, false, true), (7, 3, true, true)]
        );
    }

    #[test]
    fn wishlist_reads_the_older_two_column_format() {
        assert_eq!(parse_wishlist("7 24\n5 8\n"), vec![(7, 24, false, false), (5, 8, false, false)]);
    }

    #[test]
    fn wishlist_ignores_bad_lines() {
        assert_eq!(
            parse_wishlist("7 24\nnonsense\n\n5\n300 1\n5 8 extra\n5 9 done\n"),
            vec![(7, 24, false, false), (5, 8, false, false), (5, 9, false, true)]
        );
    }
}

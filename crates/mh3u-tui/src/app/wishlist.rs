//! The wishlist: the pieces you want, the parent weapons they need, and the shopping list for them.

use super::*;

/// Node of the upgrade tree for `parent_chain`.
pub(super) struct TreeNode {
    /// Can be made from scratch, so it never needs a parent.
    can_create: bool,
    parents: Vec<u16>,
}

/// The ancestors to add along with a wishlisted piece, nearest first. Walks up while the piece can only be
/// obtained by upgrading and no parent is owned; stops at an ancestor that is owned, wishlisted, or can be
/// created from scratch.
pub(super) fn parent_chain(
    start: u16,
    node: impl Fn(u16) -> TreeNode,
    owned: impl Fn(u16) -> bool,
    wished: impl Fn(u16) -> bool,
) -> Vec<u16> {
    let mut chain = Vec::new();
    let mut cur = start;
    for _ in 0..64 {
        let n = node(cur);
        if n.can_create || n.parents.iter().any(|&p| owned(p)) {
            break;
        }
        // Prefer a parent that is already wishlisted, which ends the walk.
        if n.parents.iter().any(|&p| wished(p)) {
            break;
        }
        let Some(&parent) = n.parents.first() else { break };
        if chain.contains(&parent) || parent == start {
            break; // guard against a cycle in the data
        }
        chain.push(parent);
        cur = parent;
    }
    chain
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
                (have < n).then_some((item, n - have))
            })
            .collect()
    }

    pub fn is_wished(&self, kind: u8, id: u16) -> bool {
        self.wishlist.contains(&(kind, id))
    }

    /// Add a piece to the wishlist, plus the parent weapons it needs: the upgrade chain leading to it, back to
    /// the first weapon you own or can make from scratch. Parents go in before the piece.
    pub(super) fn add_wish_with_parents(&mut self, kind: u8, id: u16) {
        let chain = parent_chain(
            id,
            |p| TreeNode {
                can_create: self.recipes_for(kind, p).0.is_some(),
                parents: self.upgrade_recipe(kind, p).map(|u| u.parents.clone()).unwrap_or_default(),
            },
            |p| self.save.owns_equipment(kind, p),
            |p| self.is_wished(kind, p),
        );
        let mut added = 0;
        for &parent in chain.iter().rev().chain([&id]) {
            if !self.is_wished(kind, parent) {
                self.wishlist.push((kind, parent));
                if parent != id {
                    self.auto_parents.insert((kind, parent));
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

    /// The weapon-tree rules shared by adding and removing: the parents `id` needs, ignoring the wishlist.
    pub(super) fn needed_parents(&self, kind: u8, id: u16) -> Vec<u16> {
        parent_chain(
            id,
            |p| TreeNode {
                can_create: self.recipes_for(kind, p).0.is_some(),
                parents: self.upgrade_recipe(kind, p).map(|u| u.parents.clone()).unwrap_or_default(),
            },
            |p| self.save.owns_equipment(kind, p),
            |_| false,
        )
    }

    /// Remove a piece, and the parents that were added automatically for it unless another wishlisted piece still
    /// needs them. Parents you added yourself are kept.
    pub(super) fn remove_wish(&mut self, kind: u8, id: u16) {
        self.wishlist.retain(|&w| w != (kind, id));
        self.auto_parents.remove(&(kind, id));
        let mut removed = 0;
        for parent in self.needed_parents(kind, id) {
            if !self.is_wished(kind, parent) || !self.auto_parents.contains(&(kind, parent)) {
                continue;
            }
            let still_needed = self
                .wishlist
                .iter()
                .any(|&(k, other)| k == kind && other != parent && self.needed_parents(kind, other).contains(&parent));
            if !still_needed {
                self.wishlist.retain(|&w| w != (kind, parent));
                self.auto_parents.remove(&(kind, parent));
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
        let sel = self.wish_state.selected().unwrap_or(0).min(self.wishlist.len().saturating_sub(1));
        self.wish_state.select(Some(sel));
        self.save_wishlist();
    }

    pub(super) fn save_wishlist(&mut self) {
        let Some(path) = &self.wishlist_path else { return };
        if let Err(message) = crate::files::save(path, &format_wishlist(&self.wishlist, &self.auto_parents), "wishlist") {
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
        for &(kind, id) in &self.wishlist {
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
        for &(kind, id) in &self.wishlist {
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

    /// A tiny upgrade tree: id -> (can create from scratch, parents).
    fn tree(entries: &[(u16, bool, &[u16])]) -> impl Fn(u16) -> TreeNode {
        let map: HashMap<u16, (bool, Vec<u16>)> = entries.iter().map(|&(id, c, p)| (id, (c, p.to_vec()))).collect();
        move |id| {
            let (can_create, parents) = map.get(&id).cloned().unwrap_or((false, Vec::new()));
            TreeNode { can_create, parents }
        }
    }

    #[test]
    fn chain_walks_up_to_a_weapon_that_can_be_created() {
        // 4 <- 3 <- 2 <- 1, where only 1 can be made from scratch.
        let t = tree(&[(1, true, &[]), (2, false, &[1]), (3, false, &[2]), (4, false, &[3])]);
        assert_eq!(parent_chain(4, &t, |_| false, |_| false), vec![3, 2, 1]);
    }

    #[test]
    fn chain_stops_at_an_owned_parent() {
        let t = tree(&[(1, true, &[]), (2, false, &[1]), (3, false, &[2]), (4, false, &[3])]);
        assert_eq!(parent_chain(4, &t, |p| p == 2, |_| false), vec![3]);
        assert!(parent_chain(3, &t, |p| p == 2, |_| false).is_empty());
    }

    #[test]
    fn chain_adds_nothing_for_a_piece_that_can_be_created() {
        let t = tree(&[(1, true, &[]), (2, true, &[1])]);
        assert!(parent_chain(2, &t, |_| false, |_| false).is_empty());
    }

    #[test]
    fn chain_stops_at_a_wishlisted_parent() {
        let t = tree(&[(1, true, &[]), (2, false, &[1]), (3, false, &[2])]);
        assert!(parent_chain(3, &t, |_| false, |p| p == 2).is_empty());
    }

    #[test]
    fn chain_survives_a_cycle_and_a_missing_node() {
        let t = tree(&[(1, false, &[2]), (2, false, &[1])]);
        assert_eq!(parent_chain(1, &t, |_| false, |_| false), vec![2]);
        assert!(parent_chain(9, &t, |_| false, |_| false).is_empty());
    }

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

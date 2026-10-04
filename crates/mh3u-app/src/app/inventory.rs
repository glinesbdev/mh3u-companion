//! The Items and Equipment tabs: the pouch, the item box, the equipment box and the worn gear.

use super::*;
use crate::surplus::{self, Demand, Spare};
use mh3u_core::gamedata::MaterialUse;

/// The Items and Equipment tabs: the highlighted rows, how the lists are ordered and filtered, and the lists as shown.
pub struct Inventory {
    pub box_state: ListState,
    pub pouch_state: ListState,
    /// The Items tab's highlight (and details) follow the pouch rather than the box.
    pub pouch_focus: bool,
    pub equip_state: ListState,
    pub box_sort: BoxSort,
    pub equip_sort: EquipSort,
    /// Search text for the Items tab.
    pub item_search: String,
    /// Search text for the Equipment tab (by name).
    pub equip_search: String,
    /// The equipment box in display order: indexes into `save.equipment_box` (see `equip_sort`).
    pub equip_view: Vec<usize>,
    /// The item box in display order (see `box_sort`).
    pub box_view: Vec<ItemStack>,
    /// The item pouch, filtered by the item search.
    pub pouch_view: Vec<ItemStack>,
    /// Which recipes take each item (from the game data, worked out once).
    pub(super) uses: HashMap<u16, Vec<MaterialUse>>,
    /// For each item held: how many can go (see `surplus`). Worked out again whenever the save or the wishlist changes.
    pub spare: HashMap<u16, Spare>,
    /// Show only the items with some spare.
    pub spare_only: bool,
}

impl Default for Inventory {
    fn default() -> Inventory {
        Inventory {
            box_state: ListState::default().with_selected(Some(0)),
            pouch_state: ListState::default().with_selected(Some(0)),
            pouch_focus: false,
            equip_state: ListState::default().with_selected(Some(0)),
            box_sort: BoxSort::BoxOrder,
            equip_sort: EquipSort::BoxOrder,
            item_search: String::new(),
            equip_search: String::new(),
            equip_view: Vec::new(),
            box_view: Vec::new(),
            pouch_view: Vec::new(),
            uses: HashMap::new(),
            spare: HashMap::new(),
            spare_only: false,
        }
    }
}

/// The upgrade tree popup for one weapon.
pub struct TreeView {
    pub kind: u8,
    pub tree: crate::tree::Tree,
    /// First visible line; the drawing code keeps it inside the tree.
    pub scroll: u16,
}

impl App {
    /// Items whose names match the item search, best match first (or unfiltered, in their given order).
    pub(super) fn filter_items(&self, stacks: &[ItemStack]) -> Vec<(u32, ItemStack)> {
        let words: Vec<String> = self.inv.item_search.split_whitespace().map(str::to_lowercase).collect();
        stacks
            .iter()
            .filter_map(|&stack| {
                let name = self.game.item_name(stack.id).unwrap_or("?").to_lowercase();
                let mut total = 0;
                for w in &words {
                    total += search::score(w, &name)?;
                }
                let phrase = words.join(" ");
                if name == phrase {
                    total += 6000;
                } else if words.len() > 1 && name.contains(&phrase) {
                    total += 3000;
                }
                Some((total, stack))
            })
            .collect()
    }

    /// Rebuild the pouch and item box lists for the current search and sort order.
    /// Work out how many of each held item are spare.
    pub(super) fn refresh_spare(&mut self) {
        let wishlist: HashMap<u16, u32> = self.shopping_need().0.into_iter().collect();
        let held: HashSet<u16> = self.save.pouch.iter().chain(&self.save.item_box).map(|s| s.id).collect();
        self.inv.spare = held
            .into_iter()
            .map(|item| {
                let mut demand = Demand {
                    wishlist: wishlist.get(&item).copied().unwrap_or(0),
                    ..Demand::default()
                };
                for u in self.inv.uses.get(&item).into_iter().flatten() {
                    if self.save.owns_equipment(u.kind, u.id) {
                        demand.owned_uses += 1;
                    } else {
                        demand.unowned_uses += 1;
                        demand.biggest_piece = demand.biggest_piece.max(u32::from(u.count));
                    }
                }
                (item, surplus::assess(self.save.item_count(item), &demand))
            })
            .collect();
    }

    pub(super) fn refresh_box(&mut self) {
        self.refresh_spare();
        let searching = !self.inv.item_search.trim().is_empty();
        let mut pouch = self.filter_items(&self.save.pouch);
        if searching {
            pouch.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        }
        self.inv.pouch_view = pouch.into_iter().map(|(_, s)| s).collect();
        if self.inv.spare_only {
            let spare = &self.inv.spare;
            self.inv.pouch_view.retain(|s| spare.get(&s.id).is_some_and(|p| p.spare > 0));
        }

        let mut view = self.filter_items(&self.save.item_box);
        match self.inv.box_sort {
            BoxSort::BoxOrder if searching => view.sort_by_key(|(score, _)| std::cmp::Reverse(*score)),
            BoxSort::BoxOrder => {}
            BoxSort::Name => view.sort_by_key(|(_, s)| self.game.item_name(s.id).unwrap_or("?").to_lowercase()),
            BoxSort::Quantity => view.sort_by_key(|(_, s)| std::cmp::Reverse(s.count)),
        }
        self.inv.box_view = view.into_iter().map(|(_, s)| s).collect();
        if self.inv.spare_only {
            let spare = &self.inv.spare;
            self.inv.box_view.retain(|s| spare.get(&s.id).is_some_and(|p| p.spare > 0));
        }
        let sel = self
            .inv
            .box_state
            .selected()
            .unwrap_or(0)
            .min(self.inv.box_view.len().saturating_sub(1));
        self.inv.box_state.select(Some(sel));
        let sel = self
            .inv
            .pouch_state
            .selected()
            .unwrap_or(0)
            .min(self.inv.pouch_view.len().saturating_sub(1));
        self.inv.pouch_state.select(Some(sel));
    }

    /// Put the equipment box in display order, keeping the same item selected when it is still there.
    pub(super) fn refresh_equipment(&mut self) {
        let kept = self.inv.equip_state.selected().and_then(|i| self.inv.equip_view.get(i)).copied();
        let boxed = &self.save.equipment_box;
        let name = |i: usize| self.game.equipment_name(boxed[i].kind, boxed[i].id).unwrap_or("?").to_lowercase();
        let rarity = |i: usize| self.game.equipment_rarity(boxed[i].kind, boxed[i].id).unwrap_or(0);
        let words: Vec<String> = self.inv.equip_search.split_whitespace().map(str::to_lowercase).collect();
        // every word must find the name; the sum of the scores ranks the matches
        let found = |i: usize| -> Option<u32> {
            let n = name(i);
            words.iter().try_fold(0u32, |sum, w| Some(sum + crate::search::score(w, &n)?))
        };
        let mut view: Vec<usize> = (0..boxed.len()).filter(|&i| found(i).is_some()).collect();
        let attack = |i: usize| self.game.weapon_stats(boxed[i].kind, boxed[i].id).map_or(0, |w| w.attack);
        let defense = |i: usize| self.game.armor_stats(boxed[i].kind, boxed[i].id).map_or(0, |a| a.defense);
        match self.inv.equip_sort {
            EquipSort::BoxOrder if !words.is_empty() => view.sort_by_key(|&i| std::cmp::Reverse(found(i))),
            EquipSort::BoxOrder => {}
            EquipSort::Attack => view.sort_by_key(|&i| (std::cmp::Reverse(attack(i)), std::cmp::Reverse(defense(i)), name(i))),
            EquipSort::Defense => view.sort_by_key(|&i| (std::cmp::Reverse(defense(i)), std::cmp::Reverse(attack(i)), name(i))),
            EquipSort::Name => view.sort_by_key(|&i| (name(i), kind_rank(boxed[i].kind))),
            EquipSort::Rarity => view.sort_by_key(|&i| (std::cmp::Reverse(rarity(i)), kind_rank(boxed[i].kind), name(i))),
            EquipSort::Type => view.sort_by_key(|&i| (kind_rank(boxed[i].kind), name(i))),
            EquipSort::WornFirst => view.sort_by_key(|&i| (!self.save.is_worn(&boxed[i]), kind_rank(boxed[i].kind), name(i))),
        }
        let at = kept
            .and_then(|k| view.iter().position(|&i| i == k))
            .unwrap_or_else(|| self.inv.equip_state.selected().unwrap_or(0).min(view.len().saturating_sub(1)));
        self.inv.equip_view = view;
        self.inv.equip_state.select(Some(at));
    }

    /// Whether the Items tab's highlight is on the pouch: when asked for with `p`, or when the box list has nothing to highlight.
    pub fn items_on_pouch(&self) -> bool {
        !self.inv.pouch_view.is_empty() && (self.inv.pouch_focus || self.inv.box_view.is_empty())
    }

    /// The worn armor pieces as (equipment kind, box entry), in the order head, body, arms, waist, legs.
    pub fn worn_armor(&self) -> Vec<(u8, &mh3u_core::save::Equipment)> {
        [5u8, 1, 2, 3, 4]
            .into_iter()
            .filter_map(|kind| {
                let e = self.save.equipment_box.iter().find(|e| e.kind == kind && self.save.is_worn(e))?;
                Some((kind, e))
            })
            .collect()
    }

    /// The worn talisman, if any (the save points to its equipment box slot).
    pub fn worn_talisman(&self) -> Option<&mh3u_core::save::Equipment> {
        let slot = self.save.worn_talisman?;
        self.save.equipment_box.iter().find(|e| e.slot == slot && e.kind == 6)
    }

    /// The worn weapon, if any.
    pub fn worn_weapon(&self) -> Option<&mh3u_core::save::Equipment> {
        self.save
            .equipment_box
            .iter()
            .find(|e| (7..=19).contains(&e.kind) && self.save.is_worn(e))
    }

    /// The equipment-box entry that is highlighted on the Equipment tab.
    pub fn selected_equipment(&self) -> Option<&mh3u_core::save::Equipment> {
        let i = *self.inv.equip_view.get(self.inv.equip_state.selected()?)?;
        self.save.equipment_box.get(i)
    }

    /// The weapon highlighted on the current tab, as (kind, id). `None` on tabs with no selection.
    pub fn highlighted_equipment(&self) -> Option<(u8, u16)> {
        match self.tab {
            Tab::Crafting => self
                .craft
                .state
                .selected()
                .and_then(|i| self.craft.pieces.get(i))
                .map(|p| (p.kind, p.id)),
            Tab::Equipment => self.selected_equipment().map(|e| (e.kind, e.id)),
            Tab::Wishlist => self.wish.state.selected().and_then(|i| self.wish.items.get(i)).copied(),
            Tab::Items
            | Tab::Worn
            | Tab::Monsters
            | Tab::Hunts
            | Tab::Quests
            | Tab::Families
            | Tab::Skills
            | Tab::Compare
            | Tab::Gains
            | Tab::Builds => None,
        }
    }

    /// The weapons (same kind) that `id` is upgraded from, from the game data or learned in play.
    pub fn upgrade_parents(&self, kind: u8, id: u16) -> Vec<u16> {
        self.upgrade_recipe(kind, id)
            .map(|u| u.parents.iter().copied().filter(|&p| p != 0).collect())
            .unwrap_or_default()
    }

    /// The weapons (same kind) that `id` can be upgraded into, from the game data or learned in play.
    pub fn upgrade_children(&self, kind: u8, id: u16) -> Vec<u16> {
        let mut kids = self.game.upgrade_children(kind, id);
        kids.extend(
            self.craft
                .learned_upgrade
                .iter()
                .filter(|((k, child), u)| *k == kind && *child != id && u.parents.contains(&id))
                .map(|((_, child), _)| *child),
        );
        kids.sort_unstable();
        kids.dedup();
        kids
    }

    /// Open the upgrade tree for the highlighted weapon. Armor has no upgrade line in the game data, so nothing opens for it.
    pub(super) fn open_tree(&mut self) {
        let Some((kind, id)) = self.highlighted_equipment() else { return };
        if !(7..=19).contains(&kind) || kind == 12 {
            self.status = "Only weapons have an upgrade tree.".to_string();
            return;
        }
        let tree = crate::tree::build(id, &|w| self.upgrade_parents(kind, w), &|w| self.upgrade_children(kind, w), 400);
        let scroll = tree.selected_row.saturating_sub(3) as u16;
        self.tree = Some(TreeView { kind, tree, scroll });
    }

    /// The sort label for the item box: with a search, the default order is "best match".
    pub fn box_sort_label(&self) -> &'static str {
        if self.inv.box_sort == BoxSort::BoxOrder && !self.inv.item_search.trim().is_empty() {
            "best match"
        } else {
            self.inv.box_sort.label()
        }
    }
}

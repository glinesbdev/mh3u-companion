//! The Crafting tab: pieces, their recipes, what can be made now, search and sort, and what a piece costs.

use super::*;

/// The Crafting tab: the pieces listed, the filters and sort on them, the search, and the recipes learned from play.
pub struct Crafting {
    pub state: ListState,
    pub search: String,
    pub craftable_only: bool,
    pub hide_owned: bool,
    /// Show only pieces you own that have no forging cost in the ledger yet.
    pub unpriced_only: bool,
    /// Show only pieces the blacksmith is offering (see `mh3u_core::blacksmith`).
    pub blacksmith_only: bool,
    pub sort: PieceSort,
    pub pieces: Vec<Piece>,
    pub(super) catalog: Vec<Searchable>,
    /// Recipes learned from play (see `prices::Outcome::Learned`), for pieces the game data has no recipe for.
    pub(super) learned_create: HashMap<(u8, u16), Recipe>,
    pub(super) learned_upgrade: HashMap<(u8, u16), Upgrade>,
}

impl Default for Crafting {
    fn default() -> Crafting {
        Crafting {
            state: ListState::default().with_selected(Some(0)),
            search: String::new(),
            craftable_only: false,
            hide_owned: false,
            unpriced_only: false,
            blacksmith_only: false,
            sort: PieceSort::GameOrder,
            pieces: Vec::new(),
            catalog: Vec::new(),
            learned_create: HashMap::new(),
            learned_upgrade: HashMap::new(),
        }
    }
}

/// How a piece would be obtained.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Create,
    Upgrade,
}

/// The route chosen for a piece and the materials it needs.
pub struct Plan {
    pub via: Via,
    pub materials: Vec<ItemStack>,
    /// For upgrades: whether a parent weapon is in the equipment box.
    pub parent_owned: bool,
}

/// A craftable piece shown in the crafting list.
pub struct Piece {
    pub kind: u8,
    pub id: u16,
    pub name: String,
    /// True when the piece can be made now: from scratch with the materials in the pouch and box, or
    /// (weapons) by upgrading a parent weapon that is in the equipment box.
    pub craftable: bool,
    /// True when the equipment box already holds this piece.
    pub owned: bool,
    /// True when the blacksmith is offering the piece (see `App::at_blacksmith`).
    pub offered: bool,
    /// When searching: why the piece matched, if not by name (e.g. "skill: Poison").
    pub reason: Option<String>,
}

/// Everything about a piece that the search looks at, lowercased once up front.
pub(super) struct Searchable {
    kind: u8,
    id: u16,
    name: String,
    name_lc: String,
    kind_lc: String,
    /// (display name, lowercase name)
    skills: Vec<(String, String)>,
    materials: Vec<(String, String)>,
    rarity: Option<u8>,
    /// Lowercase labels the piece can be found by: `male`, `female`, `blademaster`, `gunner`. A piece for both
    /// genders (or classes) carries both labels.
    tags: Vec<&'static str>,
}

impl Searchable {
    /// Score against the query words (all lowercase). Every word must match something. Returns the score and,
    /// if a word matched only a skill or material, that match as a reason.
    fn matches(&self, words: &[String]) -> Option<(u32, Option<String>)> {
        let (mut total, mut reason) = (0, None);
        for w in words {
            if let Some(n) = search::rarity_token(w) {
                if self.rarity == Some(n) {
                    continue;
                }
                return None;
            }
            // (weight, score, reason) of the best field for this word
            let mut best: Option<(u32, Option<String>)> = None;
            let mut consider = |weight: u32, sc: Option<u32>, why: Option<String>| {
                if let Some(sc) = sc {
                    let v = weight * sc;
                    if best.as_ref().is_none_or(|(b, _)| v > *b) {
                        best = Some((v, why));
                    }
                }
            };
            consider(3, search::score(w, &self.name_lc), None);
            consider(2, search::score(w, &self.kind_lc), None);
            for tag in &self.tags {
                consider(2, search::score_tag(w, tag), None);
            }
            for (shown, lc) in &self.skills {
                consider(2, search::score(w, lc), Some(format!("skill: {shown}")));
            }
            for (shown, lc) in &self.materials {
                consider(1, search::score(w, lc), Some(format!("needs: {shown}")));
            }
            let (v, why) = best?;
            total += v;
            reason = reason.or(why);
        }
        // Typing a whole name should put that piece first: a words-apart match can't tell "alloy cap" from "alloy cap s".
        let phrase = words.join(" ");
        if self.name_lc == phrase {
            total += 6000;
        } else if words.len() > 1 && self.name_lc.contains(&phrase) {
            total += 3000;
        }
        Some((total, reason))
    }
}

impl RecipeBook for GameBook<'_> {
    fn create_materials(&self, kind: u8, id: u16) -> Option<Vec<ItemStack>> {
        let found = self.game.recipe(kind, id).or_else(|| self.create.get(&(kind, id)));
        found.map(|r| r.materials.clone())
    }

    fn upgrade(&self, kind: u8, id: u16) -> Option<(Vec<ItemStack>, Vec<u16>)> {
        let found = self.game.upgrade(kind, id).or_else(|| self.upgrade.get(&(kind, id)));
        found.map(|u| (u.materials.clone(), u.parents.clone()))
    }
}

/// Where a kind of equipment sits when pieces are grouped: head, body, arms, waist, legs, talisman, then the weapons.
pub(super) fn kind_rank(kind: u8) -> u8 {
    match kind {
        5 => 0, // head
        1 => 1, // body
        2 => 2, // arms
        3 => 3, // waist
        4 => 4, // legs
        6 => 5, // talisman
        other => other,
    }
}

/// A search hit this strong (per query word, on average) is a match on the piece's name, type or skill; weaker ones are loose
/// fuzzy matches or pieces that merely need a material with that name.
pub(super) const STRONG_PER_WORD: u32 = 2000;

/// Order the crafting list. Like pieces always sit together (see `kind_rank`); the chosen sort orders the pieces within
/// each group, except that "craftable first" and "owned first" split the list by that flag before grouping. While
/// searching (`words` > 0), the best match comes first within a group, otherwise game order; and in the default (best
/// match) order the strong matches (see `STRONG_PER_WORD`) come first, grouped, then the weak ones, grouped, so a pile of
/// loose matches in one slot can't bury the real hits in another.
pub(super) fn sort_pieces(pieces: &mut [(u32, Piece)], sort: PieceSort, words: usize) {
    use std::cmp::Ordering;
    let searching = words > 0;
    let by_strength = searching && sort == PieceSort::GameOrder;
    let strong = |p: &(u32, Piece)| p.0 >= STRONG_PER_WORD * words as u32;
    let within = |a: &(u32, Piece), b: &(u32, Piece)| {
        let by_score = if searching { b.0.cmp(&a.0) } else { Ordering::Equal };
        by_score.then(a.1.id.cmp(&b.1.id))
    };
    let group = |a: &(u32, Piece), b: &(u32, Piece)| {
        let tier = if by_strength { strong(b).cmp(&strong(a)) } else { Ordering::Equal };
        tier.then(kind_rank(a.1.kind).cmp(&kind_rank(b.1.kind)))
    };
    pieces.sort_by(|a, b| match sort {
        PieceSort::GameOrder => group(a, b).then_with(|| within(a, b)),
        PieceSort::Name => group(a, b).then_with(|| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase())),
        PieceSort::CraftableFirst => (!a.1.craftable)
            .cmp(&!b.1.craftable)
            .then_with(|| group(a, b))
            .then_with(|| within(a, b)),
        PieceSort::OwnedFirst => (!a.1.owned).cmp(&!b.1.owned).then_with(|| group(a, b)).then_with(|| within(a, b)),
    });
}

impl App {
    /// The create recipe and upgrade recipe of a piece, ignoring any that name unreleased placeholder items.
    pub(super) fn recipes_for(&self, kind: u8, id: u16) -> (Option<&Recipe>, Option<&Upgrade>) {
        let placeholder = |m: &ItemStack| self.game.item_name(m.id).is_none_or(|n| n.starts_with("DUMMY"));
        let create = self
            .game
            .recipe(kind, id)
            .filter(|r| !r.materials.iter().any(placeholder))
            .or_else(|| self.craft.learned_create.get(&(kind, id)));
        let upgrade = self
            .game
            .upgrade(kind, id)
            .filter(|u| !u.materials.iter().any(placeholder))
            .or_else(|| self.craft.learned_upgrade.get(&(kind, id)));
        (create, upgrade)
    }

    /// The create recipe of a piece from the game data, or learned from play.
    pub fn create_recipe(&self, kind: u8, id: u16) -> Option<&Recipe> {
        self.game.recipe(kind, id).or_else(|| self.craft.learned_create.get(&(kind, id)))
    }

    /// The upgrade recipe of a weapon from the game data, or learned from play.
    pub fn upgrade_recipe(&self, kind: u8, id: u16) -> Option<&Upgrade> {
        self.game.upgrade(kind, id).or_else(|| self.craft.learned_upgrade.get(&(kind, id)))
    }

    /// Rebuild the recipe lookups from the ledger's learned entries.
    pub(super) fn rebuild_learned(&mut self) {
        self.craft.learned_create.clear();
        self.craft.learned_upgrade.clear();
        for e in self.costs.ledger.learned() {
            let materials = e.materials.clone().unwrap_or_default();
            match e.route {
                Route::Create => {
                    self.craft.learned_create.insert(
                        (e.kind, e.id),
                        Recipe {
                            materials,
                            flag: 0,
                            tier: 0,
                        },
                    );
                }
                Route::Upgrade => {
                    let parents = e.parent.into_iter().collect();
                    self.craft.learned_upgrade.insert((e.kind, e.id), Upgrade { materials, parents });
                }
            }
        }
    }

    /// The route to obtain a piece. Upgrading consumes the parent weapon, so a piece that can be made from scratch
    /// is always planned that way; upgrading is used only for pieces with no create recipe.
    pub fn plan(&self, kind: u8, id: u16) -> Option<Plan> {
        let (create, upgrade) = self.recipes_for(kind, id);
        let parent_owned = upgrade.is_some_and(|u| u.parents.iter().any(|&p| self.save.owns_equipment(kind, p)));
        match (create, upgrade) {
            (Some(r), _) => Some(Plan {
                via: Via::Create,
                materials: r.materials.clone(),
                parent_owned,
            }),
            (None, Some(u)) => Some(Plan {
                via: Via::Upgrade,
                materials: u.materials.clone(),
                parent_owned,
            }),
            (None, None) => None,
        }
    }

    /// True when the piece can be made right now by either route: create from scratch with the materials you hold,
    /// or upgrade a parent weapon you own.
    pub fn can_make_now(&self, kind: u8, id: u16) -> bool {
        let (create, upgrade) = self.recipes_for(kind, id);
        create.is_some_and(|r| self.save.can_craft(r))
            || upgrade.is_some_and(|u| {
                u.materials.iter().all(|m| self.save.item_count(m.id) >= m.count as u32)
                    && u.parents.iter().any(|&p| self.save.owns_equipment(kind, p))
            })
    }

    /// The search text for the current tab.
    pub fn active_search(&self) -> &str {
        if self.tab == Tab::Items {
            &self.inv.item_search
        } else {
            &self.craft.search
        }
    }

    pub(super) fn apply_search(&mut self) {
        if self.tab == Tab::Items {
            self.refresh_box();
        } else {
            self.refresh_pieces();
        }
    }

    /// The searchable text of every piece that has a recipe, in game order.
    pub(super) fn build_catalog(&self) -> Vec<Searchable> {
        self.game
            .find_equipment("")
            .into_iter()
            .filter(|(_, _, name)| !name.starts_with("DUMMY"))
            .filter_map(|(kind, id, name)| {
                let (create, upgrade) = self.recipes_for(kind, id);
                if create.is_none() && upgrade.is_none() {
                    return None;
                }
                let both = |n: &str| (n.to_owned(), n.to_lowercase());
                let skills = self
                    .game
                    .armor_stats(kind, id)
                    .map(|a| {
                        a.skills
                            .iter()
                            .filter_map(|&(sid, _)| self.game.skill_name(sid))
                            .map(both)
                            .collect()
                    })
                    .unwrap_or_default();
                let mut materials: Vec<(String, String)> = Vec::new();
                for m in create
                    .map(|r| &r.materials)
                    .into_iter()
                    .chain(upgrade.map(|u| &u.materials))
                    .flatten()
                {
                    if let Some(n) = self.game.item_name(m.id)
                        && !materials.iter().any(|(shown, _)| shown == n)
                    {
                        materials.push(both(n));
                    }
                }
                let stats = self.game.armor_stats(kind, id);
                let mut tags = Vec::new();
                if let Some(a) = stats {
                    match a.gender {
                        Some(Gender::Male) => tags.push("male"),
                        Some(Gender::Female) => tags.push("female"),
                        Some(Gender::Both) => tags.extend(["male", "female"]),
                        None => {}
                    }
                    match a.class {
                        Some(ArmorClass::Blademaster) => tags.push("blademaster"),
                        Some(ArmorClass::Gunner) => tags.push("gunner"),
                        Some(ArmorClass::Both) => tags.extend(["blademaster", "gunner"]),
                        None => {}
                    }
                }
                Some(Searchable {
                    rarity: self.game.equipment_rarity(kind, id),
                    tags,
                    kind,
                    id,
                    name: name.to_owned(),
                    name_lc: name.to_lowercase(),
                    kind_lc: self.game.equipment_kind_label(kind).unwrap_or("").to_lowercase(),
                    skills,
                    materials,
                })
            })
            .collect()
    }

    /// Recompute the crafting list for the current search text, filter, sort and inventory.
    pub(super) fn refresh_pieces(&mut self) {
        let words: Vec<String> = self.craft.search.split_whitespace().map(str::to_lowercase).collect();
        let mut scored: Vec<(u32, Piece)> = Vec::new();
        for entry in &self.craft.catalog {
            let Some((score, reason)) = entry.matches(&words) else { continue };
            let (kind, id) = (entry.kind, entry.id);
            let piece = Piece {
                kind,
                id,
                name: entry.name.clone(),
                craftable: self.can_make_now(kind, id),
                owned: self.save.owns_equipment(kind, id),
                offered: self.at_blacksmith(kind, id),
                reason,
            };
            let unpriced = piece.owned && self.cost(kind, id, Route::Create).is_none() && self.cost(kind, id, Route::Upgrade).is_none();
            if (!self.craft.craftable_only || piece.craftable)
                && (!self.craft.hide_owned || !piece.owned)
                && (!self.craft.unpriced_only || unpriced)
                && (!self.craft.blacksmith_only || piece.offered)
            {
                scored.push((score, piece));
            }
        }
        sort_pieces(&mut scored, self.craft.sort, words.len());
        self.craft.pieces = scored.into_iter().map(|(_, p)| p).collect();
        let sel = self
            .craft
            .state
            .selected()
            .unwrap_or(0)
            .min(self.craft.pieces.len().saturating_sub(1));
        self.craft.state.select(Some(sel));
    }

    /// The sort label to show: with a search, the default order is "best match".
    pub fn sort_label(&self) -> &'static str {
        if self.craft.sort == PieceSort::GameOrder && !self.craft.search.trim().is_empty() {
            "best match"
        } else {
            self.craft.sort.label()
        }
    }

    /// The forging cost of a piece by a route and what it came from. A price seen in play wins over the game files,
    /// which in turn win over notes and learned costs.
    pub fn cost(&self, kind: u8, id: u16, route: Route) -> Option<(u32, Source)> {
        let seen = self.costs.ledger.get(kind, id, route).filter(|e| e.source == Source::Seen);
        if let Some(e) = seen {
            return Some((e.cost, e.source));
        }
        let from_game = match route {
            Route::Create => self
                .game
                .armor_cost(kind, id)
                .or_else(|| self.game.weapon_cost(kind, id, mh3u_core::weapons::Via::Create)),
            Route::Upgrade => self.game.weapon_cost(kind, id, mh3u_core::weapons::Via::Upgrade),
        };
        if let Some(cost) = from_game {
            return Some((cost, Source::Game));
        }
        self.costs.ledger.get(kind, id, route).map(|e| (e.cost, e.source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(kind: u8, id: u16, name: &str, craftable: bool, owned: bool) -> Piece {
        Piece {
            kind,
            id,
            name: name.to_owned(),
            craftable,
            owned,
            offered: false,
            reason: None,
        }
    }

    fn names(pieces: &[(u32, Piece)]) -> Vec<&str> {
        pieces.iter().map(|(_, p)| p.name.as_str()).collect()
    }

    /// The case from the app: a search for "attack" matched Jaggi pieces of several kinds.
    fn jaggi() -> Vec<(u32, Piece)> {
        vec![
            (3000, piece(1, 10, "Jaggi Mail", false, false)),
            (1000, piece(5, 11, "Jaggi Cap", false, false)),
            (2000, piece(5, 12, "Jaggi Helm", false, false)),
            (2500, piece(2, 13, "Jaggi Vambraces", false, false)),
            (500, piece(7, 14, "Iron Sword", false, false)),
        ]
    }

    #[test]
    fn best_match_keeps_like_pieces_together() {
        let mut p = jaggi();
        sort_pieces(&mut p, PieceSort::GameOrder, 1);
        // strong matches (score 2000 or more) first, in head, body, arms order; then the weak ones (1000 and 500)
        assert_eq!(
            names(&p),
            ["Jaggi Helm", "Jaggi Mail", "Jaggi Vambraces", "Jaggi Cap", "Iron Sword"]
        );
    }

    #[test]
    fn game_order_groups_head_first_then_by_id() {
        let mut p = jaggi();
        sort_pieces(&mut p, PieceSort::GameOrder, 0);
        assert_eq!(
            names(&p),
            ["Jaggi Cap", "Jaggi Helm", "Jaggi Mail", "Jaggi Vambraces", "Iron Sword"]
        );
    }

    #[test]
    fn name_sort_is_alphabetical_within_each_group() {
        let mut p = jaggi();
        sort_pieces(&mut p, PieceSort::Name, 1);
        assert_eq!(
            names(&p),
            ["Jaggi Cap", "Jaggi Helm", "Jaggi Mail", "Jaggi Vambraces", "Iron Sword"]
        );
    }

    #[test]
    fn flag_sorts_split_first_then_group() {
        let mut p = jaggi();
        p[0].1.owned = true; // Jaggi Mail
        p[4].1.owned = true; // Iron Sword
        sort_pieces(&mut p, PieceSort::OwnedFirst, 0);
        assert_eq!(
            names(&p),
            ["Jaggi Mail", "Iron Sword", "Jaggi Cap", "Jaggi Helm", "Jaggi Vambraces"]
        );
        p.iter_mut().for_each(|(_, q)| q.craftable = q.name == "Jaggi Helm");
        sort_pieces(&mut p, PieceSort::CraftableFirst, 0);
        assert_eq!(names(&p)[0], "Jaggi Helm");
    }

    #[test]
    fn kinds_group_in_head_body_arms_waist_legs_talisman_weapon_order() {
        let ranks: Vec<u8> = [5, 1, 2, 3, 4, 6, 7, 19].iter().map(|&k| kind_rank(k)).collect();
        assert!(ranks.windows(2).all(|w| w[0] < w[1]), "{ranks:?}");
    }
}

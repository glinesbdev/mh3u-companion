use crate::{commands, search};
use anyhow::{Context, Result};
use mh3u_core::{
    armor::{ArmorClass, Gender},
    edit,
    gamedata::GameData,
    live::{LiveEvent, LiveReader},
    prices::{Ledger, Outcome, PriceEntry, PriceTracker, RecipeBook, Recorded, Route, Skip, SkipReason, Source},
    recipes::{Recipe, Upgrade},
    save::{ItemStack, Save},
};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    widgets::ListState,
};
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Items,
    Equipment,
    Crafting,
    Wishlist,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Items, Tab::Equipment, Tab::Crafting, Tab::Wishlist];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Items => "Items",
            Tab::Equipment => "Equipment",
            Tab::Crafting => "Crafting",
            Tab::Wishlist => "Wishlist",
        }
    }
}

/// A Cemu we started, and the background reader of its memory.
pub struct Live {
    pub reader: LiveReader,
    pub child: std::process::Child,
    /// True while the game's live save data is being read.
    pub connected: bool,
}

/// Ordering of the crafting list.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PieceSort {
    GameOrder,
    Name,
    CraftableFirst,
    OwnedFirst,
}

impl PieceSort {
    fn next(self) -> PieceSort {
        match self {
            PieceSort::GameOrder => PieceSort::Name,
            PieceSort::Name => PieceSort::CraftableFirst,
            PieceSort::CraftableFirst => PieceSort::OwnedFirst,
            PieceSort::OwnedFirst => PieceSort::GameOrder,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PieceSort::GameOrder => "game order",
            PieceSort::Name => "name",
            PieceSort::CraftableFirst => "craftable first",
            PieceSort::OwnedFirst => "owned first",
        }
    }
}

/// Ordering of the item box list.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BoxSort {
    BoxOrder,
    Name,
    Quantity,
}

impl BoxSort {
    fn next(self) -> BoxSort {
        match self {
            BoxSort::BoxOrder => BoxSort::Name,
            BoxSort::Name => BoxSort::Quantity,
            BoxSort::Quantity => BoxSort::BoxOrder,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BoxSort::BoxOrder => "box order",
            BoxSort::Name => "name",
            BoxSort::Quantity => "quantity",
        }
    }
}

/// Ordering of the equipment box list.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EquipSort {
    BoxOrder,
    Name,
    Rarity,
    Type,
    WornFirst,
}

impl EquipSort {
    fn next(self) -> EquipSort {
        match self {
            EquipSort::BoxOrder => EquipSort::Name,
            EquipSort::Name => EquipSort::Rarity,
            EquipSort::Rarity => EquipSort::Type,
            EquipSort::Type => EquipSort::WornFirst,
            EquipSort::WornFirst => EquipSort::BoxOrder,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EquipSort::BoxOrder => "box order",
            EquipSort::Name => "name",
            EquipSort::Rarity => "rarity",
            EquipSort::Type => "type",
            EquipSort::WornFirst => "worn first",
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
    /// When searching: why the piece matched, if not by name (e.g. "skill: Poison").
    pub reason: Option<String>,
}

/// Everything about a piece that the search looks at, lowercased once up front.
struct Searchable {
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

/// How long a change in zenny stays highlighted next to the total.
const ZENNY_SHOW: Duration = Duration::from_secs(15);

/// `1234567` -> `"1,234,567"`.
pub fn group_digits(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `+1,200` or `-300`.
pub fn signed_zenny(delta: i64) -> String {
    format!("{}{}", if delta < 0 { '-' } else { '+' }, group_digits(delta.unsigned_abs()))
}

/// The running zenny change after the total moved from `old` to `new`: changes close together are added up, and a change
/// that undoes the earlier ones clears it.
fn next_zenny_change(prev: Option<(i64, Instant)>, old: u32, new: u32, now: Instant) -> Option<(i64, Instant)> {
    if old == new {
        return prev;
    }
    let carried = prev.filter(|&(_, at)| now.duration_since(at) < ZENNY_SHOW).map_or(0, |(d, _)| d);
    let total = carried + (i64::from(new) - i64::from(old));
    (total != 0).then_some((total, now))
}

/// The game's recipes, for the price tracker.
struct GameBook<'a> {
    game: &'a GameData,
    /// Recipes learned from play, for pieces the game data lacks.
    create: &'a HashMap<(u8, u16), Recipe>,
    upgrade: &'a HashMap<(u8, u16), Upgrade>,
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
fn kind_rank(kind: u8) -> u8 {
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
const STRONG_PER_WORD: u32 = 2000;

/// Order the crafting list. Like pieces always sit together (see `kind_rank`); the chosen sort orders the pieces within
/// each group, except that "craftable first" and "owned first" split the list by that flag before grouping. While
/// searching (`words` > 0), the best match comes first within a group, otherwise game order; and in the default (best
/// match) order the strong matches (see `STRONG_PER_WORD`) come first, grouped, then the weak ones, grouped, so a pile of
/// loose matches in one slot can't bury the real hits in another.
fn sort_pieces(pieces: &mut [(u32, Piece)], sort: PieceSort, words: usize) {
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

/// Node of the upgrade tree for `parent_chain`.
struct TreeNode {
    /// Can be made from scratch, so it never needs a parent.
    can_create: bool,
    parents: Vec<u16>,
}

/// The ancestors to add along with a wishlisted piece, nearest first. Walks up while the piece can only be
/// obtained by upgrading and no parent is owned; stops at an ancestor that is owned, wishlisted, or can be
/// created from scratch.
fn parent_chain(start: u16, node: impl Fn(u16) -> TreeNode, owned: impl Fn(u16) -> bool, wished: impl Fn(u16) -> bool) -> Vec<u16> {
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

pub struct App {
    pub game: GameData,
    pub save: Save,
    pub save_path: PathBuf,
    modified: Option<SystemTime>,
    pub status: String,
    pub tab: Tab,
    pub box_state: ListState,
    pub equip_state: ListState,
    pub craft_state: ListState,
    pub search: String,
    /// Search text for the Items tab (the crafting search is `search`).
    pub item_search: String,
    pub searching: bool,
    pub craftable_only: bool,
    pub hide_owned: bool,
    /// Show only pieces you own that have no forging cost in the ledger yet.
    pub unpriced_only: bool,
    pub piece_sort: PieceSort,
    pub box_sort: BoxSort,
    pub equip_sort: EquipSort,
    /// The equipment box in display order: indexes into `save.equipment_box` (see `equip_sort`).
    pub equip_view: Vec<usize>,
    /// The upgrade tree popup, when open.
    pub tree: Option<TreeView>,
    /// The item box in display order (see `box_sort`).
    pub box_view: Vec<ItemStack>,
    /// The item pouch, filtered by the item search.
    pub pouch_view: Vec<ItemStack>,
    pub pieces: Vec<Piece>,
    catalog: Vec<Searchable>,
    /// Wishlisted pieces as (equipment kind, piece id), in the order they were added.
    pub wishlist: Vec<(u8, u16)>,
    /// Wishlisted pieces that were added automatically as a parent of another piece (see `remove_wish`).
    auto_parents: HashSet<(u8, u16)>,
    pub wish_state: ListState,
    wishlist_path: Option<PathBuf>,
    pub show_help: bool,
    /// Set when the TUI started Cemu and is reading its memory (`--live`).
    pub live: Option<Live>,
    /// Debug editing is on (`--debug-edit`): `:` opens the command line.
    pub edit_mode: bool,
    commanding: bool,
    pub command: String,
    /// The newest live save block, the base for edit commands.
    live_bytes: Option<Vec<u8>>,
    /// Forging costs seen in the game (see `mh3u_core::prices`) and the watcher that finds them.
    prices: Ledger,
    prices_path: Option<PathBuf>,
    /// Recipes learned from play (see `prices::Outcome::Learned`), for pieces the game data has no recipe for.
    learned_create: HashMap<(u8, u16), Recipe>,
    learned_upgrade: HashMap<(u8, u16), Upgrade>,
    tracker: PriceTracker,
    /// The recent change in zenny and when it happened (see `zenny_change`).
    zenny_change: Option<(i64, Instant)>,
    /// Waiting for y/n because quitting would end live updates from a Cemu that is still running.
    pub confirm_quit: bool,
    quit: bool,
}

impl App {
    pub fn new(game: GameData, save_path: PathBuf, wishlist_path: Option<PathBuf>, prices_path: Option<PathBuf>) -> Result<App> {
        let bytes = std::fs::read(&save_path).with_context(|| format!("reading {}", save_path.display()))?;
        let save = Save::parse(&bytes)?;
        let entries = wishlist_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| parse_wishlist(&t))
            .unwrap_or_default();
        let wishlist: Vec<(u8, u16)> = entries.iter().map(|&(k, i, _)| (k, i)).collect();
        let auto_parents = entries.iter().filter(|e| e.2).map(|&(k, i, _)| (k, i)).collect();
        let mut app = App {
            game,
            save,
            modified: modified(&save_path),
            save_path,
            status: String::new(),
            tab: Tab::Items,
            box_state: ListState::default().with_selected(Some(0)),
            equip_state: ListState::default().with_selected(Some(0)),
            craft_state: ListState::default().with_selected(Some(0)),
            search: String::new(),
            item_search: String::new(),
            searching: false,
            craftable_only: false,
            hide_owned: false,
            unpriced_only: false,
            piece_sort: PieceSort::GameOrder,
            box_sort: BoxSort::BoxOrder,
            equip_sort: EquipSort::BoxOrder,
            equip_view: Vec::new(),
            tree: None,
            box_view: Vec::new(),
            pouch_view: Vec::new(),
            pieces: Vec::new(),
            catalog: Vec::new(),
            wishlist,
            auto_parents,
            wish_state: ListState::default().with_selected(Some(0)),
            wishlist_path,
            show_help: false,
            live: None,
            prices: prices_path
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|t| Ledger::parse(&t))
                .unwrap_or_default(),
            prices_path,
            learned_create: HashMap::new(),
            learned_upgrade: HashMap::new(),
            tracker: PriceTracker::default(),
            zenny_change: None,
            edit_mode: false,
            commanding: false,
            command: String::new(),
            live_bytes: None,
            confirm_quit: false,
            quit: false,
        };
        app.rebuild_learned();
        app.catalog = app.build_catalog();
        app.refresh_box();
        app.refresh_equipment();
        app.refresh_pieces();
        Ok(app)
    }

    /// The create recipe and upgrade recipe of a piece, ignoring any that name unreleased placeholder items.
    fn recipes_for(&self, kind: u8, id: u16) -> (Option<&Recipe>, Option<&Upgrade>) {
        let placeholder = |m: &ItemStack| self.game.item_name(m.id).is_none_or(|n| n.starts_with("DUMMY"));
        let create = self
            .game
            .recipe(kind, id)
            .filter(|r| !r.materials.iter().any(placeholder))
            .or_else(|| self.learned_create.get(&(kind, id)));
        let upgrade = self
            .game
            .upgrade(kind, id)
            .filter(|u| !u.materials.iter().any(placeholder))
            .or_else(|| self.learned_upgrade.get(&(kind, id)));
        (create, upgrade)
    }

    /// The create recipe of a piece from the game data, or learned from play.
    pub fn create_recipe(&self, kind: u8, id: u16) -> Option<&Recipe> {
        self.game.recipe(kind, id).or_else(|| self.learned_create.get(&(kind, id)))
    }

    /// The upgrade recipe of a weapon from the game data, or learned from play.
    pub fn upgrade_recipe(&self, kind: u8, id: u16) -> Option<&Upgrade> {
        self.game.upgrade(kind, id).or_else(|| self.learned_upgrade.get(&(kind, id)))
    }

    /// Rebuild the recipe lookups from the ledger's learned entries.
    fn rebuild_learned(&mut self) {
        self.learned_create.clear();
        self.learned_upgrade.clear();
        for e in self.prices.learned() {
            let materials = e.materials.clone().unwrap_or_default();
            match e.route {
                Route::Create => {
                    self.learned_create.insert(
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
                    self.learned_upgrade.insert((e.kind, e.id), Upgrade { materials, parents });
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

    /// Items whose names match the item search, best match first (or unfiltered, in their given order).
    fn filter_items(&self, stacks: &[ItemStack]) -> Vec<(u32, ItemStack)> {
        let words: Vec<String> = self.item_search.split_whitespace().map(str::to_lowercase).collect();
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
    fn refresh_box(&mut self) {
        let searching = !self.item_search.trim().is_empty();
        let mut pouch = self.filter_items(&self.save.pouch);
        if searching {
            pouch.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
        }
        self.pouch_view = pouch.into_iter().map(|(_, s)| s).collect();

        let mut view = self.filter_items(&self.save.item_box);
        match self.box_sort {
            BoxSort::BoxOrder if searching => view.sort_by_key(|(score, _)| std::cmp::Reverse(*score)),
            BoxSort::BoxOrder => {}
            BoxSort::Name => view.sort_by_key(|(_, s)| self.game.item_name(s.id).unwrap_or("?").to_lowercase()),
            BoxSort::Quantity => view.sort_by_key(|(_, s)| std::cmp::Reverse(s.count)),
        }
        self.box_view = view.into_iter().map(|(_, s)| s).collect();
        let sel = self.box_state.selected().unwrap_or(0).min(self.box_view.len().saturating_sub(1));
        self.box_state.select(Some(sel));
    }

    /// Put the equipment box in display order, keeping the same item selected when it is still there.
    fn refresh_equipment(&mut self) {
        let kept = self.equip_state.selected().and_then(|i| self.equip_view.get(i)).copied();
        let boxed = &self.save.equipment_box;
        let name = |i: usize| self.game.equipment_name(boxed[i].kind, boxed[i].id).unwrap_or("?").to_lowercase();
        let rarity = |i: usize| self.game.equipment_rarity(boxed[i].kind, boxed[i].id).unwrap_or(0);
        let mut view: Vec<usize> = (0..boxed.len()).collect();
        match self.equip_sort {
            EquipSort::BoxOrder => {}
            EquipSort::Name => view.sort_by_key(|&i| (name(i), kind_rank(boxed[i].kind))),
            EquipSort::Rarity => view.sort_by_key(|&i| (std::cmp::Reverse(rarity(i)), kind_rank(boxed[i].kind), name(i))),
            EquipSort::Type => view.sort_by_key(|&i| (kind_rank(boxed[i].kind), name(i))),
            EquipSort::WornFirst => view.sort_by_key(|&i| (!self.save.is_worn(&boxed[i]), kind_rank(boxed[i].kind), name(i))),
        }
        let at = kept
            .and_then(|k| view.iter().position(|&i| i == k))
            .unwrap_or_else(|| self.equip_state.selected().unwrap_or(0).min(view.len().saturating_sub(1)));
        self.equip_view = view;
        self.equip_state.select(Some(at));
    }

    /// The equipment-box entry that is highlighted on the Equipment tab.
    pub fn selected_equipment(&self) -> Option<&mh3u_core::save::Equipment> {
        let i = *self.equip_view.get(self.equip_state.selected()?)?;
        self.save.equipment_box.get(i)
    }

    /// The weapon highlighted on the current tab, as (kind, id). `None` on tabs with no selection.
    pub fn highlighted_equipment(&self) -> Option<(u8, u16)> {
        match self.tab {
            Tab::Crafting => self.craft_state.selected().and_then(|i| self.pieces.get(i)).map(|p| (p.kind, p.id)),
            Tab::Equipment => self.selected_equipment().map(|e| (e.kind, e.id)),
            Tab::Wishlist => self.wish_state.selected().and_then(|i| self.wishlist.get(i)).copied(),
            Tab::Items => None,
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
            self.learned_upgrade
                .iter()
                .filter(|((k, child), u)| *k == kind && *child != id && u.parents.contains(&id))
                .map(|((_, child), _)| *child),
        );
        kids.sort_unstable();
        kids.dedup();
        kids
    }

    /// Open the upgrade tree for the highlighted weapon. Armor has no upgrade line in the game data, so nothing opens for it.
    fn open_tree(&mut self) {
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
        if self.box_sort == BoxSort::BoxOrder && !self.item_search.trim().is_empty() {
            "best match"
        } else {
            self.box_sort.label()
        }
    }

    /// The search text for the current tab.
    pub fn active_search(&self) -> &str {
        if self.tab == Tab::Items { &self.item_search } else { &self.search }
    }

    fn apply_search(&mut self) {
        if self.tab == Tab::Items {
            self.refresh_box();
        } else {
            self.refresh_pieces();
        }
    }

    /// The searchable text of every piece that has a recipe, in game order.
    fn build_catalog(&self) -> Vec<Searchable> {
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
    fn refresh_pieces(&mut self) {
        let words: Vec<String> = self.search.split_whitespace().map(str::to_lowercase).collect();
        let mut scored: Vec<(u32, Piece)> = Vec::new();
        for entry in &self.catalog {
            let Some((score, reason)) = entry.matches(&words) else { continue };
            let (kind, id) = (entry.kind, entry.id);
            let piece = Piece {
                kind,
                id,
                name: entry.name.clone(),
                craftable: self.can_make_now(kind, id),
                owned: self.save.owns_equipment(kind, id),
                reason,
            };
            let unpriced = piece.owned && self.cost(kind, id, Route::Create).is_none() && self.cost(kind, id, Route::Upgrade).is_none();
            if (!self.craftable_only || piece.craftable) && (!self.hide_owned || !piece.owned) && (!self.unpriced_only || unpriced) {
                scored.push((score, piece));
            }
        }
        sort_pieces(&mut scored, self.piece_sort, words.len());
        self.pieces = scored.into_iter().map(|(_, p)| p).collect();
        let sel = self.craft_state.selected().unwrap_or(0).min(self.pieces.len().saturating_sub(1));
        self.craft_state.select(Some(sel));
    }

    /// The sort label to show: with a search, the default order is "best match".
    pub fn sort_label(&self) -> &'static str {
        if self.piece_sort == PieceSort::GameOrder && !self.search.trim().is_empty() {
            "best match"
        } else {
            self.piece_sort.label()
        }
    }

    pub fn is_wished(&self, kind: u8, id: u16) -> bool {
        self.wishlist.contains(&(kind, id))
    }

    /// Add a piece to the wishlist, plus the parent weapons it needs: the upgrade chain leading to it, back to
    /// the first weapon you own or can make from scratch. Parents go in before the piece.
    fn add_wish_with_parents(&mut self, kind: u8, id: u16) {
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
    fn needed_parents(&self, kind: u8, id: u16) -> Vec<u16> {
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
    fn remove_wish(&mut self, kind: u8, id: u16) {
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

    fn toggle_wish(&mut self, kind: u8, id: u16) {
        if self.is_wished(kind, id) {
            self.remove_wish(kind, id);
        } else {
            self.add_wish_with_parents(kind, id);
            return;
        }
        self.after_wishlist_change();
    }

    fn after_wishlist_change(&mut self) {
        let sel = self.wish_state.selected().unwrap_or(0).min(self.wishlist.len().saturating_sub(1));
        self.wish_state.select(Some(sel));
        self.save_wishlist();
    }

    fn save_wishlist(&mut self) {
        let Some(path) = &self.wishlist_path else { return };
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(path, format_wishlist(&self.wishlist, &self.auto_parents)));
        if let Err(e) = result {
            self.status = format!("could not save wishlist: {e}");
        }
    }

    /// The zenny change to show next to the total, if one happened in the last few seconds.
    pub fn zenny_change(&self) -> Option<i64> {
        self.zenny_change
            .filter(|(_, at)| at.elapsed() < ZENNY_SHOW)
            .map(|(delta, _)| delta)
    }

    /// Make `save` the current data. Returns a message if the zenny changed.
    fn apply_save(&mut self, save: Save) -> Option<String> {
        let (old, new) = (self.save.zenny, save.zenny);
        self.zenny_change = next_zenny_change(self.zenny_change, old, new, Instant::now());
        self.save = save;
        self.refresh_box();
        self.refresh_equipment();
        self.refresh_pieces();
        (old != new).then(|| {
            format!(
                "zenny {} (now {})",
                signed_zenny(i64::from(new) - i64::from(old)),
                group_digits(u64::from(new))
            )
        })
    }

    /// Turn on the debug command line (`:`). `note` is shown in the status line.
    pub fn enable_edit(&mut self, note: String) {
        self.edit_mode = true;
        self.status = note;
    }

    /// What the wishlist still needs: (item id, total needed) for every wishlisted piece you don't own, and how many such
    /// pieces there are. The same totals as the Wishlist tab's shopping list.
    pub fn shopping_need(&self) -> (Vec<(u16, u32)>, usize) {
        self.shopping_need_with(false)
    }

    /// Like `shopping_need`; with `include_owned`, wishlisted pieces you already own count too.
    fn shopping_need_with(&self, include_owned: bool) -> (Vec<(u16, u32)>, usize) {
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

    /// Run a debug command: work out the patches, then ask the live reader to write them into the game.
    fn run_command(&mut self, text: &str) {
        let command = match commands::parse(text) {
            Ok(c) => c,
            Err(e) => return self.status = e,
        };
        let connected = self.live_connected();
        let (Some(live), Some(mut data)) = (&self.live, self.live_bytes.clone()) else {
            return self.status = "editing needs the game running through --live".into();
        };
        if !connected {
            return self.status = "editing needs a hunter loaded in the game".into();
        }
        let mut patches: Vec<edit::Patch> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        let mut push = |patch: edit::Patch, data: &mut Vec<u8>| {
            edit::apply(data, &patch);
            patches.push(patch);
        };
        let result: Result<(), String> = (|| {
            match command {
                commands::Command::Zenny(op) => {
                    let wanted = match op {
                        commands::ZennyOp::Set(n) => i64::from(n),
                        commands::ZennyOp::Add(d) => i64::from(self.save.zenny) + d,
                    };
                    let amount = wanted.clamp(0, i64::from(edit::MAX_ZENNY)) as u32;
                    push(edit::set_zenny(amount), &mut data);
                    notes.push(format!("zenny set to {}", group_digits(u64::from(amount))));
                }
                commands::Command::Give { item, count } => {
                    let found = commands::resolve_item(&self.game, &item).ok_or(format!("no item matches '{item}'"))?;
                    let have = edit::box_count(&data, found.id);
                    let target = count.map_or(edit::MAX_STACK, |n| have.saturating_add(n));
                    push(edit::set_box_item(&data, found.id, target).map_err(|e| e.to_string())?, &mut data);
                    notes.push(format!(
                        "{} in the box: {have} -> {}",
                        found.describe(),
                        target.min(edit::MAX_STACK)
                    ));
                }
                commands::Command::Set { item, count } => {
                    let found = commands::resolve_item(&self.game, &item).ok_or(format!("no item matches '{item}'"))?;
                    push(edit::set_box_item(&data, found.id, count).map_err(|e| e.to_string())?, &mut data);
                    notes.push(format!("{} in the box set to {}", found.describe(), count.min(edit::MAX_STACK)));
                }
                commands::Command::Stock { include_owned } => {
                    let (need, _) = self.shopping_need_with(include_owned);
                    for (id, wanted) in need {
                        let have = self.save.item_count(id);
                        if have >= wanted {
                            continue;
                        }
                        let in_box = edit::box_count(&data, id);
                        let target = in_box.saturating_add(u16::try_from(wanted - have).unwrap_or(u16::MAX));
                        push(edit::set_box_item(&data, id, target).map_err(|e| e.to_string())?, &mut data);
                        notes.push(self.game.item_name(id).unwrap_or("?").to_string());
                    }
                    if notes.is_empty() {
                        notes.push("the wishlist is already covered".into());
                    } else {
                        notes = vec![format!("stocked the box: {}", notes.join(", "))];
                    }
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                for patch in patches {
                    live.reader.write(patch);
                }
                // what we just wrote must not be mistaken for something the player did
                if let Ok(edited) = Save::parse(&data) {
                    self.tracker.rebase(&edited);
                }
                self.status = notes.join("; ");
            }
            Err(e) => self.status = e,
        }
    }

    pub fn set_live(&mut self, live: Live) {
        self.live = Some(live);
        self.status = "live: waiting for a hunter to be loaded in the game".into();
    }

    /// The forging cost of a piece by a route and what it came from. A price seen in play wins over the game files,
    /// which in turn win over notes and learned costs.
    pub fn cost(&self, kind: u8, id: u16, route: Route) -> Option<(u32, Source)> {
        let seen = self.prices.get(kind, id, route).filter(|e| e.source == Source::Seen);
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
        self.prices.get(kind, id, route).map(|e| (e.cost, e.source))
    }

    /// Let the price tracker judge what has settled, and keep any cost it is sure about.
    fn tick_prices(&mut self) {
        let book = GameBook {
            game: &self.game,
            create: &self.learned_create,
            upgrade: &self.learned_upgrade,
        };
        let outcome = self.tracker.finish(Instant::now(), &book);
        let when = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let (kind, id, route, cost, source, materials, parent) = match outcome {
            None => return,
            Some(Outcome::Skipped(skip)) => {
                let message = self.skip_message(&skip);
                self.log_tracker(&format!("skipped\t{message}\t{}", self.skip_detail(&skip)));
                return self.status = message;
            }
            Some(Outcome::Seen(o)) => (o.kind, o.id, o.route, o.cost, Source::Seen, None, None),
            Some(Outcome::Learned(l)) => (l.kind, l.id, l.route, l.cost, Source::Learned, Some(l.materials), l.parent),
        };
        let name = self.game.equipment_name(kind, id).unwrap_or("?").to_string();
        let learned = materials.is_some();
        let recipe_text = materials.as_ref().map(|m| {
            let items: Vec<String> = m
                .iter()
                .map(|s| format!("{} x{}", self.game.item_name(s.id).unwrap_or("?"), s.count))
                .collect();
            if items.is_empty() {
                "no materials".to_string()
            } else {
                items.join(", ")
            }
        });
        let entry = PriceEntry {
            kind,
            id,
            route,
            cost,
            source,
            when,
            name: name.clone(),
            materials,
            parent,
        };
        let (z, how) = (group_digits(u64::from(cost)), route.label());
        let message = match (self.prices.record(entry), recipe_text) {
            (_, Some(recipe)) => format!("learned {name}: {z} z to {how}, recipe {recipe}"),
            (Recorded::New, None) => format!("price noted: {name} costs {z} z to {how}"),
            (Recorded::Unchanged, None) => format!("{name} costs {z} z to {how} (as noted before)"),
            (Recorded::Changed(old), None) => format!("price changed: {name} now {z} z to {how} (was {} z)", group_digits(u64::from(old))),
        };
        self.log_tracker(&format!("recorded\t{message}"));
        self.status = message;
        if learned {
            // the new recipe makes the piece appear in the Crafting list and the wishlist
            self.rebuild_learned();
            self.catalog = self.build_catalog();
            self.refresh_pieces();
        }
        if let Some(path) = &self.prices_path {
            let saved = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(path, self.prices.format()));
            if let Err(e) = saved {
                self.status = format!("could not save prices: {e}");
            }
        }
    }

    /// Append a line to `tracker.log` next to the price ledger: when a crafting was recorded or skipped, and why. It is
    /// what to look at when a price you expected is missing.
    fn log_tracker(&self, line: &str) {
        use std::io::Write;
        let Some(path) = self.prices_path.as_ref().map(|p| p.with_file_name("tracker.log")) else {
            return;
        };
        let when = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{when}\t{line}");
        }
    }

    /// What changed in a skipped transaction: the wallet and every item (pouch and box together).
    fn skip_detail(&self, skip: &Skip) -> String {
        let items: Vec<String> = skip
            .items
            .iter()
            .map(|(id, d)| format!("{} {d:+}", self.game.item_name(*id).unwrap_or("?")))
            .collect();
        format!(
            "wallet {:+}\titems {}",
            skip.zenny,
            if items.is_empty() { "none".to_string() } else { items.join(", ") }
        )
    }

    /// Explain a piece whose price was not recorded.
    fn skip_message(&self, skip: &Skip) -> String {
        let names: Vec<&str> = skip
            .pieces
            .iter()
            .map(|&(k, i)| self.game.equipment_name(k, i).unwrap_or("?"))
            .collect();
        let why = match skip.why {
            SkipReason::NoCost => "the wallet did not go down",
            SkipReason::SeveralPieces => "several pieces appeared at once, craft one at a time",
            SkipReason::UnknownRecipe => "the app has no recipe for it",
            SkipReason::DifferentMaterials => "the items used don't match its recipe (did something else change at the same time?)",
        };
        format!("price not recorded for {}: {why}", names.join(", "))
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

    /// True while the debug command line is open.
    pub fn is_commanding(&self) -> bool {
        self.commanding
    }

    pub fn live_connected(&self) -> bool {
        self.live.as_ref().is_some_and(|l| l.connected)
    }

    /// Apply whatever the live reader has found since the last call.
    fn poll_live(&mut self) {
        let Some(live) = &mut self.live else { return };
        let (mut newest, mut status) = (None, None);
        let was_connected = live.connected;
        while let Ok(event) = live.reader.events.try_recv() {
            match event {
                LiveEvent::Connected(_) => {
                    live.connected = true;
                    status = Some("live: connected to the game".to_string());
                }
                LiveEvent::Save(bytes) => newest = Some(bytes),
                LiveEvent::WriteFailed(why) => status = Some(format!("edit failed: {why}")),
                LiveEvent::Lost => {
                    live.connected = false;
                    status = Some("live: hunter unloaded, showing the last save file".to_string());
                }
            }
        }
        if live.child.try_wait().ok().flatten().is_some() {
            live.connected = false;
            status = Some("Cemu has exited; live updates stopped".to_string());
        }
        // The live data is gone (hunter unloaded or Cemu closed), and anything not saved with it. Go back to the save file.
        let dropped = was_connected && !live.connected;
        if dropped {
            self.tracker.reset();
        }
        if let Some(bytes) = newest {
            let parsed = Save::parse(&bytes);
            self.live_bytes = Some(bytes);
            match parsed {
                Ok(save) => {
                    self.tracker.observe(&save, Instant::now());
                    if let Some(message) = self.apply_save(save) {
                        status = Some(message);
                    }
                }
                Err(e) => status = Some(format!("live data not understood: {e:#}")),
            }
        }
        if let Some(s) = status {
            self.status = s;
        }
        if dropped {
            self.modified = None; // makes `reload_if_changed` read the file now
        }
    }

    /// Reload the save file if it changed on disk (Cemu rewrites it each time the game saves).
    fn reload_if_changed(&mut self) {
        if self.live_connected() {
            return; // the live data is newer than the file
        }
        let now = modified(&self.save_path);
        if now == self.modified {
            return;
        }
        match std::fs::read(&self.save_path)
            .map_err(anyhow::Error::from)
            .and_then(|b| Save::parse(&b))
        {
            Ok(save) => {
                let forced = self.modified.is_none(); // set when the live data went away
                self.modified = now;
                self.status = match (self.apply_save(save), forced) {
                    (Some(zenny), true) => format!("hunter unloaded, back to the save file: {zenny}"),
                    (None, true) => "hunter unloaded, back to the save file".into(),
                    (Some(zenny), false) => zenny,
                    (None, false) => "save reloaded".into(),
                };
            }
            Err(e) => self.status = format!("reload failed: {e:#}"),
        }
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.quit {
            terminal.draw(|f| crate::ui::draw(f, self))?;
            if event::poll(Duration::from_millis(250))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.on_key(key.code, key.modifiers);
            }
            self.poll_live();
            self.tick_prices();
            self.reload_if_changed();
        }
        Ok(())
    }

    fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.confirm_quit {
            if matches!(code, KeyCode::Char('y' | 'Y')) {
                self.quit = true;
            }
            self.confirm_quit = false;
            return;
        }
        if let Some(view) = &mut self.tree {
            match code {
                KeyCode::Esc | KeyCode::Char('t' | 'q') => self.tree = None,
                KeyCode::Down | KeyCode::Char('j') => view.scroll = view.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => view.scroll = view.scroll.saturating_sub(1),
                KeyCode::PageDown => view.scroll = view.scroll.saturating_add(10),
                KeyCode::PageUp => view.scroll = view.scroll.saturating_sub(10),
                KeyCode::Home | KeyCode::Char('g') => view.scroll = 0,
                KeyCode::End | KeyCode::Char('G') => view.scroll = u16::MAX, // the drawing code clamps it
                _ => {}
            }
            return;
        }
        if self.commanding {
            match code {
                KeyCode::Esc => self.commanding = false,
                KeyCode::Enter => {
                    self.commanding = false;
                    let text = std::mem::take(&mut self.command);
                    self.run_command(&text);
                }
                KeyCode::Backspace => {
                    self.command.pop();
                }
                KeyCode::Char(c) => self.command.push(c),
                _ => {}
            }
            return;
        }
        if self.searching {
            let text = if self.tab == Tab::Items {
                &mut self.item_search
            } else {
                &mut self.search
            };
            match code {
                KeyCode::Esc => {
                    self.searching = false;
                    text.clear();
                }
                KeyCode::Enter => self.searching = false,
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Char(c) => text.push(c),
                _ => {}
            }
            self.apply_search();
            return;
        }
        if self.show_help {
            self.show_help = false; // any key closes the help overlay
            return;
        }
        match code {
            KeyCode::Char('q') => {
                // Closing the TUI ends live updates from a Cemu that is still running, so ask first.
                let cemu_running = self.live.as_mut().is_some_and(|l| l.child.try_wait().ok().flatten().is_none());
                if cemu_running {
                    self.confirm_quit = true;
                } else {
                    self.quit = true;
                }
            }
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('t') if self.tab != Tab::Items => self.open_tree(),
            KeyCode::Char(':') if self.edit_mode => {
                self.commanding = true;
                self.command.clear();
            }
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => self.switch_tab(1),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => self.switch_tab(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::Home | KeyCode::Char('g') => self.move_selection(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.move_selection(isize::MAX),
            KeyCode::Char('/') if matches!(self.tab, Tab::Items | Tab::Crafting) => self.searching = true,
            KeyCode::Char('c') if self.tab == Tab::Crafting => {
                self.craftable_only = !self.craftable_only;
                self.refresh_pieces();
            }
            KeyCode::Char('o') if self.tab == Tab::Crafting => {
                self.hide_owned = !self.hide_owned;
                self.refresh_pieces();
            }
            KeyCode::Char('u') if self.tab == Tab::Crafting => {
                self.unpriced_only = !self.unpriced_only;
                self.refresh_pieces();
            }
            KeyCode::Char('s') => match self.tab {
                Tab::Items => {
                    self.box_sort = self.box_sort.next();
                    self.refresh_box();
                }
                Tab::Crafting => {
                    self.piece_sort = self.piece_sort.next();
                    self.refresh_pieces();
                }
                Tab::Equipment => {
                    self.equip_sort = self.equip_sort.next();
                    self.refresh_equipment();
                }
                _ => {}
            },
            KeyCode::Char('w') if self.tab == Tab::Crafting => {
                if let Some(p) = self.craft_state.selected().and_then(|i| self.pieces.get(i)) {
                    let (kind, id) = (p.kind, p.id);
                    self.toggle_wish(kind, id);
                }
            }
            // On the searchable tabs, x clears the search; with nothing searched it does nothing.
            KeyCode::Char('x') if matches!(self.tab, Tab::Items | Tab::Crafting) => {
                if !self.active_search().is_empty() {
                    if self.tab == Tab::Items {
                        self.item_search.clear();
                    } else {
                        self.search.clear();
                    }
                    self.apply_search();
                }
            }
            KeyCode::Char('w' | 'x') | KeyCode::Delete if self.tab == Tab::Wishlist => {
                if let Some(&(kind, id)) = self.wish_state.selected().and_then(|i| self.wishlist.get(i)) {
                    self.toggle_wish(kind, id);
                }
            }
            KeyCode::Esc => {
                if self.tab == Tab::Items {
                    self.item_search.clear();
                } else {
                    self.search.clear();
                }
                self.apply_search();
            }
            _ => {}
        }
    }

    fn switch_tab(&mut self, step: isize) {
        let i = Tab::ALL.iter().position(|&t| t == self.tab).unwrap_or(0) as isize;
        self.tab = Tab::ALL[(i + step).rem_euclid(Tab::ALL.len() as isize) as usize];
    }

    fn move_selection(&mut self, step: isize) {
        let (state, len) = match self.tab {
            Tab::Items => (&mut self.box_state, self.box_view.len()),
            Tab::Equipment => (&mut self.equip_state, self.equip_view.len()),
            Tab::Crafting => (&mut self.craft_state, self.pieces.len()),
            Tab::Wishlist => (&mut self.wish_state, self.wishlist.len()),
        };
        state.select(Some(stepped(state.selected(), step, len)));
    }
}

/// The row to select after moving `step` rows from `current` in a list of `len` rows, staying inside the list.
/// `isize::MIN` and `isize::MAX` therefore go to the top and the bottom.
fn stepped(current: Option<usize>, step: isize, len: usize) -> usize {
    let next = (current.unwrap_or(0) as isize).saturating_add(step);
    next.clamp(0, len.saturating_sub(1) as isize) as usize
}

fn modified(path: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// Parse the wishlist file: `kind id` per line, with an optional trailing `auto` for pieces added as a parent of another.
/// Lines that don't parse are ignored.
fn parse_wishlist(text: &str) -> Vec<(u8, u16, bool)> {
    text.lines()
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let (kind, id) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?);
            Some((kind, id, parts.next() == Some("auto")))
        })
        .collect()
}

fn format_wishlist(wishlist: &[(u8, u16)], auto: &HashSet<(u8, u16)>) -> String {
    wishlist
        .iter()
        .map(|&(kind, id)| format!("{kind} {id}{}\n", if auto.contains(&(kind, id)) { " auto" } else { "" }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    #[test]
    fn moving_stays_inside_the_list_and_home_end_reach_the_edges() {
        assert_eq!(stepped(Some(5), 1, 10), 6);
        assert_eq!(stepped(Some(5), -10, 10), 0);
        assert_eq!(stepped(Some(5), 10, 10), 9);
        assert_eq!(stepped(Some(5), isize::MIN, 10), 0, "Home");
        assert_eq!(stepped(Some(5), isize::MAX, 10), 9, "End");
        assert_eq!(stepped(None, isize::MAX, 10), 9);
        assert_eq!(stepped(None, isize::MAX, 0), 0, "an empty list has nothing to reach");
    }

    #[test]
    fn digits_are_grouped() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(1500), "1,500");
        assert_eq!(group_digits(1_234_567), "1,234,567");
        assert_eq!(signed_zenny(1200), "+1,200");
        assert_eq!(signed_zenny(-300), "-300");
    }

    #[test]
    fn zenny_changes_add_up_while_recent_and_reset_afterwards() {
        let t0 = Instant::now();
        let step = Duration::from_secs;
        let first = next_zenny_change(None, 1500, 2700, t0);
        assert_eq!(first.map(|c| c.0), Some(1200));
        // a second change a few seconds later adds to the first
        let second = next_zenny_change(first, 2700, 2400, t0 + step(5));
        assert_eq!(second.map(|c| c.0), Some(900));
        // no change leaves it alone
        assert_eq!(next_zenny_change(second, 2400, 2400, t0 + step(6)), second);
        // after the window it starts over
        assert_eq!(next_zenny_change(second, 2400, 2500, t0 + step(60)).map(|c| c.0), Some(100));
        // undoing the earlier change clears it
        assert_eq!(next_zenny_change(first, 2700, 1500, t0 + step(2)), None);
    }

    fn piece(kind: u8, id: u16, name: &str, craftable: bool, owned: bool) -> Piece {
        Piece {
            kind,
            id,
            name: name.to_owned(),
            craftable,
            owned,
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

//! What crafting costs, learned from watching the game.
//!
//! The forging prices are not (yet) known from the game files, but the live save data shows them: when a piece is crafted,
//! the wallet drops by its price. The [`PriceTracker`] notices that, but only when it is sure what happened, and the
//! [`Ledger`] keeps the results in a small text file so they accumulate over time. The file also lists exact (piece, cost)
//! pairs, which is what is needed to search the game's data for where the prices are stored.

use crate::save::{ItemStack, Save};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

/// How a piece was obtained. Upgrading costs a different amount to creating from scratch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Create,
    Upgrade,
}

impl Route {
    pub fn label(self) -> &'static str {
        match self {
            Route::Create => "create",
            Route::Upgrade => "upgrade",
        }
    }

    fn parse(s: &str) -> Option<Route> {
        match s {
            "create" => Some(Route::Create),
            "upgrade" => Some(Route::Upgrade),
            _ => None,
        }
    }
}

/// Where a ledger entry came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Seen by the tracker while crafting in the live game.
    Seen,
    /// Typed in from the game's own screens (the forge menu).
    Notes,
    /// Seen while crafting a piece the game data has no recipe for; the entry then also holds the recipe.
    Learned,
    /// Worked out from the game's own weapon price tables; never stored in the ledger.
    Game,
}

impl Source {
    fn label(self) -> &'static str {
        match self {
            Source::Seen => "seen",
            Source::Notes => "notes",
            Source::Learned => "learned",
            Source::Game => "game",
        }
    }

    fn parse(s: &str) -> Option<Source> {
        match s {
            "seen" => Some(Source::Seen),
            "notes" => Some(Source::Notes),
            "learned" => Some(Source::Learned),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceEntry {
    /// Equipment kind and piece id, as everywhere else.
    pub kind: u8,
    pub id: u16,
    pub route: Route,
    pub cost: u32,
    pub source: Source,
    /// Unix time it was recorded.
    pub when: u64,
    /// Only for people reading the file; not used when loading.
    pub name: String,
    /// The recipe, for entries learned from play (`Source::Learned`); for the game's own recipes this is `None`.
    pub materials: Option<Vec<ItemStack>>,
    /// For a learned upgrade: the weapon that was used up.
    pub parent: Option<u16>,
}

/// What `Ledger::record` did.
#[derive(Debug, PartialEq, Eq)]
pub enum Recorded {
    New,
    Unchanged,
    /// The piece already had a different cost, now replaced.
    Changed(u32),
}

/// The saved costs, one per (piece, route).
#[derive(Debug, Default)]
pub struct Ledger {
    entries: Vec<PriceEntry>,
}

impl Ledger {
    /// Read the file's text. Lines that don't parse, and `#` comments, are ignored.
    ///
    /// Format, tab separated: `kind id route cost source when name`.
    pub fn parse(text: &str) -> Ledger {
        let mut ledger = Ledger::default();
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            let f: Vec<&str> = line.split('\t').collect();
            let entry = (|| {
                Some(PriceEntry {
                    kind: f.first()?.parse().ok()?,
                    id: f.get(1)?.parse().ok()?,
                    route: Route::parse(f.get(2)?)?,
                    cost: f.get(3)?.parse().ok()?,
                    source: Source::parse(f.get(4)?)?,
                    when: f.get(5)?.parse().ok()?,
                    name: f.get(6).unwrap_or(&"").to_string(),
                    materials: f.get(7).and_then(|m| parse_materials(m)),
                    parent: f.get(8).and_then(|p| p.parse().ok()),
                })
            })();
            if let Some(e) = entry {
                ledger.record(e);
            }
        }
        ledger
    }

    pub fn format(&self) -> String {
        let mut entries: Vec<&PriceEntry> = self.entries.iter().collect();
        entries.sort_by_key(|e| (e.kind, e.id, e.route.label()));
        let mut out = String::from("# kind\tid\troute\tcost\tsource\twhen\tname\t[recipe item:count,...\tparent]\n");
        for e in entries {
            out += &format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                e.kind,
                e.id,
                e.route.label(),
                e.cost,
                e.source.label(),
                e.when,
                e.name
            );
            if e.materials.is_some() || e.parent.is_some() {
                let materials = e.materials.as_deref().map(format_materials).unwrap_or_default();
                let parent = e.parent.map(|p| p.to_string()).unwrap_or_default();
                out += &format!("\t{materials}\t{parent}");
            }
            out.push('\n');
        }
        out
    }

    pub fn get(&self, kind: u8, id: u16, route: Route) -> Option<&PriceEntry> {
        self.entries.iter().find(|e| e.kind == kind && e.id == id && e.route == route)
    }

    pub fn entries(&self) -> &[PriceEntry] {
        &self.entries
    }

    /// The entries that carry a recipe learned from play.
    pub fn learned(&self) -> impl Iterator<Item = &PriceEntry> {
        self.entries.iter().filter(|e| e.materials.is_some())
    }

    /// Add or replace the cost of a piece.
    pub fn record(&mut self, entry: PriceEntry) -> Recorded {
        match self
            .entries
            .iter_mut()
            .find(|e| e.kind == entry.kind && e.id == entry.id && e.route == entry.route)
        {
            Some(existing) if existing.cost == entry.cost => Recorded::Unchanged,
            Some(existing) => {
                let old = existing.cost;
                *existing = entry;
                Recorded::Changed(old)
            }
            None => {
                self.entries.push(entry);
                Recorded::New
            }
        }
    }
}

/// `214:3,216:1`; an empty recipe is the empty string.
fn format_materials(materials: &[ItemStack]) -> String {
    materials
        .iter()
        .map(|m| format!("{}:{}", m.id, m.count))
        .collect::<Vec<_>>()
        .join(",")
}

/// The inverse of `format_materials`. An empty string is an empty recipe; anything malformed is `None`.
fn parse_materials(text: &str) -> Option<Vec<ItemStack>> {
    if text.is_empty() {
        return Some(Vec::new());
    }
    text.split(',')
        .map(|part| {
            let (id, count) = part.split_once(':')?;
            Some(ItemStack {
                id: id.parse().ok()?,
                count: count.parse().ok()?,
            })
        })
        .collect()
}

/// The recipes the tracker checks a crafting against.
pub trait RecipeBook {
    fn create_materials(&self, kind: u8, id: u16) -> Option<Vec<ItemStack>>;
    /// Materials and the ids of the parent weapons.
    fn upgrade(&self, kind: u8, id: u16) -> Option<(Vec<ItemStack>, Vec<u16>)>;
}

/// A crafting the tracker is sure about.
#[derive(Debug, PartialEq, Eq)]
pub struct Observation {
    pub kind: u8,
    pub id: u16,
    pub route: Route,
    pub cost: u32,
}

/// Changes closer together than this belong to one transaction.
const QUIET: Duration = Duration::from_secs(1);
/// How long a transaction may stay open: one that keeps changing, or that looks like half a crafting still waiting for
/// its other half (see `Pending::unfinished`), is judged after this long anyway.
const HOLD: Duration = Duration::from_secs(10);

#[derive(Default)]
struct Snapshot {
    zenny: u32,
    pieces: HashMap<(u8, u16), i32>,
    /// Pouch and box together.
    items: HashMap<u16, i64>,
}

impl Snapshot {
    fn of(save: &Save) -> Snapshot {
        let mut s = Snapshot {
            zenny: save.zenny,
            ..Snapshot::default()
        };
        for e in &save.equipment_box {
            *s.pieces.entry((e.kind, e.id)).or_insert(0) += 1;
        }
        for st in save.pouch.iter().chain(&save.item_box) {
            *s.items.entry(st.id).or_insert(0) += i64::from(st.count);
        }
        s
    }
}

/// Everything that changed since the transaction began.
#[derive(Default)]
struct Pending {
    started: Option<Instant>,
    last: Option<Instant>,
    zenny: i64,
    pieces: HashMap<(u8, u16), i32>,
    items: HashMap<u16, i64>,
}

impl Pending {
    /// Looks like one half of a crafting whose other half has not arrived yet: the wallet went down and only items were
    /// used up (the piece comes after the animation), or a piece appeared before the payment and the items were taken.
    /// Buying or selling items, or a fee on its own, is not held, so it can't be mixed into a later crafting.
    fn unfinished(&self) -> bool {
        let added = self.pieces.values().any(|d| *d > 0);
        let consumed_only = !self.items.is_empty() && self.items.values().all(|d| *d < 0);
        match (added, self.zenny < 0) {
            (false, true) => consumed_only,
            (true, false) => true,
            (true, true) => self.items.is_empty(),
            (false, false) => false,
        }
    }
}

fn add_deltas<K: std::hash::Hash + Eq + Copy, V: Copy + Default + std::ops::Add<Output = V> + std::ops::Sub<Output = V> + PartialEq>(
    into: &mut HashMap<K, V>,
    before: &HashMap<K, V>,
    after: &HashMap<K, V>,
) -> bool {
    let mut changed = false;
    let keys: std::collections::HashSet<K> = before.keys().chain(after.keys()).copied().collect();
    for key in keys.iter() {
        let delta = after.get(key).copied().unwrap_or_default() - before.get(key).copied().unwrap_or_default();
        if delta != V::default() {
            let slot = into.entry(*key).or_default();
            *slot = *slot + delta;
            changed = true;
        }
    }
    into.retain(|_, v| *v != V::default());
    changed
}

/// Watches successive live saves and works out what a crafting cost.
#[derive(Default)]
pub struct PriceTracker {
    prev: Option<Snapshot>,
    pending: Pending,
}

impl PriceTracker {
    /// Feed it every new live save, in order.
    pub fn observe(&mut self, save: &Save, now: Instant) {
        let snap = Snapshot::of(save);
        if let Some(prev) = &self.prev {
            let zenny = i64::from(snap.zenny) - i64::from(prev.zenny);
            let pieces = add_deltas(&mut self.pending.pieces, &prev.pieces, &snap.pieces);
            let items = add_deltas(&mut self.pending.items, &prev.items, &snap.items);
            if zenny != 0 || pieces || items {
                self.pending.zenny += zenny;
                self.pending.started.get_or_insert(now);
                self.pending.last = Some(now);
            }
        }
        self.prev = Some(snap);
    }

    /// Tell it what the game will look like once something other than the player has changed it, e.g. the debug editor's
    /// writes. Those changes are then not counted as the player's, and a crafting right afterwards is still seen. (If the
    /// game is observed in its old state first, the old-to-new step cancels the new-to-old one, so nothing is left over.)
    pub fn rebase(&mut self, expected: &Save) {
        self.prev = Some(Snapshot::of(expected));
        self.pending = Pending::default();
    }

    /// Forget everything, e.g. when the hunter was unloaded.
    pub fn reset(&mut self) {
        *self = PriceTracker::default();
    }

    /// Call regularly. Once the changes have settled, says what came of them: a crafting whose cost is certain, or a piece
    /// that appeared but could not be attributed (and why). Changes that brought no new piece are not reported.
    pub fn finish(&mut self, now: Instant, book: &dyn RecipeBook) -> Option<Outcome> {
        let (started, last) = (self.pending.started?, self.pending.last?);
        let (quiet, too_old) = (now.duration_since(last) >= QUIET, now.duration_since(started) >= HOLD);
        if (!quiet && !too_old) || (!too_old && self.pending.unfinished()) {
            return None;
        }
        evaluate(&std::mem::take(&mut self.pending), book)
    }
}

/// What a settled set of changes turned out to be.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Seen(Observation),
    /// A piece the game data has no recipe for (for that route): its recipe and cost, as watched.
    Learned(Learned),
    Skipped(Skip),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Learned {
    pub kind: u8,
    pub id: u16,
    pub route: Route,
    pub cost: u32,
    /// What vanished, by item id; sorted.
    pub materials: Vec<ItemStack>,
    /// For an upgrade, the weapon that vanished.
    pub parent: Option<u16>,
}

/// A piece appeared but its cost was not recorded.
#[derive(Debug, PartialEq, Eq)]
pub struct Skip {
    pub pieces: Vec<(u8, u16)>,
    pub why: SkipReason,
    /// What changed in the transaction, for the log: the wallet, and each item's change (pouch and box together).
    pub zenny: i64,
    pub items: Vec<(u16, i64)>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum SkipReason {
    /// The wallet did not go down.
    NoCost,
    /// More than one piece appeared in the same moment.
    SeveralPieces,
    /// We have no recipe for the piece, so there is nothing to check the items against.
    UnknownRecipe,
    /// The items that vanished are not that piece's recipe (or, for an upgrade, the parent did not vanish).
    DifferentMaterials,
}

/// A crafting is: the wallet went down, exactly one piece appeared, and the materials that vanished are exactly one of that
/// piece's recipes (for an upgrade, with one of its parent weapons vanishing too). If the game data has no recipe for the
/// route used, what vanished (provided nothing increased) is taken as the recipe and reported as learned. Anything else is
/// not attributed.
fn evaluate(p: &Pending, book: &dyn RecipeBook) -> Option<Outcome> {
    let added: Vec<(u8, u16)> = p
        .pieces
        .iter()
        .filter(|(_, d)| **d > 0)
        .flat_map(|(piece, d)| std::iter::repeat_n(*piece, *d as usize))
        .collect();
    if added.is_empty() {
        return None;
    }
    let mut item_changes: Vec<(u16, i64)> = p.items.iter().map(|(i, d)| (*i, *d)).collect();
    item_changes.sort();
    let skip = |why| {
        Some(Outcome::Skipped(Skip {
            pieces: added.clone(),
            why,
            zenny: p.zenny,
            items: item_changes.clone(),
        }))
    };
    if p.zenny >= 0 {
        return skip(SkipReason::NoCost);
    }
    let [(kind, id)] = added[..] else {
        return skip(SkipReason::SeveralPieces);
    };
    let removed: Vec<_> = p.pieces.iter().filter(|(_, d)| **d < 0).collect();
    let cost = u32::try_from(-p.zenny).ok()?;
    let (route, parent) = match removed[..] {
        [] => (Route::Create, None),
        [(&(removed_kind, parent), &-1)] if removed_kind == kind => (Route::Upgrade, Some(parent)),
        _ => return skip(SkipReason::DifferentMaterials),
    };
    let matches = |materials: &[ItemStack]| {
        let expected: HashMap<u16, i64> = materials.iter().map(|m| (m.id, -i64::from(m.count))).collect();
        expected == p.items
    };
    let known = match route {
        Route::Create => book.create_materials(kind, id),
        Route::Upgrade => match book.upgrade(kind, id) {
            // a recipe with other parents: not this crafting
            Some((_, parents)) if !parent.is_some_and(|p| parents.contains(&p)) => return skip(SkipReason::DifferentMaterials),
            other => other.map(|(materials, _)| materials),
        },
    };
    match known {
        Some(materials) if matches(&materials) => Some(Outcome::Seen(Observation { kind, id, route, cost })),
        Some(_) => skip(SkipReason::DifferentMaterials),
        None => {
            // No recipe to check against: take what vanished as the recipe, but only if that is all that happened to the items.
            if p.items.values().any(|d| *d > 0) {
                return skip(SkipReason::UnknownRecipe);
            }
            let mut materials = Vec::new();
            for (item, delta) in &p.items {
                let Ok(count) = u16::try_from(-delta) else {
                    return skip(SkipReason::UnknownRecipe);
                };
                materials.push(ItemStack { id: *item, count });
            }
            materials.sort_by_key(|m| m.id);
            Some(Outcome::Learned(Learned {
                kind,
                id,
                route,
                cost,
                materials,
                parent,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::Equipment;

    fn stack(id: u16, count: u16) -> ItemStack {
        ItemStack { id, count }
    }

    fn piece(slot: u16, kind: u8, id: u16) -> Equipment {
        Equipment {
            slot,
            kind,
            upgrade: 0,
            id,
            raw_tail: [0; 12],
        }
    }

    fn save(zenny: u32, items: &[(u16, u16)], pieces: &[(u8, u16)]) -> Save {
        Save {
            hunter_name: "T".into(),
            zenny,
            pouch: Vec::new(),
            item_box: items.iter().map(|&(i, c)| stack(i, c)).filter(|s| s.count > 0).collect(),
            hunted: Vec::new(),
            equipment_box: pieces.iter().enumerate().map(|(n, &(k, i))| piece(n as u16, k, i)).collect(),
            worn_slots: Vec::new(),
        }
    }

    /// Piece (5, 10) is made from 2 x item 1 and 2 x item 2; piece (7, 30) is an upgrade of (7, 20) for item 3 x 5.
    struct Book;
    impl RecipeBook for Book {
        fn create_materials(&self, kind: u8, id: u16) -> Option<Vec<ItemStack>> {
            ((kind, id) == (5, 10)).then(|| vec![stack(1, 2), stack(2, 2)])
        }
        fn upgrade(&self, kind: u8, id: u16) -> Option<(Vec<ItemStack>, Vec<u16>)> {
            ((kind, id) == (7, 30)).then(|| (vec![stack(3, 5)], vec![20]))
        }
    }

    const MS: fn(u64) -> Duration = Duration::from_millis;

    /// The observation, if the outcome was a recorded price.
    fn seen(outcome: Option<Outcome>) -> Option<Observation> {
        match outcome {
            Some(Outcome::Seen(o)) => Some(o),
            _ => None,
        }
    }

    #[test]
    fn sees_a_piece_created_and_waits_for_the_changes_to_settle() {
        let t0 = Instant::now();
        let mut t = PriceTracker::default();
        t.observe(&save(5000, &[(1, 3), (2, 2)], &[]), t0);
        t.observe(&save(4700, &[(1, 1), (2, 0)], &[(5, 10)]), t0 + MS(100));
        assert_eq!(t.finish(t0 + MS(500), &Book), None, "still within the quiet period");
        let seen = seen(t.finish(t0 + MS(1300), &Book));
        assert_eq!(
            seen,
            Some(Observation {
                kind: 5,
                id: 10,
                route: Route::Create,
                cost: 300
            })
        );
        assert_eq!(t.finish(t0 + MS(2500), &Book), None, "reported once");
    }

    #[test]
    fn changes_split_over_several_updates_are_one_transaction() {
        let t0 = Instant::now();
        let mut t = PriceTracker::default();
        t.observe(&save(5000, &[(1, 3), (2, 2)], &[]), t0);
        t.observe(&save(4700, &[(1, 3), (2, 2)], &[]), t0 + MS(100)); // wallet first
        t.observe(&save(4700, &[(1, 1), (2, 0)], &[]), t0 + MS(300)); // then materials
        t.observe(&save(4700, &[(1, 1), (2, 0)], &[(5, 10)]), t0 + MS(500)); // then the piece
        assert_eq!(seen(t.finish(t0 + MS(2000), &Book)).map(|o| o.cost), Some(300));
    }

    #[test]
    fn sees_an_upgrade_when_the_parent_vanishes() {
        let t0 = Instant::now();
        let mut t = PriceTracker::default();
        t.observe(&save(3000, &[(3, 9)], &[(7, 20)]), t0);
        t.observe(&save(1500, &[(3, 4)], &[(7, 30)]), t0 + MS(100));
        let seen = seen(t.finish(t0 + MS(1500), &Book));
        assert_eq!(
            seen,
            Some(Observation {
                kind: 7,
                id: 30,
                route: Route::Upgrade,
                cost: 1500
            })
        );
    }

    #[test]
    fn says_why_it_skipped_a_piece_it_cannot_attribute() {
        let t0 = Instant::now();
        let run = |before: Save, after: Save| {
            let mut t = PriceTracker::default();
            t.observe(&before, t0);
            t.observe(&after, t0 + MS(100));
            t.finish(t0 + MS(2000), &Book)
        };
        // a transaction that looks half finished (a piece with no payment yet) is held until HOLD has passed
        let run_late = |before: Save, after: Save| {
            let mut t = PriceTracker::default();
            t.observe(&before, t0);
            t.observe(&after, t0 + MS(100));
            t.finish(t0 + MS(11_000), &Book)
        };
        let skipped = |why: SkipReason, pieces: &[(u8, u16)]| Some((pieces.to_vec(), why));
        // the reason for a skipped piece, ignoring the detail kept for the log
        let reason = |outcome: Option<Outcome>| match outcome {
            Some(Outcome::Skipped(s)) => Some((s.pieces, s.why)),
            _ => None,
        };
        let run_late = |before: Save, after: Save| reason(run_late(before, after));
        let run = |before: Save, after: Save| reason(run(before, after));
        // only buying an item: no piece appeared, so there is nothing to report
        assert_eq!(run(save(500, &[(9, 0)], &[]), save(300, &[(9, 5)], &[])), None);
        // the piece appeared and zenny fell, but a different set of materials vanished
        assert_eq!(
            run(save(5000, &[(1, 3), (2, 2)], &[]), save(4700, &[(1, 1), (2, 1)], &[(5, 10)])),
            skipped(SkipReason::DifferentMaterials, &[(5, 10)])
        );
        // an extra item was used at the same time
        assert_eq!(
            run(
                save(5000, &[(1, 3), (2, 2), (8, 5)], &[]),
                save(4700, &[(1, 1), (2, 0), (8, 4)], &[(5, 10)])
            ),
            skipped(SkipReason::DifferentMaterials, &[(5, 10)])
        );
        // the wallet went up (e.g. something was sold) while a piece appeared
        assert_eq!(
            run_late(save(5000, &[(1, 3), (2, 2)], &[]), save(5100, &[(1, 1), (2, 0)], &[(5, 10)])),
            skipped(SkipReason::NoCost, &[(5, 10)])
        );
        // two pieces at once
        assert_eq!(
            run(
                save(5000, &[(1, 3), (2, 2)], &[]),
                save(4000, &[(1, 1), (2, 0)], &[(5, 10), (5, 10)])
            ),
            skipped(SkipReason::SeveralPieces, &[(5, 10), (5, 10)])
        );
        // a piece the recipe book knows nothing about, while an item also increased: not safe to learn from
        assert_eq!(
            run(save(5000, &[(1, 3)], &[]), save(4700, &[(1, 1), (8, 2)], &[(5, 99)])),
            skipped(SkipReason::UnknownRecipe, &[(5, 99)])
        );
    }

    #[test]
    fn learns_the_recipe_of_a_piece_the_game_data_lacks() {
        let t0 = Instant::now();
        let run = |before: Save, after: Save| {
            let mut t = PriceTracker::default();
            t.observe(&before, t0);
            t.observe(&after, t0 + MS(100));
            t.finish(t0 + MS(2000), &Book)
        };
        // create: piece (5, 99) is not in the book; two of item 1 and one of item 4 vanished, 300 paid
        let learned = run(save(5000, &[(4, 2), (1, 3)], &[]), save(4700, &[(1, 1)], &[(5, 99)]));
        assert_eq!(
            learned,
            Some(Outcome::Learned(Learned {
                kind: 5,
                id: 99,
                route: Route::Create,
                cost: 300,
                materials: vec![stack(1, 2), stack(4, 2)],
                parent: None,
            }))
        );
        // upgrade: piece (7, 31) has no upgrade recipe in the book; parent (7, 20) vanished, 5 of item 3 used
        let learned = run(save(3000, &[(3, 9)], &[(7, 20)]), save(1500, &[(3, 4)], &[(7, 31)]));
        assert_eq!(
            learned,
            Some(Outcome::Learned(Learned {
                kind: 7,
                id: 31,
                route: Route::Upgrade,
                cost: 1500,
                materials: vec![stack(3, 5)],
                parent: Some(20),
            }))
        );
        // the data lists only an upgrade for (7, 30), but it was created from scratch (the parent stayed): a create recipe is learned
        let learned = run(save(3000, &[(3, 9)], &[(7, 20)]), save(1500, &[(3, 4)], &[(7, 20), (7, 30)]));
        assert!(matches!(
            learned,
            Some(Outcome::Learned(Learned {
                kind: 7,
                id: 30,
                route: Route::Create,
                parent: None,
                ..
            }))
        ));
        // a piece sold for nothing but zenny (no materials at all) still learns an empty recipe
        // (held while it waits for items that never come, so judged after the hold)
        let learned = {
            let mut t = PriceTracker::default();
            t.observe(&save(5000, &[], &[]), t0);
            t.observe(&save(4400, &[], &[(5, 99)]), t0 + MS(100));
            t.finish(t0 + MS(11_000), &Book)
        };
        assert!(matches!(learned, Some(Outcome::Learned(Learned { cost: 600, ref materials, .. })) if materials.is_empty()));
    }

    #[test]
    fn a_known_recipe_that_does_not_match_is_never_overwritten_by_learning() {
        let t0 = Instant::now();
        let mut t = PriceTracker::default();
        // piece (5, 10) has a recipe in the book (2 x item 1, 2 x item 2); this crafting used something else
        t.observe(&save(5000, &[(1, 3), (2, 2), (8, 5)], &[]), t0);
        t.observe(&save(4700, &[(1, 1), (8, 3)], &[(5, 10)]), t0 + MS(100));
        assert_eq!(
            t.finish(t0 + MS(2000), &Book),
            Some(Outcome::Skipped(Skip {
                pieces: vec![(5, 10)],
                why: SkipReason::DifferentMaterials,
                zenny: -300,
                items: vec![(1, -2), (2, -2), (8, -2)],
            }))
        );
    }

    #[test]
    fn ledger_keeps_a_learned_recipe_and_its_parent_through_the_file() {
        let mut l = Ledger::default();
        let learned = PriceEntry {
            kind: 7,
            id: 31,
            route: Route::Upgrade,
            cost: 1500,
            source: Source::Learned,
            when: 1,
            name: "Commander's Dagger".into(),
            materials: Some(vec![stack(3, 5), stack(9, 1)]),
            parent: Some(20),
        };
        l.record(learned);
        let plain = PriceEntry {
            kind: 5,
            id: 10,
            route: Route::Create,
            cost: 300,
            source: Source::Seen,
            when: 2,
            name: "Piscine Mask".into(),
            materials: None,
            parent: None,
        };
        l.record(plain);
        let text = l.format();
        assert!(
            text.contains("7\t31\tupgrade\t1500\tlearned\t1\tCommander's Dagger\t3:5,9:1\t20\n"),
            "{text}"
        );
        assert!(
            text.contains("5\t10\tcreate\t300\tseen\t2\tPiscine Mask\n"),
            "plain rows keep their short form: {text}"
        );
        let back = Ledger::parse(&text);
        let e = back.get(7, 31, Route::Upgrade).unwrap();
        assert_eq!((e.materials.clone(), e.parent), (Some(vec![stack(3, 5), stack(9, 1)]), Some(20)));
        assert_eq!(back.get(5, 10, Route::Create).unwrap().materials, None);
        assert_eq!(back.learned().count(), 1);
    }

    #[test]
    fn debug_edits_are_not_counted_and_a_crafting_right_after_them_still_is() {
        let t0 = Instant::now();
        let mut t = PriceTracker::default();
        let before = save(5000, &[(1, 3), (2, 2)], &[]);
        t.observe(&before, t0);
        // the editor stocks materials and sets the wallet; the tracker is told what the game will look like
        let edited = save(9000, &[(1, 9), (2, 9)], &[]);
        t.rebase(&edited);
        // the game is seen in its old state first, then the new one: the two steps cancel
        t.observe(&before, t0 + MS(100));
        t.observe(&edited, t0 + MS(300));
        assert_eq!(t.finish(t0 + MS(1600), &Book), None, "an edit alone is nothing");
        // the player crafts a second after the command: 300 z and two of each material
        t.observe(&save(8700, &[(1, 7), (2, 7)], &[(5, 10)]), t0 + MS(1200));
        assert_eq!(seen(t.finish(t0 + MS(3000), &Book)).map(|o| o.cost), Some(300));
    }

    #[test]
    fn an_edit_seen_at_once_leaves_nothing_behind() {
        let t0 = Instant::now();
        let mut t = PriceTracker::default();
        t.observe(&save(5000, &[(1, 3)], &[]), t0);
        let edited = save(4000, &[(1, 5), (7, 2)], &[]);
        t.rebase(&edited);
        t.observe(&edited, t0 + MS(100));
        assert_eq!(t.finish(t0 + MS(2000), &Book), None);
        assert_eq!(t.finish(t0 + MS(12_000), &Book), None);
    }

    #[test]
    fn a_crafting_split_by_a_long_animation_is_still_one_crafting() {
        let t0 = Instant::now();
        let secs = Duration::from_secs;
        // payment and materials first, the piece only after a 2.4 second animation
        let mut t = PriceTracker::default();
        t.observe(&save(5000, &[(1, 3), (2, 2)], &[]), t0);
        t.observe(&save(4700, &[(1, 1), (2, 0)], &[]), t0 + MS(100));
        assert_eq!(t.finish(t0 + MS(1500), &Book), None, "held: the first half of a crafting");
        t.observe(&save(4700, &[(1, 1), (2, 0)], &[(5, 10)]), t0 + MS(2500));
        assert_eq!(seen(t.finish(t0 + secs(4), &Book)).map(|o| (o.id, o.cost)), Some((10, 300)));

        // the other way round: the piece first, then the payment and materials
        let mut t = PriceTracker::default();
        t.observe(&save(5000, &[(1, 3), (2, 2)], &[]), t0);
        t.observe(&save(5000, &[(1, 3), (2, 2)], &[(5, 10)]), t0 + MS(100));
        assert_eq!(t.finish(t0 + MS(1500), &Book), None);
        t.observe(&save(4700, &[(1, 1), (2, 0)], &[(5, 10)]), t0 + MS(2200));
        assert_eq!(seen(t.finish(t0 + secs(4), &Book)).map(|o| o.cost), Some(300));
    }

    #[test]
    fn a_half_crafting_that_never_completes_is_dropped_and_does_not_taint_the_next() {
        let t0 = Instant::now();
        let secs = Duration::from_secs;
        let mut t = PriceTracker::default();
        t.observe(&save(5000, &[(1, 3), (2, 2)], &[]), t0);
        t.observe(&save(4900, &[(1, 2), (2, 2)], &[]), t0 + MS(100)); // wallet down, an item used, no piece
        assert_eq!(t.finish(t0 + secs(5), &Book), None, "still held");
        assert_eq!(t.finish(t0 + secs(11), &Book), None, "dropped after the hold, nothing reported");
        // a later crafting is judged on its own
        t.observe(&save(4400, &[(1, 0), (2, 0)], &[(5, 10)]), t0 + secs(20));
        assert_eq!(seen(t.finish(t0 + secs(22), &Book)).map(|o| o.cost), Some(500));
    }

    #[test]
    fn buying_items_or_paying_a_fee_is_not_held_and_does_not_taint_a_later_crafting() {
        let t0 = Instant::now();
        let secs = Duration::from_secs;
        let mut t = PriceTracker::default();
        t.observe(&save(5000, &[(1, 3), (2, 2)], &[]), t0);
        t.observe(&save(4950, &[(1, 3), (2, 2), (9, 1)], &[]), t0 + MS(100)); // bought an item
        assert_eq!(t.finish(t0 + MS(1500), &Book), None);
        t.observe(&save(4850, &[(1, 3), (2, 2), (9, 1)], &[]), t0 + secs(3)); // a fee
        assert_eq!(t.finish(t0 + secs(5), &Book), None);
        t.observe(&save(4550, &[(1, 1), (2, 0), (9, 1)], &[(5, 10)]), t0 + secs(8)); // then a crafting
        assert_eq!(seen(t.finish(t0 + secs(10), &Book)).map(|o| o.cost), Some(300), "not 400 or 450");
    }

    #[test]
    fn ledger_round_trips_and_reports_changes() {
        let mut l = Ledger::default();
        let entry = |cost| PriceEntry {
            kind: 5,
            id: 10,
            route: Route::Create,
            cost,
            source: Source::Seen,
            when: 1790995354,
            name: "Piscine Mask".into(),
            materials: None,
            parent: None,
        };
        assert_eq!(l.record(entry(300)), Recorded::New);
        assert_eq!(l.record(entry(300)), Recorded::Unchanged);
        let upgrade = PriceEntry {
            route: Route::Upgrade,
            cost: 1500,
            source: Source::Notes,
            ..entry(0)
        };
        assert_eq!(l.record(upgrade), Recorded::New, "create and upgrade are separate");
        let text = l.format();
        assert!(text.contains("5\t10\tcreate\t300\tseen\t1790995354\tPiscine Mask"));
        let back = Ledger::parse(&text);
        assert_eq!(back.get(5, 10, Route::Create).map(|e| e.cost), Some(300));
        assert_eq!(
            back.get(5, 10, Route::Upgrade).map(|e| (e.cost, e.source)),
            Some((1500, Source::Notes))
        );
        let mut back = back;
        assert_eq!(back.record(entry(350)), Recorded::Changed(300));
    }

    #[test]
    fn ledger_ignores_comments_and_bad_lines() {
        let text = "# header\n5\t10\tcreate\t300\tseen\t1\tA\nnot a line\n5\t11\tbuy\t1\tseen\t1\tB\n5\t12\tcreate\tx\tseen\t1\tC\n";
        let l = Ledger::parse(text);
        assert_eq!(l.entries().len(), 1);
    }
}

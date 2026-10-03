//! What monsters drop when carved, read from the game executable's data section.
//!
//! There are three pointer tables per rank (low, high, G): one each for body carves, tail carves and shiny drops. Each table has
//! 101 entries indexed by monster id (the index of the monster's name in the text archive; entry 0 is unused), and each entry points
//! to a list of 4-byte records `[0, chance in percent, item id u16]` ended by `ff ff 00 00`. The record's first byte is always zero,
//! so every carve gives one item. The chances of a real list add up to 100; monsters with nothing to carve point at filler, which
//! that check (and the item id range) rejects.
//!
//! Capture rewards and part-break rewards are lists of `[item id u16, quantity, chance]` ended by a zero record. Their pointers sit
//! in one flat table per rank (203 entries) with 2 to 6 lists per monster, and nothing in the data says where one monster's lists end.
//! They are assigned by their contents: each list goes to the monster whose carve items it mostly holds, with monsters in id order.
//! A capturable monster's group is `[capture, break lists..., one more list]` (the last repeats the capture's items and is not
//! shown); a monster that cannot be captured has only break lists. The break lists are in the game's order, but which body part each
//! is, is not known. That inference reproduces 106 of the 108 capture and break lists checked for 12 monsters.
//!
//! Found by matching published drop lists; offsets are for the US v32 executable.

use anyhow::{Result, bail};
use std::collections::{HashMap, HashSet};

/// Where monster 1's entry sits for each rank's first table (body carves); the tail and shiny tables follow `TABLE_STRIDE` apart.
const RANK_STARTS: [usize; 3] = [0x765e0, 0x76dc8, 0x775b0];
const TABLE_STRIDE: usize = 0x194;
/// Monsters 1 to 100 have entries.
const MONSTERS: u16 = 100;
const MAX_ITEM_ID: u16 = 1550;
const MAX_LIST: usize = 12;
/// Start of each rank's flat table of capture and part-break list pointers, and how many entries each has.
const FLAT_STARTS: [usize; 3] = [0x78fd8, 0x79304, 0x79630];
const FLAT_LEN: usize = 203;
/// The rows of the tables are not in the order of the name table: rows for small creatures sit between the monsters (and the
/// fish has none), so later monsters' rows are further down than their names. Worked out from the contents (a row of Arzuros Pelt
/// and Arzuros Shell is Arzuros; a row with Popo Tongue is Popo, ...). Each entry is (first row, last row, name id of the first
/// row); a row in none of them belongs to no named monster.
const ROW_RUNS: [(u16, u16, u16); 7] = [
    (1, 27, 1),     // Rathian .. Aptonoth
    (29, 32, 28),   // Popo, Rhenoplos, Felyne, Melynx
    (33, 39, 33),   // Altaroth .. the Bnahabra
    (52, 53, 41),   // Zinogre, Arzuros
    (56, 63, 43),   // Lagombi .. Gargwa
    (66, 93, 51),   // Crimson Qurupeco .. Slagtoth
    (100, 100, 85), // Hallowed Jhen Mohran
];
/// What it costs to move on to the next monster when assigning lists; keeps the assignment from flickering between monsters.
const SWITCH_COST: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rank {
    Low,
    High,
    G,
}

impl Rank {
    pub const ALL: [Rank; 3] = [Rank::Low, Rank::High, Rank::G];

    pub fn label(self) -> &'static str {
        match self {
            Rank::Low => "Low rank",
            Rank::High => "High rank",
            Rank::G => "G rank",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Method {
    BodyCarve,
    TailCarve,
    Shiny,
    Capture,
    /// The nth part-break reward (from 1), in the game's order for that monster.
    Break(u8),
}

impl Method {
    /// The kinds of drop read from the carve tables.
    pub const CARVES: [Method; 3] = [Method::BodyCarve, Method::TailCarve, Method::Shiny];

    pub fn label(self) -> String {
        match self {
            Method::BodyCarve => "Body carve".to_string(),
            Method::TailCarve => "Tail carve".to_string(),
            Method::Shiny => "Shiny drops".to_string(),
            Method::Capture => "Capture".to_string(),
            Method::Break(n) => format!("Part break {n}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drop {
    pub item: u16,
    pub quantity: u8,
    /// Chance in percent.
    pub percent: u8,
}

/// One way to get an item: this monster drops it in this rank by this method, with this chance (percent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Source {
    pub monster: u16,
    pub rank: Rank,
    pub method: Method,
    pub percent: u8,
}

/// Every drop list, with the lookups by item and by monster worked out once up front (the app asks for them every frame).
#[derive(Debug, Default)]
pub struct Drops {
    lists: HashMap<(u16, Rank, Method), Vec<Drop>>,
    /// Monster ids that have at least one list, ascending.
    monsters: Vec<u16>,
    by_item: HashMap<u16, Vec<Source>>,
    /// A monster's lists as (method, rank), in display order.
    by_monster: HashMap<u16, Vec<(Method, Rank)>>,
}

/// Read the list `ptr` points at, if it is a real drop list.
fn read_list(data: &[u8], data_addr: u32, ptr: u32) -> Option<Vec<Drop>> {
    let mut at = usize::try_from(ptr.checked_sub(data_addr)?).ok()?;
    let mut out = Vec::new();
    loop {
        let rec = data.get(at..at + 4)?;
        if rec == [0xff, 0xff, 0, 0] {
            break;
        }
        let item = u16::from_be_bytes([rec[2], rec[3]]);
        if rec[0] != 0 || item == 0 || item > MAX_ITEM_ID || out.len() >= MAX_LIST {
            return None;
        }
        out.push(Drop {
            item,
            quantity: 1,
            percent: rec[1],
        });
        at += 4;
    }
    (!out.is_empty() && out.iter().map(|d| u32::from(d.percent)).sum::<u32>() == 100).then_some(out)
}

/// Parse every carve table. `data_addr` is the virtual address of the data section (pointers are absolute).
pub fn parse(data: &[u8], data_addr: u32) -> Result<Drops> {
    let mut lists = HashMap::new();
    for (r, &start) in RANK_STARTS.iter().enumerate() {
        for (j, method) in Method::CARVES.into_iter().enumerate() {
            for monster in 1..=MONSTERS {
                let at = start + TABLE_STRIDE * j + 4 * usize::from(monster - 1);
                let Some(word) = data.get(at..at + 4) else {
                    bail!("drop table runs past the data section; unsupported executable?");
                };
                let ptr = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
                if let Some(list) = read_list(data, data_addr, ptr) {
                    lists.insert((monster, Rank::ALL[r], method), list);
                }
            }
        }
    }
    if lists.len() < 300 {
        bail!("only {} monster drop lists found; unsupported executable?", lists.len());
    }
    let pools = item_pools(&lists);
    for (r, &start) in FLAT_STARTS.iter().enumerate() {
        let Some(table) = data.get(start..start + 4 * FLAT_LEN) else {
            bail!("capture and break table runs past the data section; unsupported executable?");
        };
        let flat: Vec<Vec<Drop>> = table
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| read_reward_list(data, data_addr, u32::from_be_bytes(*w)).unwrap_or_default())
            .collect();
        for (monster, group) in assign_groups(&flat, &pools) {
            let (capture, breaks) = split_group(group);
            if let Some(list) = capture {
                lists.insert((monster, Rank::ALL[r], Method::Capture), list);
            }
            for (n, list) in breaks.into_iter().enumerate() {
                lists.insert((monster, Rank::ALL[r], Method::Break(n as u8 + 1)), list);
            }
        }
    }
    Ok(Drops::new(lists_by_name(lists)))
}

/// The name id of a table row, or `None` if the row belongs to no named monster.
fn monster_of_row(row: u16) -> Option<u16> {
    ROW_RUNS
        .iter()
        .find(|&&(first, last, _)| (first..=last).contains(&row))
        .map(|&(first, _, name)| name + (row - first))
}

/// Re-key the lists from table rows to name ids, dropping the rows that no named monster owns.
fn lists_by_name(lists: HashMap<(u16, Rank, Method), Vec<Drop>>) -> HashMap<(u16, Rank, Method), Vec<Drop>> {
    lists
        .into_iter()
        .filter_map(|((row, rank, method), list)| Some(((monster_of_row(row)?, rank, method), list)))
        .collect()
}

/// The items each monster is known to drop, from its carve lists in every rank.
fn item_pools(lists: &HashMap<(u16, Rank, Method), Vec<Drop>>) -> Vec<(u16, HashSet<u16>)> {
    let mut pools: HashMap<u16, HashSet<u16>> = HashMap::new();
    for (&(monster, ..), list) in lists {
        pools.entry(monster).or_default().extend(list.iter().map(|d| d.item));
    }
    let mut out: Vec<(u16, HashSet<u16>)> = pools.into_iter().collect();
    out.sort_unstable_by_key(|(m, _)| *m);
    out
}

/// A capture or part-break list: records `[item id u16, quantity, chance]` ended by a zero item. `None` if it is not a real list.
fn read_reward_list(data: &[u8], data_addr: u32, ptr: u32) -> Option<Vec<Drop>> {
    let mut at = usize::try_from(ptr.checked_sub(data_addr)?).ok()?;
    let mut out = Vec::new();
    loop {
        let rec = data.get(at..at + 4)?;
        let item = u16::from_be_bytes([rec[0], rec[1]]);
        if item == 0 {
            break;
        }
        if item > MAX_ITEM_ID || rec[2] == 0 || out.len() >= MAX_LIST {
            return None;
        }
        out.push(Drop {
            item,
            quantity: rec[2],
            percent: rec[3],
        });
        at += 4;
    }
    (!out.is_empty() && out.iter().map(|d| u32::from(d.percent)).sum::<u32>() == 100).then_some(out)
}

/// Give every list in the flat table to a monster. The lists of one monster are next to each other and monsters come in id order,
/// so this picks, for each list, the monster whose carve items it holds most of, never going back to an earlier monster; each run of
/// lists given to one monster is that monster's group. Returns (monster, its lists in table order).
fn assign_groups(flat: &[Vec<Drop>], pools: &[(u16, HashSet<u16>)]) -> Vec<(u16, Vec<Vec<Drop>>)> {
    if flat.is_empty() || pools.is_empty() {
        return Vec::new();
    }
    let (n, m) = (flat.len(), pools.len());
    let score = |i: usize, k: usize| -> f64 {
        if flat[i].is_empty() {
            return 0.0;
        }
        flat[i].iter().filter(|d| pools[k].1.contains(&d.item)).count() as f64 / flat[i].len() as f64
    };
    // dp[i][k]: best total score with list i given to monster k (monsters never decrease down the table)
    let mut dp = vec![vec![f64::NEG_INFINITY; m]; n];
    let mut back = vec![vec![0usize; m]; n];
    for (k, cell) in dp[0].iter_mut().enumerate() {
        *cell = score(0, k);
    }
    for i in 1..n {
        let (mut best, mut best_k) = (f64::NEG_INFINITY, 0);
        for k in 0..m {
            let stay = dp[i - 1][k];
            let switch = best - SWITCH_COST;
            if stay >= switch {
                dp[i][k] = stay + score(i, k);
                back[i][k] = k;
            } else {
                dp[i][k] = switch + score(i, k);
                back[i][k] = best_k;
            }
            if dp[i - 1][k] > best {
                best = dp[i - 1][k];
                best_k = k;
            }
        }
    }
    let mut k = (0..m).fold(0, |b, k| if dp[n - 1][k] > dp[n - 1][b] { k } else { b });
    let mut owners = vec![0usize; n];
    for i in (0..n).rev() {
        owners[i] = k;
        if i > 0 {
            k = back[i][k];
        }
    }
    let mut groups: Vec<(u16, Vec<Vec<Drop>>)> = Vec::new();
    for (i, &k) in owners.iter().enumerate() {
        let monster = pools[k].0;
        match groups.last_mut() {
            Some((m, lists)) if *m == monster => lists.push(flat[i].clone()),
            _ => groups.push((monster, vec![flat[i].clone()])),
        }
    }
    groups
}

/// Split a monster's group into its capture list and its part-break lists. If the last list repeats the first one's items, the
/// monster can be captured: the first is the capture reward and the last is an extra that is not used. Otherwise they are all breaks.
fn split_group(mut group: Vec<Vec<Drop>>) -> (Option<Vec<Drop>>, Vec<Vec<Drop>>) {
    let items = |l: &Vec<Drop>| l.iter().map(|d| d.item).collect::<HashSet<u16>>();
    let capturable = group.len() >= 2 && {
        let (first, last) = (items(&group[0]), items(&group[group.len() - 1]));
        !first.is_empty() && first.intersection(&last).count() as f64 / first.union(&last).count() as f64 >= 0.8
    };
    if capturable {
        group.pop();
        let capture = group.remove(0);
        (Some(capture), group.into_iter().filter(|l| !l.is_empty()).collect())
    } else {
        (None, group.into_iter().filter(|l| !l.is_empty()).collect())
    }
}

impl Drops {
    fn new(lists: HashMap<(u16, Rank, Method), Vec<Drop>>) -> Drops {
        let mut by_item: HashMap<u16, Vec<Source>> = HashMap::new();
        let mut by_monster: HashMap<u16, Vec<(Method, Rank)>> = HashMap::new();
        for (&(monster, rank, method), list) in &lists {
            by_monster.entry(monster).or_default().push((method, rank));
            for d in list {
                by_item.entry(d.item).or_default().push(Source {
                    monster,
                    rank,
                    method,
                    percent: d.percent,
                });
            }
        }
        by_item.values_mut().for_each(|v| v.sort_unstable());
        by_monster.values_mut().for_each(|v| v.sort_unstable());
        let mut monsters: Vec<u16> = by_monster.keys().copied().collect();
        monsters.sort_unstable();
        Drops {
            lists,
            monsters,
            by_item,
            by_monster,
        }
    }

    pub fn list(&self, monster: u16, rank: Rank, method: Method) -> Option<&[Drop]> {
        self.lists.get(&(monster, rank, method)).map(Vec::as_slice)
    }

    /// All of a monster's lists as (method, rank, list), ordered by kind of drop and then rank.
    pub fn lists_for(&self, monster: u16) -> impl Iterator<Item = (Method, Rank, &[Drop])> + '_ {
        self.by_monster
            .get(&monster)
            .into_iter()
            .flatten()
            .filter_map(move |&(method, rank)| Some((method, rank, self.list(monster, rank, method)?)))
    }

    /// Monster ids that have at least one drop list, ascending.
    pub fn monsters(&self) -> &[u16] {
        &self.monsters
    }

    /// Every way to get `item`, ordered by monster, then rank and method.
    pub fn sources(&self, item: u16) -> &[Source] {
        self.by_item.get(&item).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDR: u32 = 0x1000_0000;

    /// A data section with one list per (rank, method, monster) for monsters 1 to 3, each dropping item 100 + monster at 100%.
    fn section() -> Vec<u8> {
        let mut data = vec![0u8; 0x90000];
        let mut next = 0x80000;
        for &start in &RANK_STARTS {
            for j in 0..3 {
                for monster in 1..=MONSTERS {
                    let at = start + TABLE_STRIDE * j + 4 * usize::from(monster - 1);
                    data[at..at + 4].copy_from_slice(&(ADDR + next as u32).to_be_bytes());
                    // real list: one item at 100%, then the terminator
                    let item = 100 + monster;
                    data[next..next + 4].copy_from_slice(&[0, 100, (item >> 8) as u8, item as u8]);
                    data[next + 4..next + 8].copy_from_slice(&[0xff, 0xff, 0, 0]);
                    next += 8;
                }
            }
        }
        data
    }

    #[test]
    fn reads_lists_by_rank_method_and_monster() {
        let d = parse(&section(), ADDR).unwrap();
        assert_eq!(
            d.list(1, Rank::Low, Method::BodyCarve),
            Some(
                &[Drop {
                    item: 101,
                    quantity: 1,
                    percent: 100
                }][..]
            )
        );
        assert_eq!(
            d.list(85, Rank::G, Method::Shiny).unwrap()[0].item,
            200,
            "row 100 is the monster named 85"
        );
        assert_eq!(d.list(100, Rank::G, Method::Shiny), None);
        assert_eq!(d.monsters().len(), 77, "rows without a named monster are left out");
        assert!(
            d.list(1, Rank::Low, Method::Capture).is_none(),
            "no capture or break lists in the fake section"
        );
        assert_eq!(d.list(101, Rank::Low, Method::BodyCarve), None, "there is no monster 101");
    }

    #[test]
    fn finds_the_monsters_that_drop_an_item() {
        let d = parse(&section(), ADDR).unwrap();
        let sources = d.sources(105);
        assert_eq!(sources.len(), 9, "monster 5 in 3 ranks x 3 methods");
        assert!(sources.iter().all(|s| s.monster == 5 && s.percent == 100));
        assert_eq!(
            sources[0],
            Source {
                monster: 5,
                rank: Rank::Low,
                method: Method::BodyCarve,
                percent: 100
            }
        );
    }

    #[test]
    fn filler_and_lists_that_do_not_add_up_are_rejected() {
        let mut data = section();
        let at = RANK_STARTS[0];
        let ptr = u32::from_be_bytes(data[at..at + 4].try_into().unwrap());
        let list = (ptr - ADDR) as usize;
        data[list + 1] = 60; // 60% only
        let d = parse(&data, ADDR).unwrap();
        assert_eq!(d.list(1, Rank::Low, Method::BodyCarve), None);
        assert!(d.list(2, Rank::Low, Method::BodyCarve).is_some());
        // a pointer at nothing
        data[at..at + 4].copy_from_slice(&0u32.to_be_bytes());
        assert!(parse(&data, ADDR).is_ok());
    }

    fn drops(items: &[(u16, u8)]) -> Vec<Drop> {
        items
            .iter()
            .map(|&(item, percent)| Drop {
                item,
                quantity: 1,
                percent,
            })
            .collect()
    }

    #[test]
    fn lists_are_given_to_the_monster_whose_items_they_hold_and_grouped_in_order() {
        let pools: Vec<(u16, HashSet<u16>)> = vec![
            (1, HashSet::from([10, 11, 12])),
            (2, HashSet::from([20, 21])),
            (3, HashSet::from([30, 31])),
        ];
        let flat = vec![
            drops(&[(10, 60), (11, 40)]), // monster 1: capture
            drops(&[(12, 100)]),          // break
            drops(&[(10, 50), (11, 50)]), // same items as the capture
            drops(&[(20, 100)]),          // monster 2 cannot be captured: breaks only
            drops(&[(99, 100)]),          // generic items stay with the monster around them
            drops(&[(21, 100)]),
            drops(&[(30, 70), (31, 30)]), // monster 3
            drops(&[(30, 20), (31, 80)]),
        ];
        let groups = assign_groups(&flat, &pools);
        let shape: Vec<(u16, usize)> = groups.iter().map(|(m, l)| (*m, l.len())).collect();
        assert_eq!(shape, [(1, 3), (2, 3), (3, 2)]);
        let (capture, breaks) = split_group(groups[0].1.clone());
        assert_eq!(capture, Some(drops(&[(10, 60), (11, 40)])));
        assert_eq!(breaks, [drops(&[(12, 100)])]);
        let (capture, breaks) = split_group(groups[1].1.clone());
        assert_eq!((capture, breaks.len()), (None, 3));
        let (capture, breaks) = split_group(groups[2].1.clone());
        assert!(capture.is_some() && breaks.is_empty(), "a capture and its repeat, no breaks");
    }

    #[test]
    fn a_section_that_does_not_have_the_tables_is_an_error() {
        assert!(parse(&vec![0u8; 0x90000], ADDR).is_err(), "no lists found");
        assert!(parse(&[0u8; 100], ADDR).is_err(), "too short");
    }
}

#[cfg(test)]
mod real_data {
    use super::*;
    use crate::recipes::DATA_SECTION_ADDR;

    /// Rathian's lists against the extracted data section, when it is around. The percentages match a published drop list.
    #[test]
    fn rathian_drops_match_the_published_list() {
        let Ok(data) = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/live/rpx_data.bin")) else {
            return;
        };
        let d = parse(&data, DATA_SECTION_ADDR).unwrap();
        let pairs = |rank, method| {
            d.list(1, rank, method)
                .unwrap()
                .iter()
                .map(|x| (x.item, x.percent))
                .collect::<Vec<_>>()
        };
        // Rathian Scale, Shell, Webbing, Flame Sac, Spike
        assert_eq!(
            pairs(Rank::Low, Method::BodyCarve),
            [(563, 40), (566, 30), (569, 15), (289, 10), (570, 5)]
        );
        // Rathian Scale+, Carapace, Webbing, Inferno Sac, Spike+, Spike
        assert_eq!(
            pairs(Rank::High, Method::BodyCarve),
            [(564, 38), (567, 25), (569, 12), (290, 12), (571, 5), (570, 8)]
        );
        // Rathian Shard first in G rank
        assert_eq!(pairs(Rank::G, Method::BodyCarve)[0], (565, 38));
        let scale: Vec<_> = d
            .sources(563)
            .iter()
            .filter(|s| s.monster == 1)
            .map(|s| (s.rank, s.method, s.percent))
            .collect();
        assert_eq!(
            scale,
            [
                (Rank::Low, Method::BodyCarve, 40),
                (Rank::Low, Method::TailCarve, 62),
                (Rank::Low, Method::Shiny, 15),
                (Rank::Low, Method::Capture, 25),
                (Rank::Low, Method::Break(1), 25),
                (Rank::High, Method::TailCarve, 7),
            ]
        );
    }

    #[test]
    fn rows_after_the_small_monsters_map_to_their_names() {
        assert_eq!(monster_of_row(27), Some(27));
        assert_eq!(monster_of_row(28), None);
        assert_eq!(monster_of_row(29), Some(28), "Popo");
        assert_eq!(monster_of_row(33), Some(33));
        assert_eq!(monster_of_row(40), None);
        assert_eq!(monster_of_row(52), Some(41));
        assert_eq!(monster_of_row(53), Some(42));
        assert_eq!(monster_of_row(54), None);
        assert_eq!(monster_of_row(56), Some(43));
        assert_eq!(monster_of_row(66), Some(51));
        assert_eq!(monster_of_row(93), Some(78));
        assert_eq!(monster_of_row(100), Some(85));
    }

    /// From the extracted data section: Arzuros' items are Arzuros' own, not Sand Barioth's.
    #[test]
    fn later_monsters_keep_their_own_drops() {
        let Ok(data) = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/live/rpx_data.bin")) else {
            return;
        };
        let d = parse(&data, DATA_SECTION_ADDR).unwrap();
        let pelt = d.list(42, Rank::Low, Method::BodyCarve).unwrap();
        assert_eq!(pelt.len(), 3);
        assert_eq!(pelt[0].percent, 65, "Arzuros Pelt 65%");
        assert!(d.list(53, Rank::Low, Method::BodyCarve).is_none() || d.list(53, Rank::Low, Method::BodyCarve).unwrap()[0].percent != 65);
    }

    /// Capture and part-break rewards for Rathian and Ceadeus, from the extracted data section. They match a published list
    /// (Rathian's capture and head and wing breaks; the order of the breaks is the game's).
    #[test]
    fn capture_and_break_rewards_follow_the_monsters() {
        let Ok(data) = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/live/rpx_data.bin")) else {
            return;
        };
        let d = parse(&data, DATA_SECTION_ADDR).unwrap();
        let get = |m, method| {
            d.list(m, Rank::Low, method)
                .unwrap()
                .iter()
                .map(|x| (x.item, x.quantity, x.percent))
                .collect::<Vec<_>>()
        };
        // Rathian Shell, Scale x2, Rath Marrow, Flame Sac, Rathian Plate
        assert_eq!(
            get(1, Method::Capture),
            [(566, 1, 30), (563, 2, 25), (599, 1, 23), (289, 1, 20), (573, 1, 2)]
        );
        assert_eq!(get(1, Method::Break(1)), [(566, 1, 71), (563, 1, 25), (573, 1, 4)], "head");
        assert_eq!(
            get(1, Method::Break(2)),
            [(597, 1, 70), (318, 4, 15), (569, 1, 15)],
            "wing: Rath Talon, Wyvern Claw x4, Webbing"
        );
        assert!(d.list(1, Rank::Low, Method::Break(3)).is_none());
        // Ceadeus cannot be captured: only breaks
        assert!(d.list(20, Rank::Low, Method::Capture).is_none());
        assert_eq!(get(20, Method::Break(1))[0], (782, 1, 67));
        assert!(d.list(20, Rank::Low, Method::Break(3)).is_some());
        let talon = d
            .sources(597)
            .iter()
            .filter(|s| s.rank == Rank::Low && s.method == Method::Break(2))
            .count();
        assert!(talon >= 2, "Rath Talon: Rathian and Rathalos");
    }
}

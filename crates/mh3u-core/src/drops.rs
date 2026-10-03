//! What monsters drop when carved, read from the game executable's data section.
//!
//! There are three pointer tables per rank (low, high, G): one each for body carves, tail carves and shiny drops. Each table has
//! 101 entries indexed by monster id (the index of the monster's name in the text archive; entry 0 is unused), and each entry points
//! to a list of 4-byte records `[0, chance in percent, item id u16]` ended by `ff ff 00 00`. The record's first byte is always zero,
//! so every carve gives one item. The chances of a real list add up to 100; monsters with nothing to carve point at filler, which
//! that check (and the item id range) rejects.
//!
//! Capture rewards and part-break rewards are in the same section as `[item id u16, quantity, chance]` lists, but the pointers to
//! them are in one flat table with a varying number of entries per monster, and how monsters map onto it is not decoded yet.
//! Found by matching two monsters' published drop lists; checked on all three ranks. Offsets are for the US v32 executable.

use anyhow::{Result, bail};
use std::collections::HashMap;

/// Where monster 1's entry sits for each rank's first table (body carves); the tail and shiny tables follow `TABLE_STRIDE` apart.
const RANK_STARTS: [usize; 3] = [0x765e0, 0x76dc8, 0x775b0];
const TABLE_STRIDE: usize = 0x194;
/// Monsters 1 to 100 have entries.
const MONSTERS: u16 = 100;
const MAX_ITEM_ID: u16 = 1550;
const MAX_LIST: usize = 12;

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
}

impl Method {
    pub const ALL: [Method; 3] = [Method::BodyCarve, Method::TailCarve, Method::Shiny];

    pub fn label(self) -> &'static str {
        match self {
            Method::BodyCarve => "Body carve",
            Method::TailCarve => "Tail carve",
            Method::Shiny => "Shiny drops",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Drop {
    pub item: u16,
    /// Chance in percent.
    pub percent: u8,
}

#[derive(Debug, Default)]
pub struct Drops {
    lists: HashMap<(u16, Rank, Method), Vec<Drop>>,
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
        out.push(Drop { item, percent: rec[1] });
        at += 4;
    }
    (!out.is_empty() && out.iter().map(|d| u32::from(d.percent)).sum::<u32>() == 100).then_some(out)
}

/// Parse every carve table. `data_addr` is the virtual address of the data section (pointers are absolute).
pub fn parse(data: &[u8], data_addr: u32) -> Result<Drops> {
    let mut lists = HashMap::new();
    for (r, &start) in RANK_STARTS.iter().enumerate() {
        for (j, method) in Method::ALL.into_iter().enumerate() {
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
    Ok(Drops { lists })
}

impl Drops {
    pub fn list(&self, monster: u16, rank: Rank, method: Method) -> Option<&[Drop]> {
        self.lists.get(&(monster, rank, method)).map(Vec::as_slice)
    }

    /// Monster ids that have at least one drop list, ascending.
    pub fn monsters(&self) -> Vec<u16> {
        let mut ids: Vec<u16> = self.lists.keys().map(|&(m, ..)| m).collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// Every (monster, rank, method, chance) that can drop `item`, ordered by monster, then rank and method.
    pub fn sources(&self, item: u16) -> Vec<(u16, Rank, Method, u8)> {
        let mut out: Vec<(u16, Rank, Method, u8)> = self
            .lists
            .iter()
            .flat_map(|(&(m, r, me), list)| list.iter().filter(move |d| d.item == item).map(move |d| (m, r, me, d.percent)))
            .collect();
        out.sort_unstable();
        out
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
            Some(&[Drop { item: 101, percent: 100 }][..])
        );
        assert_eq!(d.list(100, Rank::G, Method::Shiny).unwrap()[0].item, 200);
        assert_eq!(d.monsters().len(), 100);
        assert_eq!(d.list(101, Rank::Low, Method::BodyCarve), None, "there is no monster 101");
    }

    #[test]
    fn finds_the_monsters_that_drop_an_item() {
        let d = parse(&section(), ADDR).unwrap();
        let sources = d.sources(105);
        assert_eq!(sources.len(), 9, "monster 5 in 3 ranks x 3 methods");
        assert!(sources.iter().all(|s| s.0 == 5 && s.3 == 100));
        assert_eq!(sources[0], (5, Rank::Low, Method::BodyCarve, 100));
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
        let scale: Vec<_> = d.sources(563).into_iter().filter(|s| s.0 == 1).collect();
        assert_eq!(
            scale,
            [
                (1, Rank::Low, Method::BodyCarve, 40),
                (1, Rank::Low, Method::TailCarve, 62),
                (1, Rank::Low, Method::Shiny, 15),
                (1, Rank::High, Method::TailCarve, 7),
            ]
        );
    }
}

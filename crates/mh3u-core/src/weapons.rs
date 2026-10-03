//! Weapon forging prices, read from the game executable's data section.
//!
//! Each weapon type has a table of stats records, one per weapon id (id 0 is a placeholder). Every record holds a price as a
//! big-endian u32: **upgrading a weapon costs exactly that price, and creating it from scratch costs 1.5 times it**. This
//! was found by matching 28 prices seen in play against the tables and every one agrees (see `docs/prices.md`).
//!
//! Nine types use 28-byte records (the price is 4 bytes in), the two bowguns and the bow use 100-byte records. The table
//! positions are for the US v32 executable and are checked on load.

use anyhow::{Result, bail};
use std::collections::HashMap;

/// (equipment kind, offset of weapon id 1's price field in the data section, record size, highest id).
const TABLES: &[(u8, usize, usize, u16)] = &[
    (7, 0x51dd8, 28, 135),   // great sword
    (8, 0x52cb8, 28, 140),   // sword & shield
    (9, 0x53c24, 28, 134),   // hammer
    (10, 0x58978, 28, 145),  // lance
    (11, 0x4bf08, 100, 91),  // heavy bowgun
    (13, 0x49924, 100, 84),  // light bowgun
    (14, 0x54ae8, 28, 114),  // long sword
    (15, 0x5577c, 28, 113),  // switch axe
    (16, 0x563f4, 28, 114),  // gunlance
    (17, 0x4e2f8, 100, 115), // bow
    (18, 0x57088, 28, 125),  // dual blades
    (19, 0x57e50, 28, 101),  // hunting horn
];

/// The most a stored price can plausibly be; anything above means the table isn't where we think it is.
const MAX_PRICE: u32 = 10_000_000;

/// How you obtain a weapon, which decides what it costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Create,
    Upgrade,
}

/// The stored price of every weapon, keyed by (equipment kind, weapon id). Weapons whose stored price is zero are left out.
pub fn parse(data: &[u8]) -> Result<HashMap<(u8, u16), u32>> {
    let mut out = HashMap::new();
    for &(kind, first, stride, max_id) in TABLES {
        let (mut seen, mut odd) = (0usize, 0usize);
        for id in 1..=max_id {
            let at = first + stride * (id as usize - 1);
            let Some(bytes) = data.get(at..at + 4) else {
                bail!("weapon price table for kind {kind} runs past the data section; unsupported executable?");
            };
            let price = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            if price == 0 {
                continue;
            }
            seen += 1;
            if price > MAX_PRICE || price % 10 != 0 {
                odd += 1;
            }
            out.insert((kind, id), price);
        }
        // Real prices are all multiples of 10; a table in the wrong place is noise.
        if seen < 20 || odd * 10 > seen {
            bail!("weapon price table for kind {kind} looks wrong ({odd} of {seen} prices implausible); unsupported executable?");
        }
    }
    Ok(out)
}

/// What it costs to get a weapon whose stored price is `stored`.
pub fn cost(stored: u32, via: Via) -> u32 {
    match via {
        Via::Upgrade => stored,
        Via::Create => stored / 2 * 3 + (stored % 2) * 3 / 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_costs_one_and_a_half_times_the_stored_price() {
        // pairs seen in play: (stored price, create cost)
        for (stored, created) in [(500, 750), (5000, 7500), (40000, 60000), (3830, 5745), (700, 1050), (2200, 3300)] {
            assert_eq!(cost(stored, Via::Create), created);
        }
        assert_eq!(cost(1500, Via::Upgrade), 1500);
        assert_eq!(cost(2000, Via::Upgrade), 2000);
    }

    /// A fake data section with plausible tables at the real positions.
    fn fake_section() -> Vec<u8> {
        let end = TABLES
            .iter()
            .map(|&(_, first, stride, max)| first + stride * max as usize + 4)
            .max()
            .unwrap();
        let mut data = vec![0u8; end];
        for &(kind, first, stride, max) in TABLES {
            for id in 1..=max {
                let at = first + stride * (id as usize - 1);
                let price = 500 + 10 * u32::from(id) + u32::from(kind) * 100_000;
                data[at..at + 4].copy_from_slice(&price.to_be_bytes());
            }
        }
        data
    }

    #[test]
    fn reads_every_table_by_id() {
        let prices = parse(&fake_section()).unwrap();
        assert_eq!(prices[&(7, 1)], 500 + 10 + 700_000);
        assert_eq!(prices[&(7, 135)], 500 + 1350 + 700_000);
        assert_eq!(prices[&(13, 84)], 500 + 840 + 1_300_000);
        assert_eq!(prices[&(17, 115)], 500 + 1150 + 1_700_000);
        assert_eq!(prices.len(), TABLES.iter().map(|t| t.3 as usize).sum::<usize>());
    }

    #[test]
    fn noise_in_place_of_a_table_is_rejected() {
        let mut data = fake_section();
        // scribble odd numbers over the great sword table
        for (n, byte) in data[0x51dd8..0x51dd8 + 28 * 135].iter_mut().enumerate() {
            *byte = (n * 37 + 11) as u8;
        }
        assert!(parse(&data).is_err());
        assert!(
            parse(&data[..0x51000]).is_err(),
            "a section that is too short is rejected, not read out of bounds"
        );
    }
}

#[cfg(test)]
mod real_data {
    use super::*;

    /// Checks the tables against the prices seen in play, when the extracted data section is around.
    #[test]
    fn matches_prices_seen_in_play() {
        let Ok(data) = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/live/rpx_data.bin")) else {
            return;
        };
        let p = parse(&data).unwrap();
        for (kind, id, via, cost_seen) in [
            (7, 1, Via::Create, 750),
            (7, 3, Via::Upgrade, 2000),
            (11, 27, Via::Create, 9000),
            (13, 18, Via::Create, 3300),
            (17, 6, Via::Create, 5745),
            (17, 34, Via::Upgrade, 2150),
            (19, 2, Via::Upgrade, 2200),
            (14, 49, Via::Create, 750),
        ] {
            assert_eq!(cost(p[&(kind, id)], via), cost_seen, "kind {kind} id {id}");
        }
    }
}

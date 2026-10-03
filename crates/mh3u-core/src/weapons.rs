//! Weapon stats and forging prices, read from the game executable's data section.
//!
//! Each weapon type has a table of stats records, one per weapon id (id 0 is a placeholder). Every record holds a price as a
//! big-endian u32: **upgrading a weapon costs exactly that price, and creating it from scratch costs 1.5 times it**. This
//! was found by matching 28 prices seen in play against the tables and every one agrees (see `docs/prices.md`).
//!
//! Nine types use 28-byte records, the two bowguns and the bow use 100-byte records. Around the price field the record also
//! holds the weapon's rarity, attack, affinity and gem slots (offsets below, relative to the price field). Attack is stored
//! as a base number that the game multiplies by a per-type factor for display. Sharpness and element are not decoded: they
//! are not in these records. The table positions are for the US v32 executable and are checked on load.

use anyhow::{Result, bail};
use std::collections::HashMap;

/// Where a table's fields sit, relative to a record's price field.
#[derive(Clone, Copy)]
struct Fields {
    rarity: isize,
    attack: isize,
    affinity: isize,
    slots: isize,
}

/// 28-byte records: rarity, then the price, then attack (u16), affinity (i8) and slots further on.
const MELEE: Fields = Fields {
    rarity: -2,
    attack: 6,
    affinity: 9,
    slots: 16,
};
/// 100-byte records: rarity and attack come before the price.
const RANGED: Fields = Fields {
    rarity: -10,
    attack: -8,
    affinity: 6,
    slots: 5,
};

struct Table {
    kind: u8,
    /// Offset of weapon id 1's price field in the data section.
    first: usize,
    stride: usize,
    max_id: u16,
    fields: Fields,
    /// Displayed attack is the stored number times this, in percent (rounded down).
    attack_percent: u32,
}

const fn table(kind: u8, first: usize, stride: usize, max_id: u16, fields: Fields, attack_percent: u32) -> Table {
    Table {
        kind,
        first,
        stride,
        max_id,
        fields,
        attack_percent,
    }
}

const TABLES: &[Table] = &[
    table(7, 0x51dd8, 28, 135, MELEE, 480),    // great sword
    table(8, 0x52cb8, 28, 140, MELEE, 140),    // sword & shield
    table(9, 0x53c24, 28, 134, MELEE, 520),    // hammer
    table(10, 0x58978, 28, 145, MELEE, 230),   // lance
    table(11, 0x4bf08, 100, 91, RANGED, 148),  // heavy bowgun
    table(13, 0x49924, 100, 84, RANGED, 130),  // light bowgun
    table(14, 0x54ae8, 28, 114, MELEE, 330),   // long sword
    table(15, 0x5577c, 28, 113, MELEE, 460),   // switch axe
    table(16, 0x563f4, 28, 114, MELEE, 230),   // gunlance
    table(17, 0x4e2f8, 100, 115, RANGED, 120), // bow
    table(18, 0x57088, 28, 125, MELEE, 140),   // dual blades
    table(19, 0x57e50, 28, 101, MELEE, 460),   // hunting horn
];

/// What the game data says about one weapon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weapon {
    /// The stored price; see [`cost`].
    pub price: u32,
    pub rarity: u8,
    /// The attack the game displays.
    pub attack: u32,
    /// Affinity in percent, negative for a penalty.
    pub affinity: i8,
    pub slots: u8,
}

/// The most a stored price can plausibly be; anything above means the table isn't where we think it is.
const MAX_PRICE: u32 = 10_000_000;

/// How you obtain a weapon, which decides what it costs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Via {
    Create,
    Upgrade,
}

/// Every weapon, keyed by (equipment kind, weapon id). Weapons whose stored price is zero are left out.
pub fn parse(data: &[u8]) -> Result<HashMap<(u8, u16), Weapon>> {
    let mut out = HashMap::new();
    for t in TABLES {
        let (mut seen, mut odd) = (0usize, 0usize);
        for id in 1..=t.max_id {
            let at = t.first + t.stride * (id as usize - 1);
            let get = |offset: isize, len: usize| -> Option<&[u8]> {
                let start = at.checked_add_signed(offset)?;
                data.get(start..start + len)
            };
            let f = t.fields;
            let (Some(bytes), Some(rarity), Some(attack), Some(affinity), Some(slots)) =
                (get(0, 4), get(f.rarity, 1), get(f.attack, 2), get(f.affinity, 1), get(f.slots, 1))
            else {
                bail!(
                    "weapon table for kind {} runs past the data section; unsupported executable?",
                    t.kind
                );
            };
            let (rarity, affinity, slots) = (rarity[0], affinity[0], slots[0]);
            let price = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            if price == 0 {
                continue;
            }
            seen += 1;
            if price > MAX_PRICE || price % 10 != 0 || rarity > 10 || slots > 3 {
                odd += 1;
            }
            let raw = u32::from(u16::from_be_bytes([attack[0], attack[1]]));
            out.insert(
                (t.kind, id),
                Weapon {
                    price,
                    rarity: rarity.saturating_add(1),
                    attack: raw * t.attack_percent / 100,
                    affinity: affinity as i8,
                    slots,
                },
            );
        }
        // Real prices are all multiples of 10; a table in the wrong place is noise.
        if seen < 20 || odd * 10 > seen {
            bail!(
                "weapon table for kind {} looks wrong ({odd} of {seen} records implausible); unsupported executable?",
                t.kind
            );
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
        let end = TABLES.iter().map(|t| t.first + t.stride * t.max_id as usize + 4).max().unwrap();
        let mut data = vec![0u8; end];
        for t in TABLES {
            for id in 1..=t.max_id {
                let at = t.first + t.stride * (id as usize - 1);
                let price = 500 + 10 * u32::from(id) + u32::from(t.kind) * 100_000;
                data[at..at + 4].copy_from_slice(&price.to_be_bytes());
            }
        }
        data
    }

    #[test]
    fn reads_every_table_by_id() {
        let prices = parse(&fake_section()).unwrap();
        assert_eq!(prices[&(7, 1)].price, 500 + 10 + 700_000);
        assert_eq!(prices[&(7, 135)].price, 500 + 1350 + 700_000);
        assert_eq!(prices[&(13, 84)].price, 500 + 840 + 1_300_000);
        assert_eq!(prices[&(17, 115)].price, 500 + 1150 + 1_700_000);
        assert_eq!(prices.len(), TABLES.iter().map(|t| t.max_id as usize).sum::<usize>());
    }

    #[test]
    fn reads_the_stats_around_the_price_in_both_record_layouts() {
        let mut data = fake_section();
        // great sword id 3: rarity byte before the price, then attack (x4.8), affinity and slots after it
        let gs = 0x51dd8 + 28 * 2;
        data[gs - 2] = 4;
        data[gs + 6..gs + 8].copy_from_slice(&90u16.to_be_bytes());
        data[gs + 9] = (-15i8) as u8;
        data[gs + 16] = 2;
        // light bowgun id 2: rarity and attack before the price, slots and affinity after it
        let lbg = 0x49924 + 100;
        data[lbg - 10] = 6;
        data[lbg - 8..lbg - 6].copy_from_slice(&120u16.to_be_bytes());
        data[lbg + 5] = 3;
        data[lbg + 6] = 10;
        let w = parse(&data).unwrap();
        assert_eq!(
            (w[&(7, 3)].rarity, w[&(7, 3)].attack, w[&(7, 3)].affinity, w[&(7, 3)].slots),
            (5, 432, -15, 2)
        );
        assert_eq!(
            (w[&(13, 2)].rarity, w[&(13, 2)].attack, w[&(13, 2)].affinity, w[&(13, 2)].slots),
            (7, 156, 10, 3)
        );
        assert_eq!(w[&(11, 1)].attack, 0, "an empty record has no attack");
    }

    #[test]
    fn the_heavy_bowgun_multiplier_rounds_down() {
        let mut data = fake_section();
        let hbg = 0x4bf08;
        data[hbg - 8..hbg - 6].copy_from_slice(&170u16.to_be_bytes());
        assert_eq!(parse(&data).unwrap()[&(11, 1)].attack, 251, "170 x 1.48 = 251.6");
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
            assert_eq!(cost(p[&(kind, id)].price, via), cost_seen, "kind {kind} id {id}");
        }
    }
    /// Stats decoded from the extracted data section. The numbers agree with a published weapon list (about 97% of all weapons
    /// do; the rest are weapons past the tables' ends and a few disagreements in that list), not yet with the game's own screens.
    #[test]
    fn decodes_stats_for_each_record_layout() {
        let Ok(data) = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/live/rpx_data.bin")) else {
            return;
        };
        let p = parse(&data).unwrap();
        // (kind, id, rarity, attack, affinity, slots)
        for (kind, id, rarity, attack, affinity, slots) in [
            (7, 1, 1, 336, 0, 0), // great sword
            (7, 6, 3, 672, 5, 1),
            (7, 14, 9, 1296, 0, 2),
            (8, 2, 1, 112, 0, 0),   // sword & shield
            (11, 27, 2, 207, 0, 1), // heavy bowgun (x1.48, rounded down)
            (13, 18, 1, 143, 0, 0), // light bowgun
            (17, 6, 2, 120, 0, 1),  // bow
            (19, 2, 1, 460, 0, 1),  // hunting horn
        ] {
            let w = p[&(kind, id)];
            assert_eq!(
                (w.rarity, w.attack, w.affinity, w.slots),
                (rarity, attack, affinity, slots),
                "kind {kind} id {id}"
            );
        }
    }
}

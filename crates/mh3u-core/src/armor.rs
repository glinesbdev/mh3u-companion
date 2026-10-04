//! Armor stats, read from the game executable's data section.
//!
//! One table per armor slot, each a run of 24-byte records indexed by piece id (the same ids as the
//! name tables). The tables are contiguous. Record layout:
//!
//! | bytes | field |
//! |-------|-------|
//! | 0     | base defense |
//! | 7     | rarity - 1 |
//! | 6     | bitfield: bits 0-1 gender (1 male, 2 female, 3 both), bits 2-3 class (1 blademaster, 2 gunner, 3 both) |
//! | 8..13 | resistances (i8): fire, water, thunder, ice, dragon |
//! | 13    | gem slots |
//! | 14..24| five (skill id, signed points) pairs; id 0 = empty |
//!
//! Bytes 1..6 (model ids, maximum defense data) are not decoded. Skill ids index the game's
//! skill-tree name table.
//!
//! Prices live in a second set of tables, also indexed by piece id, of 32-byte rows: a big-endian u16 at byte 14 holds
//! **half the zenny price** (a Jaggi piece costs 1,150 and stores 575). The same rows hold the upgrade-level data that
//! decides maximum defense (see [`max_defense`]). The price tables follow each other starting at [`PRICE_TABLES`]. This was found
//! by matching 966 armor prices from a published list (942 agree) and all 39 prices seen in play (38 agree; the other is a
//! hand-written note), see `docs/prices.md`.

use anyhow::{Result, bail};
use std::collections::HashMap;

const RECORD_LEN: usize = 24;

/// (equipment kind, byte offset in the data section, record count). Kinds match `GameData`.
const TABLES: &[(u8, usize, usize)] = &[
    (1, 0x178b8, 382), // body
    (2, 0x19c88, 363), // arms
    (3, 0x1be90, 371), // waist
    (4, 0x1e158, 377), // legs
    (5, 0x204b0, 380), // head
];

/// (equipment kind, byte offset of the 32-byte price rows in the data section). Same kinds and counts as [`TABLES`].
const PRICE_TABLES: &[(u8, usize)] = &[(1, 0x22958), (2, 0x25918), (3, 0x28678), (4, 0x2b4d8), (5, 0x2e3f8)];
const PRICE_ROW_LEN: usize = 32;
const PRICE_AT: usize = 14;
/// In a price row: byte 2 is 1 for the gunner version of a piece, and bytes 3 to 8 are six growth numbers.
const GUNNER_AT: usize = 2;
const GROWTH_AT: usize = 3;
/// The most an armor piece plausibly costs; anything above means the table isn't where we think it is.
const MAX_PRICE: u32 = 200_000;

/// The defense a piece has fully upgraded, from its base defense and its price row (`row[2]` the gunner flag, `row[3..9]` the growth
/// numbers g3 to g8). For a blademaster piece the gain over the base is `6*g7 + 2*g8 - g3 - g4 - g5 - 3*g6 - 2`, for a gunner piece
/// `3*g7 + g8 - g4 - g6 - 2`. These were fitted to a published list of 969 pieces: the blademaster formula is exact on all 56 distinct rows
/// of numbers, and the gunner formula on 51 of 53, and 950 of the 969 pieces agree (the other 19 are a few sets that are 1 or 2 away,
/// and one earring). `None` if the result would not be a plausible defense.
pub fn max_defense(base: u8, row: &[u8]) -> Option<u8> {
    let g: Vec<i32> = row.get(GROWTH_AT..GROWTH_AT + 6)?.iter().map(|&b| i32::from(b)).collect();
    let gain = if row.get(GUNNER_AT) == Some(&1) {
        3 * g[4] + g[5] - g[1] - g[3] - 2
    } else {
        6 * g[4] + 2 * g[5] - g[0] - g[1] - g[2] - 3 * g[3] - 2
    };
    u8::try_from(i32::from(base) + gain).ok().filter(|&m| m >= base)
}

/// Which hunters can wear a piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    Male,
    Female,
    Both,
}

impl Gender {
    pub fn label(self) -> &'static str {
        match self {
            Gender::Male => "Male",
            Gender::Female => "Female",
            Gender::Both => "Both",
        }
    }
}

/// Which weapon class a piece is for. Kiranico calls `Both` "All".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmorClass {
    Blademaster,
    Gunner,
    Both,
}

impl ArmorClass {
    pub fn label(self) -> &'static str {
        match self {
            ArmorClass::Blademaster => "Blademaster",
            ArmorClass::Gunner => "Gunner",
            ArmorClass::Both => "Both",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArmorStats {
    pub defense: u8,
    pub rarity: u8,
    pub slots: u8,
    /// `None` only if the record's flag bits hold an unknown value.
    pub gender: Option<Gender>,
    pub class: Option<ArmorClass>,
    /// Fire, water, thunder, ice, dragon.
    pub resist: [i8; 5],
    /// (skill id, points); points may be negative.
    pub skills: Vec<(u8, i8)>,
    /// What the forge charges to create the piece, in zenny. `None` when the table holds no price for it.
    pub price: Option<u32>,
    /// The defense when fully upgraded (see [`max_defense`]); `None` for a talisman or when the table gives no sensible number.
    pub max_defense: Option<u8>,
}

impl ArmorStats {
    /// What a talisman adds to a set: its skills and nothing else.
    pub fn talisman(skills: Vec<(u8, i8)>) -> ArmorStats {
        ArmorStats {
            defense: 0,
            rarity: 1,
            slots: 0,
            gender: None,
            class: None,
            resist: [0; 5],
            skills,
            price: None,
            max_defense: None,
        }
    }
}

fn parse_record(r: &[u8]) -> ArmorStats {
    ArmorStats {
        defense: r[0],
        rarity: r[7] + 1,
        slots: r[13],
        gender: match r[6] & 3 {
            1 => Some(Gender::Male),
            2 => Some(Gender::Female),
            3 => Some(Gender::Both),
            _ => None,
        },
        class: match (r[6] >> 2) & 3 {
            1 => Some(ArmorClass::Blademaster),
            2 => Some(ArmorClass::Gunner),
            3 => Some(ArmorClass::Both),
            _ => None,
        },
        resist: [r[8] as i8, r[9] as i8, r[10] as i8, r[11] as i8, r[12] as i8],
        skills: r[14..24]
            .as_chunks::<2>()
            .0
            .iter()
            .filter(|p| p[0] != 0)
            .map(|p| (p[0], p[1] as i8))
            .collect(),
        price: None,
        max_defense: None,
    }
}

/// Parse every armor table into a map keyed by (equipment kind, piece id).
pub fn parse(data: &[u8]) -> Result<HashMap<(u8, u16), ArmorStats>> {
    let mut out = HashMap::new();
    for &(kind, start, count) in TABLES {
        let Some(table) = data.get(start..start + count * RECORD_LEN) else {
            bail!("armor table for kind {kind} runs past the data section; unsupported executable?");
        };
        for (id, rec) in table.as_chunks::<RECORD_LEN>().0.iter().enumerate() {
            if rec[7] > 11 || rec[13] > 3 {
                bail!("implausible armor record for kind {kind} id {id}; unsupported executable?");
            }
            out.insert((kind, id as u16), parse_record(rec));
        }
    }
    for (&(kind, start), &(_, _, count)) in PRICE_TABLES.iter().zip(TABLES) {
        let Some(rows) = data.get(start..start + count * PRICE_ROW_LEN) else {
            bail!("armor price table for kind {kind} runs past the data section; unsupported executable?");
        };
        let (mut seen, mut odd) = (0usize, 0usize);
        for (id, row) in rows.as_chunks::<PRICE_ROW_LEN>().0.iter().enumerate() {
            if let Some(stats) = out.get_mut(&(kind, id as u16)) {
                stats.max_defense = max_defense(stats.defense, row);
            }
            let price = u32::from(u16::from_be_bytes([row[PRICE_AT], row[PRICE_AT + 1]])) * 2;
            if price == 0 {
                continue;
            }
            seen += 1;
            // real prices are all multiples of 50
            if price > MAX_PRICE || price % 50 != 0 {
                odd += 1;
            }
            if let Some(stats) = out.get_mut(&(kind, id as u16)) {
                stats.price = Some(price);
            }
        }
        if seen < 20 || odd * 10 > seen {
            bail!("armor price table for kind {kind} looks wrong ({odd} of {seen} prices implausible); unsupported executable?");
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maximum_defense_follows_the_growth_numbers_by_version() {
        // A blademaster row (flag 0) and a gunner row (flag 1) with the same numbers as Hunter's Helm and Hunter's Cap, whose fully upgraded
        // defense is 100 and 55 in the game's published lists.
        let row = |gunner: u8, last: u8| {
            let mut r = [0u8; 32];
            r[2] = gunner;
            r[3..9].copy_from_slice(&[3, 5, 8, 10, 14, last]);
            r
        };
        assert_eq!(max_defense(6, &row(0, 0x1d)), Some(100));
        assert_eq!(max_defense(3, &row(1, 0x1b)), Some(55));
        assert_eq!(max_defense(1, &row(1, 27)), Some(53), "Leather Headgear");
        assert_eq!(max_defense(5, &[0; 4]), None, "a row too short");
        assert_eq!(max_defense(200, &row(0, 255)), None, "does not fit a byte");
    }

    #[test]
    fn parses_dianthus_wealskirt_record() {
        // Verified in-game: defense 76, rarity 9, no slots, no resistances,
        // Poison -2, Wide-Range +4, Perception +2, Psychic +5, FreeElemnt +1.
        let rec = [
            0x4c, 0x63, 0x00, 0x86, 0x02, 0x23, 0x06, 0x08, 0, 0, 0, 0, 0, 0, 0x52, 0x04, 0x63, 0x05, 0x62, 0x02, 0x72, 0x01, 0x02, 0xfe,
        ];
        let s = parse_record(&rec);
        assert_eq!((s.defense, s.rarity, s.slots, s.resist), (76, 9, 0, [0; 5]));
        assert_eq!(s.skills, vec![(82, 4), (99, 5), (98, 2), (114, 1), (2, -2)]);
        assert_eq!((s.gender, s.class), (Some(Gender::Female), Some(ArmorClass::Blademaster)));
    }

    #[test]
    fn decodes_every_gender_and_class_flag_combination() {
        // Byte 6 values seen in the game, with the labels Kiranico gives those pieces.
        let cases = [
            (0x05, Gender::Male, ArmorClass::Blademaster),
            (0x06, Gender::Female, ArmorClass::Blademaster),
            (0x07, Gender::Both, ArmorClass::Blademaster),
            (0x09, Gender::Male, ArmorClass::Gunner),
            (0x0a, Gender::Female, ArmorClass::Gunner),
            (0x0b, Gender::Both, ArmorClass::Gunner),
            (0x0d, Gender::Male, ArmorClass::Both),
            (0x0e, Gender::Female, ArmorClass::Both),
            (0x0f, Gender::Both, ArmorClass::Both),
        ];
        for (byte, gender, class) in cases {
            let mut rec = [0u8; 24];
            rec[6] = byte;
            let s = parse_record(&rec);
            assert_eq!((s.gender, s.class), (Some(gender), Some(class)), "byte 6 = {byte:#04x}");
        }
        assert_eq!(parse_record(&[0u8; 24]).gender, None);
    }

    #[test]
    fn parses_negative_resistance() {
        // Gargwa Mask: defense 20, rarity 5, resist fire -4 water -2 thunder -1 ice +2 dragon +1, Combo Rate +10.
        let rec = [
            0x14, 0x00, 0x00, 0x00, 0x02, 0x00, 0x0f, 0x04, 0xfc, 0xfe, 0xff, 0x02, 0x01, 0x00, 88, 10, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let s = parse_record(&rec);
        assert_eq!(s.resist, [-4, -2, -1, 2, 1]);
        assert_eq!(s.skills, vec![(88, 10)]);
        assert_eq!((s.gender, s.class), (Some(Gender::Both), Some(ArmorClass::Both)));
    }

    /// A data section with plausible price rows at the real positions: piece `id` of every kind stores `id * 25 + 25`
    /// (so it costs `id * 50 + 50`).
    fn fake_section_with_prices() -> Vec<u8> {
        let end = PRICE_TABLES
            .iter()
            .zip(TABLES)
            .map(|(&(_, p), &(_, _, count))| p + count * PRICE_ROW_LEN)
            .max()
            .unwrap();
        let mut data = vec![0u8; end];
        for (&(_, start), &(_, _, count)) in PRICE_TABLES.iter().zip(TABLES) {
            for id in 0..count {
                let at = start + id * PRICE_ROW_LEN + PRICE_AT;
                data[at..at + 2].copy_from_slice(&((id as u16) * 25 + 25).to_be_bytes());
            }
        }
        data
    }

    #[test]
    fn prices_are_twice_the_stored_half() {
        let data = fake_section_with_prices();
        let armor = parse(&data).unwrap();
        assert_eq!(armor[&(4, 11)].price, Some(600), "id 11 stores 300");
        assert_eq!(armor[&(5, 0)].price, Some(50));
        assert_eq!(armor[&(1, 381)].price, Some(381 * 50 + 50));
    }

    #[test]
    fn a_price_table_in_the_wrong_place_is_rejected() {
        let mut data = fake_section_with_prices();
        // odd numbers where the head prices should be
        let start = PRICE_TABLES[4].1;
        for (n, byte) in data[start..start + 380 * PRICE_ROW_LEN].iter_mut().enumerate() {
            *byte = (n * 37 + 11) as u8;
        }
        assert!(parse(&data).is_err());
        assert!(parse(&data[..0x22000]).is_err(), "a section that is too short is rejected");
    }
}

#[cfg(test)]
mod real_data {
    use super::*;

    /// Prices seen in play (and their set mates), against the extracted data section when it is around.
    #[test]
    fn matches_prices_seen_in_play() {
        let Ok(data) = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/live/rpx_data.bin")) else {
            return;
        };
        let armor = parse(&data).unwrap();
        // (kind, id, price seen in the forge list)
        for (kind, id, price) in [
            (1, 2, 200),   // Chainmail Vest
            (1, 4, 450),   // Hunter's Mail
            (1, 8, 750),   // Alloy Mail
            (4, 11, 1150), // Jaggi Greaves
            (4, 12, 1150), // Jaggi Leggings
            (5, 9, 750),   // Alloy Cap
        ] {
            assert_eq!(armor[&(kind, id)].price, Some(price), "kind {kind} id {id}");
        }
    }
}

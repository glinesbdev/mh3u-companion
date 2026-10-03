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
//! skill-tree name table. Zenny prices are not stored here.

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
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

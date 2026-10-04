//! Decorations (jewels): the table in the executable that says what each one does.
//!
//! A run of 12-byte records in the data section, from [`TABLE_AT`], 202 of them (a few are DUMMY items). A record is:
//!
//! | bytes | field |
//! |-------|-------|
//! | 0     | the skill it takes points from (a penalty), 0 for none |
//! | 1     | that penalty as a signed byte (0xff = -1) |
//! | 2     | slots it needs: 1, 2 or 3 |
//! | 4-5   | the item id of the jewel (u16) |
//! | 6-9   | a u32, 10 for 1-slot jewels, 20 for 2-slot and 30 for 3-slot (the sell price) |
//! | 10    | the skill it adds |
//! | 11    | the points it adds (signed) |
//!
//! The record's position in the table, counting from 1, is the number a save keeps when a decoration is socketed: a Tenderizer Jwl 1
//! (record 144, counting from 0) socketed in a talisman showed as `0x91` = 145 in the save.

use anyhow::{Result, bail};

/// Byte offset of the first record in the data section.
const TABLE_AT: usize = 0x7fe;
const RECORD_LEN: usize = 12;
const RECORDS: usize = 202;
/// Items with this id or a higher are not decorations, and the table is checked against that.
const MAX_ITEM_ID: u16 = 1600;

/// What one decoration does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decoration {
    /// The decoration's item id.
    pub item: u16,
    /// Gem slots it takes.
    pub slots: u8,
    /// The skill it adds and how many points.
    pub skill: u8,
    pub points: i8,
    /// The skill it costs points of, if any.
    pub penalty: Option<(u8, i8)>,
}

/// The table, indexed by the number the save keeps minus one.
pub fn parse(data: &[u8]) -> Result<Vec<Decoration>> {
    let Some(table) = data.get(TABLE_AT..TABLE_AT + RECORDS * RECORD_LEN) else {
        bail!("the decoration table runs past the data section; unsupported executable?");
    };
    let all: Vec<Decoration> = table
        .as_chunks::<RECORD_LEN>()
        .0
        .iter()
        .map(|r| Decoration {
            item: u16::from_be_bytes([r[4], r[5]]),
            slots: r[2],
            skill: r[10],
            points: r[11] as i8,
            penalty: (r[0] != 0).then_some((r[0], r[1] as i8)),
        })
        .collect();
    // DUMMY entries have no skill; the rest are jewels of 1 to 3 slots with item ids in the jewel range
    let real = all.iter().filter(|d| d.skill != 0).count();
    if real < 150 || all.iter().any(|d| d.item < 1000 || d.item >= MAX_ITEM_ID) || all.iter().any(|d| d.slots > 3) {
        bail!("the decoration table looks wrong ({real} decorations); unsupported executable?");
    }
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_with(records: &[(usize, [u8; RECORD_LEN])]) -> Vec<u8> {
        let mut d = vec![0u8; TABLE_AT + RECORDS * RECORD_LEN];
        for i in 0..RECORDS {
            let at = TABLE_AT + i * RECORD_LEN;
            d[at + 2] = 1;
            d[at + 4..at + 6].copy_from_slice(&1100u16.to_be_bytes());
            d[at + 10] = 5;
            d[at + 11] = 1;
        }
        for &(i, r) in records {
            d[TABLE_AT + i * RECORD_LEN..TABLE_AT + (i + 1) * RECORD_LEN].copy_from_slice(&r);
        }
        d
    }

    #[test]
    fn a_record_gives_the_skill_the_points_and_the_penalty() {
        // Tenderizer Jwl 3: takes a point of skill 13, gives 4 of skill 19, needs 3 slots
        let d = data_with(&[(145, [0x0d, 0xff, 3, 0, 0x05, 0xb6, 0, 0, 0, 0x1e, 0x13, 4])]);
        let t = parse(&d).unwrap();
        assert_eq!(t.len(), RECORDS);
        assert_eq!(
            t[145],
            Decoration {
                item: 1462,
                slots: 3,
                skill: 0x13,
                points: 4,
                penalty: Some((13, -1))
            }
        );
        assert_eq!(t[0].penalty, None);
    }

    #[test]
    fn a_table_that_is_not_one_is_refused() {
        assert!(parse(&[0u8; 100]).is_err());
        assert!(parse(&vec![0u8; TABLE_AT + RECORDS * RECORD_LEN]).is_err(), "no decorations");
    }
}

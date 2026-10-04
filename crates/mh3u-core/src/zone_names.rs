//! Names of monsters' hit zones ("Head", "Front Legs", "Tail").
//!
//! The game's hit-zone tables (see `hitzones.rs`) carry no names, only rows of numbers. `data/zone_names.tsv` is taken from Kiranico's
//! Monster Hunter 3 Ultimate database: each row of a monster's first table was matched to the database's zone with the same eight
//! numbers (328 of 361 rows; the rest, mostly zones of other states, stay numbered). Where two zones of a monster have the same eight
//! numbers, the names are given out in the database's order. A name is only used when the monster's name and the row's numbers are
//! the game's (see `GameData::zone_name`).

use anyhow::{Context, Result, bail};
use std::collections::HashMap;

const TABLE: &str = include_str!("../data/zone_names.tsv");

/// A zone's name with the monster and numbers it was matched on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub monster: String,
    /// cut, impact, shot, fire, water, ice, thunder, dragon.
    pub values: [u8; 8],
    pub name: String,
}

/// The table by (monster id, row).
pub fn parse() -> Result<HashMap<(u16, usize), Entry>> {
    parse_text(TABLE)
}

fn parse_text(text: &str) -> Result<HashMap<(u16, usize), Entry>> {
    let mut out = HashMap::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 12 {
            bail!("zone_names.tsv: a line has {} fields, not 12: {line}", f.len());
        }
        let values: Vec<u8> = f[3..11].iter().map(|v| v.parse().context("zone number")).collect::<Result<_>>()?;
        out.insert(
            (f[0].parse()?, f[2].parse()?),
            Entry {
                monster: f[1].to_string(),
                values: values.try_into().map_err(|_| anyhow::anyhow!("eight numbers"))?,
                name: f[11].to_string(),
            },
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_reads() {
        let t = parse().unwrap();
        assert!(t.len() > 300);
        let rathian_head = &t[&(1, 0)];
        assert_eq!((rathian_head.monster.as_str(), rathian_head.name.as_str()), ("Rathian", "Head"));
        assert_eq!(rathian_head.values, [90, 80, 70, 0, 15, 15, 20, 35]);
    }

    #[test]
    fn bad_lines_are_errors() {
        assert!(parse_text("1\tRathian\t0\t1\t2").is_err());
        assert!(parse_text("1\tRathian\t0\t1\t2\t3\t4\t5\t6\t7\tx\tHead").is_err());
    }
}

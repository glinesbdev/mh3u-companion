//! Carry limit and shop price of items.
//!
//! The game's item table has the price a shop pays for an item (see `items.rs`), but the carry limit and the price a shop asks are not
//! in anything the program reads (byte 7 of the item record agrees with the real limit for only 87% of items). `data/item_extras.tsv`
//! is taken from Kiranico's Monster Hunter 3 Ultimate database and joined to the game's items by name (1,293 of its 1,331 items; the 38 left
//! are not items in the game's text). On load, a line whose name is not the game's name for that item id is dropped.

use anyhow::{Context, Result, bail};
use std::collections::HashMap;

const TABLE: &str = include_str!("../data/item_extras.tsv");

/// What is known of one item besides its sell price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemExtras {
    /// The most of it the hunter can carry in one stack.
    pub carry: Option<u16>,
    /// What a shop asks for it, if some shop sells it.
    pub buy: Option<u32>,
}

/// The table as item id -> (name, extras).
pub fn parse() -> Result<HashMap<u16, (String, ItemExtras)>> {
    parse_text(TABLE)
}

fn number<T: std::str::FromStr>(text: &str) -> Result<Option<T>>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    if text.is_empty() {
        Ok(None)
    } else {
        Ok(Some(text.parse().with_context(|| format!("number {text}"))?))
    }
}

fn parse_text(text: &str) -> Result<HashMap<u16, (String, ItemExtras)>> {
    let mut out = HashMap::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 4 {
            bail!("item_extras.tsv: a line has {} fields, not 4: {line}", f.len());
        }
        out.insert(
            f[0].parse()?,
            (
                f[1].to_string(),
                ItemExtras {
                    carry: number(f[2])?,
                    buy: number(f[3])?,
                },
            ),
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
        assert!(t.len() > 1250);
        let potion = t.values().find(|(n, _)| n == "Potion").unwrap();
        assert_eq!(
            potion.1,
            ItemExtras {
                carry: Some(10),
                buy: Some(66)
            }
        );
        assert!(t.values().any(|(_, e)| e.buy.is_none()));
        assert!(t.values().all(|(_, e)| e.carry.is_none_or(|c| c > 0 && c <= 99)));
    }

    #[test]
    fn bad_lines_are_errors() {
        assert!(parse_text("8\tPotion\t10").is_err());
        assert!(parse_text("8\tPotion\tten\t66").is_err());
    }
}

//! What the game's item table says about each item: the price a shop pays for it.
//!
//! One table in the executable's data section, a run of 20-byte records indexed by item id (the ids of the name table), from
//! [`TABLE_AT`]. A big-endian u32 at the start of a record is the **sell price** in zenny. All 681 items whose price a published list
//! gives agree. Byte 7 of the record looks like the carry limit (99 for most things, 10 for others) but agrees with a published list for
//! only 87% of items, so it is not read; the carry limit and the shop price come from `item_extras.rs`.

use anyhow::{Result, bail};

/// Byte offset of the first record in the data section.
const TABLE_AT: usize = 0x1188;
const RECORD_LEN: usize = 20;
/// The records run to item 1532; the ids after that are dummy items and the bytes there are other data.
const RECORDS: usize = 1533;
/// No real item sells for more.
const MAX_SELL: u32 = 1_000_000;

/// Sell prices by item id (0 where the game gives the item no value).
pub fn parse_sell_prices(data: &[u8]) -> Result<Vec<u32>> {
    let Some(table) = data.get(TABLE_AT..TABLE_AT + RECORDS * RECORD_LEN) else {
        bail!("the item table runs past the data section; unsupported executable?");
    };
    let prices: Vec<u32> = table
        .as_chunks::<RECORD_LEN>()
        .0
        .iter()
        .map(|r| u32::from_be_bytes([r[0], r[1], r[2], r[3]]))
        .collect();
    let odd = prices.iter().filter(|&&p| p > MAX_SELL).count();
    let priced = prices.iter().filter(|&&p| p > 0).count();
    if priced < 1000 || odd > 5 {
        bail!("the item table looks wrong ({priced} priced items, {odd} implausible prices); unsupported executable?");
    }
    Ok(prices.into_iter().map(|p| if p > MAX_SELL { 0 } else { p }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_with(prices: &[(usize, u32)]) -> Vec<u8> {
        let mut d = vec![0u8; TABLE_AT + RECORDS * RECORD_LEN];
        // make the table look like one: most items have a price
        for id in 0..RECORDS {
            let at = TABLE_AT + id * RECORD_LEN;
            d[at..at + 4].copy_from_slice(&10u32.to_be_bytes());
        }
        for &(id, price) in prices {
            let at = TABLE_AT + id * RECORD_LEN;
            d[at..at + 4].copy_from_slice(&price.to_be_bytes());
        }
        d
    }

    #[test]
    fn a_price_is_the_first_four_bytes_of_the_items_record() {
        let p = parse_sell_prices(&data_with(&[(573, 4850), (9, 5)])).unwrap();
        assert_eq!((p[573], p[9], p.len()), (4850, 5, RECORDS));
    }

    #[test]
    fn an_implausible_price_counts_as_none_and_a_wrong_table_is_refused() {
        let p = parse_sell_prices(&data_with(&[(5, 4_000_000_000)])).unwrap();
        assert_eq!(p[5], 0);
        assert!(
            parse_sell_prices(&vec![0u8; TABLE_AT + RECORDS * RECORD_LEN]).is_err(),
            "no prices at all"
        );
        assert!(parse_sell_prices(&[0u8; 100]).is_err(), "too short");
    }
}

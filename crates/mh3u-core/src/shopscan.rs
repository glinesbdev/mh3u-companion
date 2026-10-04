//! Looking for the blacksmith's list of pieces in the emulator's memory (the `scan` debug command).
//!
//! Where the game keeps the list of pieces on offer is not known. If it keeps one, it is probably an array of piece ids, perhaps with
//! other fields between them, so this finds runs of big-endian u16 values that are all in a given set of ids (the pieces we expect
//! on offer), strictly increasing, at one fixed distance apart. The result shows where our idea of the list and the game's differ.

use crate::procmem::{CHUNK_LEN, ProcMem};
use std::{collections::HashSet, io};

/// Distances between the ids to try, in bytes.
pub const STRIDES: [usize; 8] = [2, 4, 6, 8, 12, 16, 20, 24];
/// The same for ids stored in one byte.
pub const BYTE_STRIDES: [usize; 6] = [1, 2, 4, 8, 12, 16];

/// A run of ids found in a block of memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// Offset of the first id in the block.
    pub offset: usize,
    pub stride: usize,
    pub ids: Vec<u16>,
}

/// How a run's ids are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Strictly increasing, like a list sorted by id.
    Increasing,
    /// Each id once, in any order, with at least one step down (the in-game menu may group pieces by set, not by id). Runs that only
    /// go up are the `Increasing` kind.
    Shuffled,
}

/// Every run of at least `min_len` ids from `wanted`, `stride` bytes apart, starting at an even offset below `start_below`. A run is as
/// long as it can be (it is not reported again from inside).
pub fn runs(data: &[u8], wanted: &HashSet<u16>, stride: usize, min_len: usize, start_below: usize, order: Order) -> Vec<Run> {
    runs_of_width(data, wanted, stride, min_len, start_below, order, 2)
}

/// Like [`runs`], with the ids stored in `width` bytes (1 or 2).
pub fn runs_of_width(
    data: &[u8],
    wanted: &HashSet<u16>,
    stride: usize,
    min_len: usize,
    start_below: usize,
    order: Order,
    width: usize,
) -> Vec<Run> {
    let at = |o: usize| -> Option<u16> {
        let v = if width == 1 {
            u16::from(*data.get(o)?)
        } else {
            u16::from_be_bytes([*data.get(o)?, *data.get(o + 1)?])
        };
        wanted.contains(&v).then_some(v)
    };
    let mut out = Vec::new();
    let mut o = 0;
    while o < data.len().min(start_below) {
        if let Some(first) = at(o) {
            let continues_one = match order {
                Order::Increasing => o >= stride && at(o - stride).is_some_and(|p| p < first),
                Order::Shuffled => o >= stride && at(o - stride).is_some(),
            };
            if !continues_one {
                let mut ids = vec![first];
                let mut p = o + stride;
                while let Some(v) = at(p) {
                    let fits = match order {
                        Order::Increasing => v > *ids.last().unwrap_or(&0),
                        Order::Shuffled => !ids.contains(&v),
                    };
                    if !fits {
                        break;
                    }
                    ids.push(v);
                    p += stride;
                }
                let steps_down = ids.windows(2).any(|w| w[1] < w[0]);
                if ids.len() >= min_len && (order == Order::Increasing || steps_down) {
                    out.push(Run { offset: o, stride, ids });
                }
            }
        }
        o += width;
    }
    out
}

/// A run's value as a find: more of the wanted ids is better, and ids that are not simply consecutive count for more (a counting table
/// matches anything contiguous).
pub fn score(run: &Run) -> usize {
    let gaps = run.ids.windows(2).filter(|w| w[1] != w[0] + 1).count();
    let steps_down = run.ids.windows(2).filter(|w| w[1] < w[0]).count();
    run.ids.len() * 4 + gaps * 3 + steps_down * 6
}

/// A run and where it is in the emulator's memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub host: u64,
    pub run: Run,
}

/// Search all of the emulator's writable memory. The best `keep` finds, best first.
pub fn scan(mem: &ProcMem, wanted: &HashSet<u16>, min_len: usize, keep: usize) -> io::Result<Vec<Found>> {
    let mut found = Vec::new();
    mem.for_each_chunk(4096, |addr, data| {
        for (width, strides) in [(2, &STRIDES[..]), (1, &BYTE_STRIDES[..])] {
            for &stride in strides {
                for order in [Order::Increasing, Order::Shuffled] {
                    for run in runs_of_width(data, wanted, stride, min_len, CHUNK_LEN, order, width) {
                        found.push(Found {
                            host: addr + run.offset as u64,
                            run,
                        });
                    }
                }
            }
        }
    })?;
    found.sort_by_key(|f| std::cmp::Reverse(score(&f.run)));
    found.truncate(keep);
    Ok(found)
}

/// How a row of flags (one per piece) could be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// One byte per flag, zero for off, anything else for on.
    Bytes,
    /// A big-endian u16 per flag.
    Words,
    /// A big-endian u32 per flag.
    Dwords,
    /// Packed bits, the first flag in the top bit of the first byte.
    BitsMsb,
    /// Packed bits, the first flag in the lowest bit of the first byte.
    BitsLsb,
    /// Packed in big-endian u32 words, the first flag in the lowest bit of the first word.
    BitsWord,
}

pub const LAYOUTS: [Layout; 6] = [
    Layout::Bytes,
    Layout::Words,
    Layout::Dwords,
    Layout::BitsMsb,
    Layout::BitsLsb,
    Layout::BitsWord,
];

impl Layout {
    /// The indices below `count` whose flag is set, reading until the data stops being clean flags (so a bigger array than the pattern
    /// shows what the game flags beyond it).
    pub fn set_flags(self, data: &[u8], count: usize) -> Vec<usize> {
        (0..count)
            .map_while(|i| self.flag(data, i).map(|f| (i, f)))
            .filter(|&(_, f)| f)
            .map(|(i, _)| i)
            .collect()
    }

    /// Flag `i` of an array starting at `data[0]`; `None` if it is not a clean flag (a value other than 0 or 1) or past the end.
    fn flag(self, data: &[u8], i: usize) -> Option<bool> {
        let value = |width: usize| -> Option<bool> {
            let at = i * width;
            // any non-zero value is "set": the game may keep a state (1 new, 2 seen...) and not just a yes or no
            Some(data.get(at..at + width)?.iter().any(|&b| b != 0))
        };
        match self {
            Layout::Bytes => value(1),
            Layout::Words => value(2),
            Layout::Dwords => value(4),
            Layout::BitsMsb => Some(data.get(i / 8)? >> (7 - i % 8) & 1 == 1),
            Layout::BitsLsb => Some(data.get(i / 8)? >> (i % 8) & 1 == 1),
            Layout::BitsWord => Some(data.get((i / 32) * 4 + 3 - (i % 32) / 8)? >> (i % 8) & 1 == 1),
        }
    }
}

/// A place where the flags of a pattern are, with at most `max_miss` flags different.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagHit {
    pub host: u64,
    pub layout: Layout,
    pub misses: usize,
    /// Which pattern (by position in the list given to `scan_flags`).
    pub pattern: usize,
}

/// Every offset in `data` (below `start_below`) where the flags of `pattern` are found in `layout`, with up to `max_miss` different.
pub fn flag_matches(data: &[u8], pattern: &[bool], layout: Layout, max_miss: usize, start_below: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    // A cheap test that rules out nearly every offset before the full comparison. For flags stored as numbers: the first flag that
    // is set must be non-zero. For packed bits: the first byte may differ from the pattern's first eight flags by at most `max_miss` bits.
    let anchor = pattern.iter().position(|&f| f);
    let first_byte = (pattern.len() >= 8).then(|| {
        let bits = |i: usize| u8::from(pattern[i]);
        match layout {
            Layout::BitsMsb => (0, (0..8).fold(0u8, |b, i| b | bits(i) << (7 - i))),
            _ => (
                if layout == Layout::BitsWord { 3 } else { 0 },
                (0..8).fold(0u8, |b, i| b | bits(i) << i),
            ),
        }
    });
    let width = match layout {
        Layout::Words => 2,
        Layout::Dwords => 4,
        _ => 1,
    };
    for o in 0..data.len().min(start_below) {
        let plausible = match layout {
            Layout::Bytes | Layout::Words | Layout::Dwords => anchor.is_none_or(|a| {
                data.get(o + a * width..o + (a + 1) * width)
                    .is_some_and(|c| c.iter().any(|&b| b != 0))
            }),
            _ => first_byte.is_none_or(|(at, want)| data.get(o + at).is_some_and(|&b| ((b ^ want).count_ones() as usize) <= max_miss)),
        };
        if !plausible {
            continue;
        }
        let mut misses = 0;
        let mut ok = true;
        for (i, &want) in pattern.iter().enumerate() {
            match layout.flag(&data[o..], i) {
                Some(got) if got == want => {}
                Some(_) => {
                    misses += 1;
                    if misses > max_miss {
                        ok = false;
                        break;
                    }
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            out.push((o, misses));
        }
    }
    out
}

/// Search the emulator's memory for any of the flag patterns in any layout. Best (fewest misses) first, at most `keep`.
pub fn scan_flags(mem: &ProcMem, patterns: &[Vec<bool>], max_miss: usize, keep: usize) -> io::Result<Vec<FlagHit>> {
    let mut hits = Vec::new();
    mem.for_each_chunk(4096, |addr, data| {
        for (pattern, flags) in patterns.iter().enumerate() {
            for layout in LAYOUTS {
                for (o, misses) in flag_matches(data, flags, layout, max_miss, CHUNK_LEN) {
                    hits.push(FlagHit {
                        host: addr + o as u64,
                        layout,
                        misses,
                        pattern,
                    });
                }
            }
        }
    })?;
    hits.sort_by_key(|h| (h.misses, h.host));
    hits.truncate(keep);
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn be(ids: &[u16], stride: usize) -> Vec<u8> {
        let mut d = vec![0u8; ids.len() * stride + 8];
        for (i, id) in ids.iter().enumerate() {
            d[4 + i * stride..6 + i * stride].copy_from_slice(&id.to_be_bytes());
        }
        d
    }

    #[test]
    fn finds_an_increasing_run_at_its_stride() {
        let wanted: HashSet<u16> = [3, 5, 9, 12, 20].into();
        for stride in [2, 4, 12] {
            let d = be(&[3, 5, 9, 12, 20], stride);
            let found = runs(&d, &wanted, stride, 4, d.len(), Order::Increasing);
            assert_eq!(
                found,
                vec![Run {
                    offset: 4,
                    stride,
                    ids: vec![3, 5, 9, 12, 20]
                }],
                "stride {stride}"
            );
        }
    }

    #[test]
    fn short_runs_unwanted_values_and_decreasing_ones_do_not_count() {
        let wanted: HashSet<u16> = [3, 5, 9, 12].into();
        assert!(runs(&be(&[3, 5], 2), &wanted, 2, 3, 100, Order::Increasing).is_empty());
        assert!(
            runs(&be(&[3, 5, 7, 9], 2), &wanted, 2, 4, 100, Order::Increasing).is_empty(),
            "7 is not wanted"
        );
        assert!(
            runs(&be(&[12, 9, 5, 3], 2), &wanted, 2, 2, 100, Order::Increasing).is_empty(),
            "decreasing"
        );
    }

    #[test]
    fn a_run_is_reported_once_from_its_start() {
        let wanted: HashSet<u16> = [1, 2, 3, 4, 5].into();
        let d = be(&[1, 2, 3, 4, 5], 2);
        assert_eq!(runs(&d, &wanted, 2, 2, d.len(), Order::Increasing).len(), 1);
    }

    #[test]
    fn ids_stored_in_single_bytes_are_found() {
        let wanted: HashSet<u16> = [3, 5, 9, 12, 20].into();
        let d = [0xffu8, 3, 5, 9, 12, 20, 0xff];
        let found = runs_of_width(&d, &wanted, 1, 4, d.len(), Order::Increasing, 1);
        assert_eq!(
            found,
            vec![Run {
                offset: 1,
                stride: 1,
                ids: vec![3, 5, 9, 12, 20]
            }]
        );
    }

    #[test]
    fn a_shuffled_run_is_found_when_the_order_is_not_by_id() {
        let wanted: HashSet<u16> = [3, 5, 9, 12, 20].into();
        let d = be(&[9, 3, 12, 5, 20], 4);
        assert!(runs(&d, &wanted, 4, 4, d.len(), Order::Increasing).is_empty());
        let found = runs(&d, &wanted, 4, 4, d.len(), Order::Shuffled);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].ids, vec![9, 3, 12, 5, 20]);
        // an id repeated ends the run, and a run that only goes up is not "shuffled"
        let up = be(&[3, 5, 9, 12], 4);
        assert!(runs(&up, &wanted, 4, 4, up.len(), Order::Shuffled).is_empty());
    }

    #[test]
    fn flags_are_found_in_each_layout() {
        let pattern = [true, false, true, true, false, false, true, false, true, true];
        let at = |bytes: &[u8]| {
            let mut d = vec![0xffu8; 3];
            d.extend_from_slice(bytes);
            d.extend_from_slice(&[0xff; 3]);
            d
        };
        let per = |w: usize| -> Vec<u8> {
            pattern
                .iter()
                .flat_map(|&f| (0..w).map(move |k| u8::from(f && k == w - 1)))
                .collect()
        };
        for (layout, w) in [(Layout::Bytes, 1), (Layout::Words, 2), (Layout::Dwords, 4)] {
            let d = at(&per(w));
            // a wider number also matches a byte later (any non-zero byte counts), so only check that it is found
            assert!(flag_matches(&d, &pattern, layout, 0, d.len()).contains(&(3, 0)), "{layout:?}");
        }
        // bits: 1011 0010 11 -> MSB first 0xb2 0xc0; LSB first 0x4d 0x03
        let d = at(&[0xb2, 0xc0]);
        assert_eq!(flag_matches(&d, &pattern, Layout::BitsMsb, 0, d.len()), vec![(3, 0)]);
        let d = at(&[0x4d, 0x03]);
        assert_eq!(flag_matches(&d, &pattern, Layout::BitsLsb, 0, d.len()), vec![(3, 0)]);
        // word layout: flag 0 is the lowest bit of the last byte of the first word
        let d = at(&[0, 0, 0, 0x4d]);
        assert!(flag_matches(&d, &pattern[..8], Layout::BitsWord, 0, d.len()).contains(&(3, 0)));
    }

    #[test]
    fn the_set_flags_are_listed_up_to_the_count() {
        assert_eq!(Layout::Bytes.set_flags(&[0, 1, 0, 1, 1, 9, 1], 5), vec![1, 3, 4]);
        assert_eq!(Layout::BitsLsb.set_flags(&[0b0000_0110, 0b1000_0000], 16), vec![1, 2, 15]);
    }

    #[test]
    fn any_non_zero_value_is_a_set_flag_and_misses_are_counted() {
        let d = [0u8, 1, 1, 7, 0];
        assert_eq!(
            flag_matches(&d, &[false, true, true, true], Layout::Bytes, 0, d.len()),
            vec![(0, 0)]
        );
        assert!(flag_matches(&d, &[false, true, true, false], Layout::Bytes, 0, d.len()).is_empty());
        let d = [0u8, 1, 1, 1, 0];
        assert_eq!(
            flag_matches(&d, &[false, true, false, true], Layout::Bytes, 1, d.len()),
            vec![(0, 1)]
        );
    }

    #[test]
    fn gaps_score_above_a_plain_count() {
        let plain = Run {
            offset: 0,
            stride: 2,
            ids: vec![1, 2, 3, 4],
        };
        let gappy = Run {
            offset: 0,
            stride: 2,
            ids: vec![1, 3, 6, 9],
        };
        assert!(score(&gappy) > score(&plain));
    }
}

//! Looking for the blacksmith's list of pieces in the emulator's memory (the `scan` debug command).
//!
//! Where the game keeps the list of pieces on offer is not known. If it keeps one, it is probably an array of piece ids, perhaps with
//! other fields between them, so this finds runs of big-endian u16 values that are all in a given set of ids (the pieces we expect
//! on offer), strictly increasing, at one fixed distance apart. The result shows where our idea of the list and the game's differ.

use crate::procmem::{CHUNK_LEN, ProcMem};
use std::{collections::HashSet, io};

/// Distances between the ids to try, in bytes.
pub const STRIDES: [usize; 8] = [2, 4, 6, 8, 12, 16, 20, 24];

/// A run of ids found in a block of memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    /// Offset of the first id in the block.
    pub offset: usize,
    pub stride: usize,
    pub ids: Vec<u16>,
}

/// Every run of at least `min_len` ids from `wanted`, increasing, `stride` bytes apart, starting at an even offset below `start_below`.
/// A run is as long as it can be (it is not reported again from inside).
pub fn runs(data: &[u8], wanted: &HashSet<u16>, stride: usize, min_len: usize, start_below: usize) -> Vec<Run> {
    let at = |o: usize| -> Option<u16> {
        let v = u16::from_be_bytes([*data.get(o)?, *data.get(o + 1)?]);
        wanted.contains(&v).then_some(v)
    };
    let mut out = Vec::new();
    let mut o = 0;
    while o < data.len().min(start_below) {
        if let Some(first) = at(o) {
            let continues_one = o >= stride && at(o - stride).is_some_and(|p| p < first);
            if !continues_one {
                let mut ids = vec![first];
                let mut p = o + stride;
                while let Some(v) = at(p).filter(|&v| v > *ids.last().unwrap_or(&0)) {
                    ids.push(v);
                    p += stride;
                }
                if ids.len() >= min_len {
                    out.push(Run { offset: o, stride, ids });
                }
            }
        }
        o += 2;
    }
    out
}

/// A run's value as a find: more of the wanted ids is better, and ids that are not simply consecutive count for more (a counting table
/// matches anything contiguous).
pub fn score(run: &Run) -> usize {
    let gaps = run.ids.windows(2).filter(|w| w[1] != w[0] + 1).count();
    run.ids.len() * 4 + gaps * 3
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
        for stride in STRIDES {
            for run in runs(data, wanted, stride, min_len, CHUNK_LEN) {
                found.push(Found {
                    host: addr + run.offset as u64,
                    run,
                });
            }
        }
    })?;
    found.sort_by_key(|f| std::cmp::Reverse(score(&f.run)));
    found.truncate(keep);
    Ok(found)
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
            let found = runs(&d, &wanted, stride, 4, d.len());
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
        assert!(runs(&be(&[3, 5], 2), &wanted, 2, 3, 100).is_empty());
        assert!(runs(&be(&[3, 5, 7, 9], 2), &wanted, 2, 4, 100).is_empty(), "7 is not wanted");
        assert!(runs(&be(&[12, 9, 5, 3], 2), &wanted, 2, 2, 100).is_empty(), "decreasing");
    }

    #[test]
    fn a_run_is_reported_once_from_its_start() {
        let wanted: HashSet<u16> = [1, 2, 3, 4, 5].into();
        let d = be(&[1, 2, 3, 4, 5], 2);
        assert_eq!(runs(&d, &wanted, 2, 2, d.len()).len(), 1);
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

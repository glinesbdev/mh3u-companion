//! What the hunter picked up while the game ran: a log of item gains, kept per hunter.
//!
//! Live mode sees the pouch and the item box change as it happens, so each change that adds items is a gain: a carve, a quest
//! reward, a purchase. Moving an item between the pouch and the box adds nothing, and using or selling one is not logged. The game
//! does not say when a quest ends, so changes close together in time (see [`MERGE_SECONDS`]) are one entry; the quest rewards, which
//! arrive all at once, then show as a single block.

use mh3u_core::save::Save;
use std::collections::HashMap;

/// Changes at most this many seconds after the last one belong to the same entry.
pub const MERGE_SECONDS: u64 = 20;
/// Entries kept; the oldest go first.
const KEEP: usize = 300;

/// How many of each item the hunter has in the pouch and the box together.
pub type Counts = HashMap<u16, u32>;

pub fn counts(save: &Save) -> Counts {
    let mut out = Counts::new();
    for s in save.pouch.iter().chain(&save.item_box) {
        *out.entry(s.id).or_default() += u32::from(s.count);
    }
    out
}

/// What was gained over one stretch of play: items (id, how many more), and how the zenny changed over the same time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Seconds since the Unix epoch of the last change in this entry.
    pub at: u64,
    pub zenny: i64,
    pub items: Vec<(u16, u32)>,
}

/// The entries, newest last.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Log {
    pub entries: Vec<Entry>,
}

/// The items that `now` has more of than `before`, in item id order.
pub fn gained(before: &Counts, now: &Counts) -> Vec<(u16, u32)> {
    let mut out: Vec<(u16, u32)> = now
        .iter()
        .filter_map(|(&id, &n)| {
            n.checked_sub(before.get(&id).copied().unwrap_or(0))
                .filter(|&d| d > 0)
                .map(|d| (id, d))
        })
        .collect();
    out.sort_unstable();
    out
}

impl Log {
    /// Record a change from `before` to `now`. Returns what was gained (empty when nothing was, and then nothing is logged).
    pub fn observe(&mut self, before: &Counts, zenny_before: u32, now: &Counts, zenny: u32, at: u64) -> Vec<(u16, u32)> {
        let items = gained(before, now);
        if items.is_empty() {
            return items;
        }
        let zenny_change = i64::from(zenny) - i64::from(zenny_before);
        match self.entries.last_mut().filter(|e| at.saturating_sub(e.at) <= MERGE_SECONDS) {
            Some(last) => {
                last.at = at;
                last.zenny += zenny_change;
                for &(id, n) in &items {
                    match last.items.iter_mut().find(|(i, _)| *i == id) {
                        Some((_, total)) => *total += n,
                        None => last.items.push((id, n)),
                    }
                }
                last.items.sort_unstable();
            }
            None => {
                self.entries.push(Entry {
                    at,
                    zenny: zenny_change,
                    items: items.clone(),
                });
                let extra = self.entries.len().saturating_sub(KEEP);
                self.entries.drain(..extra);
            }
        }
        items
    }

    /// One line per entry: `time`, `zenny change`, `item:count,item:count`, separated by tabs.
    pub fn format(&self) -> String {
        self.entries
            .iter()
            .map(|e| {
                let items: Vec<String> = e.items.iter().map(|(i, n)| format!("{i}:{n}")).collect();
                format!("{}\t{}\t{}\n", e.at, e.zenny, items.join(","))
            })
            .collect()
    }

    /// Read what `format` wrote; lines that do not parse are skipped.
    pub fn parse(text: &str) -> Log {
        let entries = text
            .lines()
            .filter_map(|l| {
                let mut f = l.split('\t');
                let (at, zenny, list) = (f.next()?.parse().ok()?, f.next()?.parse().ok()?, f.next()?);
                let items = list
                    .split(',')
                    .filter_map(|p| {
                        let (i, n) = p.split_once(':')?;
                        Some((i.parse().ok()?, n.parse().ok()?))
                    })
                    .collect::<Vec<_>>();
                (!items.is_empty()).then_some(Entry { at, zenny, items })
            })
            .collect();
        Log { entries }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(pairs: &[(u16, u32)]) -> Counts {
        pairs.iter().copied().collect()
    }

    #[test]
    fn only_more_of_an_item_is_a_gain() {
        let before = c(&[(1, 5), (2, 3), (3, 1)]);
        let now = c(&[(1, 5), (2, 1), (3, 4), (9, 2)]);
        assert_eq!(gained(&before, &now), vec![(3, 3), (9, 2)]);
        assert!(gained(&now, &now).is_empty());
    }

    #[test]
    fn changes_close_together_make_one_entry_and_a_pause_starts_another() {
        let mut log = Log::default();
        log.observe(&c(&[]), 100, &c(&[(1, 2)]), 100, 1000);
        log.observe(&c(&[(1, 2)]), 100, &c(&[(1, 3), (4, 1)]), 600, 1010);
        assert_eq!(log.entries.len(), 1);
        assert_eq!(
            log.entries[0],
            Entry {
                at: 1010,
                zenny: 500,
                items: vec![(1, 3), (4, 1)]
            }
        );
        log.observe(&c(&[(1, 3), (4, 1)]), 600, &c(&[(1, 4), (4, 1)]), 600, 1011 + MERGE_SECONDS);
        assert_eq!(log.entries.len(), 2);
    }

    #[test]
    fn a_change_with_no_gain_is_not_logged() {
        let mut log = Log::default();
        assert!(log.observe(&c(&[(1, 2)]), 100, &c(&[(1, 1)]), 400, 5).is_empty());
        assert!(log.entries.is_empty());
    }

    #[test]
    fn the_oldest_entries_go_first() {
        let mut log = Log::default();
        for i in 0..KEEP as u64 + 5 {
            log.observe(&c(&[]), 0, &c(&[(1, 1)]), 0, i * 1000);
        }
        assert_eq!(log.entries.len(), KEEP);
        assert_eq!(log.entries[0].at, 5000);
    }

    #[test]
    fn the_file_round_trips_and_bad_lines_are_skipped() {
        let mut log = Log::default();
        log.observe(&c(&[]), 0, &c(&[(7, 2), (9, 1)]), 300, 42);
        let text = log.format();
        assert_eq!(text, "42\t300\t7:2,9:1\n");
        assert_eq!(Log::parse(&format!("junk\n{text}1\tx\t2:2\n")), log);
    }
}

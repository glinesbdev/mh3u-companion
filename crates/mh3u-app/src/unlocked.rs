//! The pieces the blacksmith has been seen offering, per hunter, kept in `unlocked.tsv` next to the price ledger.
//!
//! A piece the blacksmith offers stays on the list for good, but the app can only see what the hunter holds right now. So
//! whenever a piece is seen on offer (its unlock material is in the pouch or box), it is remembered here.

use std::collections::{HashMap, HashSet};

/// (equipment kind, piece id).
pub type Piece = (u8, u16);

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Unlocked {
    by_hunter: HashMap<String, HashSet<Piece>>,
}

impl Unlocked {
    /// Lines of `hunter<TAB>kind<TAB>id`; lines that don't parse are ignored.
    pub fn parse(text: &str) -> Unlocked {
        let mut out = Unlocked::default();
        for line in text.lines() {
            let mut parts = line.split('\t');
            let (Some(hunter), Some(kind), Some(id)) = (parts.next(), parts.next(), parts.next()) else {
                continue;
            };
            if let (false, Ok(kind), Ok(id)) = (hunter.is_empty(), kind.parse(), id.parse()) {
                out.add(hunter, (kind, id));
            }
        }
        out
    }

    pub fn format(&self) -> String {
        let mut lines: Vec<(&str, Piece)> = self
            .by_hunter
            .iter()
            .flat_map(|(hunter, pieces)| pieces.iter().map(move |&p| (hunter.as_str(), p)))
            .collect();
        lines.sort_unstable();
        lines.iter().map(|(h, (kind, id))| format!("{h}\t{kind}\t{id}\n")).collect()
    }

    pub fn contains(&self, hunter: &str, piece: Piece) -> bool {
        self.by_hunter.get(hunter).is_some_and(|set| set.contains(&piece))
    }

    /// Remember a piece; returns whether it was new. A nameless hunter is never recorded.
    pub fn add(&mut self, hunter: &str, piece: Piece) -> bool {
        !hunter.is_empty() && self.by_hunter.entry(hunter.to_owned()).or_default().insert(piece)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_pieces_per_hunter() {
        let mut u = Unlocked::default();
        assert!(u.add("Ann", (4, 11)));
        assert!(!u.add("Ann", (4, 11)), "already known");
        assert!(u.add("Bob", (4, 11)));
        assert!(u.contains("Ann", (4, 11)) && u.contains("Bob", (4, 11)));
        assert!(!u.contains("Ann", (4, 12)));
        assert!(!u.contains("Cy", (4, 11)));
        assert!(!u.add("", (1, 1)), "a nameless hunter is not recorded");
    }

    #[test]
    fn round_trips_through_text_and_skips_bad_lines() {
        let mut u = Unlocked::default();
        u.add("Ann", (4, 12));
        u.add("Ann", (4, 11));
        u.add("Bob", (7, 3));
        let text = u.format();
        assert_eq!(text, "Ann\t4\t11\nAnn\t4\t12\nBob\t7\t3\n", "sorted, one piece per line");
        assert_eq!(Unlocked::parse(&text), u);
        assert_eq!(Unlocked::parse("junk\n\t1\t2\nAnn\tx\t2\nAnn\t1\n"), Unlocked::default());
    }
}

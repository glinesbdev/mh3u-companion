//! "What changed since last time": what the hunter held when the app last closed, kept in a small text file per hunter, and a
//! sentence about how the save differs now.

use mh3u_core::save::Save;
use std::collections::{BTreeMap, BTreeSet};

/// What the hunter held: zenny, how many of each item (pouch and box together) and which pieces of equipment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Seen {
    pub hunter: String,
    pub zenny: u32,
    pub items: BTreeMap<u16, u32>,
    pub pieces: BTreeSet<(u8, u16)>,
}

impl Seen {
    pub fn of(save: &Save) -> Seen {
        let mut items = BTreeMap::new();
        for s in save.pouch.iter().chain(&save.item_box) {
            *items.entry(s.id).or_default() += u32::from(s.count);
        }
        Seen {
            hunter: save.hunter_name.clone(),
            zenny: save.zenny,
            items,
            pieces: save.equipment_box.iter().map(|e| (e.kind, e.id)).collect(),
        }
    }

    /// One line per fact: `hunter<TAB>name`, `zenny<TAB>n`, `item<TAB>id<TAB>count`, `piece<TAB>kind<TAB>id`.
    pub fn format(&self) -> String {
        let mut out = format!("hunter\t{}\nzenny\t{}\n", self.hunter, self.zenny);
        for (id, n) in &self.items {
            out.push_str(&format!("item\t{id}\t{n}\n"));
        }
        for (kind, id) in &self.pieces {
            out.push_str(&format!("piece\t{kind}\t{id}\n"));
        }
        out
    }

    /// Read what `format` wrote; `None` if it is not that (no hunter line).
    pub fn parse(text: &str) -> Option<Seen> {
        let mut seen = Seen::default();
        let mut named = false;
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            match f.as_slice() {
                ["hunter", name] => {
                    seen.hunter = name.to_string();
                    named = true;
                }
                ["zenny", n] => seen.zenny = n.parse().ok()?,
                ["item", id, n] => {
                    seen.items.insert(id.parse().ok()?, n.parse().ok()?);
                }
                ["piece", kind, id] => {
                    seen.pieces.insert((kind.parse().ok()?, id.parse().ok()?));
                }
                _ => {}
            }
        }
        named.then_some(seen)
    }
}

/// A sentence on how `now` differs from `before`, or `None` when it is the same hunter with nothing different. Items count by the
/// number of each, so moving between pouch and box is no change. `item_name` and `piece_name` give the names shown.
pub fn summary(before: &Seen, now: &Seen, item_name: impl Fn(u16) -> String, piece_name: impl Fn(u8, u16) -> String) -> Option<String> {
    if before.hunter != now.hunter {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    if now.zenny != before.zenny {
        let delta = i64::from(now.zenny) - i64::from(before.zenny);
        parts.push(format!("zenny {}{delta}", if delta > 0 { "+" } else { "" }));
    }
    let gained: Vec<(u16, u32)> = now
        .items
        .iter()
        .filter_map(|(&id, &n)| {
            n.checked_sub(before.items.get(&id).copied().unwrap_or(0))
                .filter(|&d| d > 0)
                .map(|d| (id, d))
        })
        .collect();
    if !gained.is_empty() {
        let shown: Vec<String> = gained.iter().take(3).map(|&(id, n)| format!("{} x{n}", item_name(id))).collect();
        let more = gained.len().saturating_sub(3);
        parts.push(format!(
            "gained {}{}",
            shown.join(", "),
            if more > 0 { format!(" and {more} more") } else { String::new() }
        ));
    }
    let used = before
        .items
        .iter()
        .filter(|&(&id, &n)| now.items.get(&id).copied().unwrap_or(0) < n)
        .count();
    if used > 0 {
        parts.push(format!("{used} kind(s) of item used or sold"));
    }
    let new_pieces: Vec<&(u8, u16)> = now.pieces.difference(&before.pieces).collect();
    if !new_pieces.is_empty() {
        let shown: Vec<String> = new_pieces.iter().take(3).map(|&&(k, id)| piece_name(k, id)).collect();
        let more = new_pieces.len().saturating_sub(3);
        parts.push(format!(
            "new gear {}{}",
            shown.join(", "),
            if more > 0 { format!(" and {more} more") } else { String::new() }
        ));
    }
    (!parts.is_empty()).then(|| format!("since last time: {}", parts.join(" · ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(zenny: u32, items: &[(u16, u32)], pieces: &[(u8, u16)]) -> Seen {
        Seen {
            hunter: "H".into(),
            zenny,
            items: items.iter().copied().collect(),
            pieces: pieces.iter().copied().collect(),
        }
    }

    fn say(before: &Seen, now: &Seen) -> Option<String> {
        summary(before, now, |id| format!("item{id}"), |k, id| format!("piece{k}.{id}"))
    }

    #[test]
    fn the_file_round_trips() {
        let s = seen(1234, &[(5, 3), (9, 1)], &[(5, 2), (7, 40)]);
        assert_eq!(Seen::parse(&s.format()), Some(s));
        assert_eq!(Seen::parse("junk"), None, "no hunter line");
    }

    #[test]
    fn what_is_different_is_told_and_what_is_not_is_left_out() {
        let before = seen(1000, &[(1, 5), (2, 2)], &[(5, 1)]);
        assert_eq!(say(&before, &before.clone()), None);
        let now = seen(2500, &[(1, 8), (2, 1), (3, 4)], &[(5, 1), (5, 2)]);
        assert_eq!(
            say(&before, &now).unwrap(),
            "since last time: zenny +1500 · gained item1 x3, item3 x4 · 1 kind(s) of item used or sold · new gear piece5.2"
        );
        assert_eq!(
            say(&now, &before).unwrap(),
            "since last time: zenny -1500 · gained item2 x1 · 2 kind(s) of item used or sold"
        );
    }

    #[test]
    fn a_long_list_is_cut_and_another_hunter_is_no_comparison() {
        let before = seen(0, &[], &[]);
        let now = seen(0, &[(1, 1), (2, 1), (3, 1), (4, 1), (5, 1)], &[]);
        assert!(
            say(&before, &now)
                .unwrap()
                .ends_with("gained item1 x1, item2 x1, item3 x1 and 2 more")
        );
        let mut other = now.clone();
        other.hunter = "Other".into();
        assert_eq!(say(&before, &other), None);
    }
}

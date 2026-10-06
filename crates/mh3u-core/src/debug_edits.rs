//! A record of what the debug commands (`--debug-edit`) changed in a hunter's data, so that it can be taken out again.
//!
//! Every command's effect is found by comparing the hunter's live data before and after it: the zenny added, the items added to
//! the item box, the equipment records made or changed. That is kept per hunter in a small file. [`Ledger::purge`] works out the
//! patches that take it all out of the data again: it removes the zenny and items still there (never more than the hunter now has),
//! empties the pieces that were made (taking them off first if worn) and puts changed records back. The program does this on its own when
//! the emulator goes online (see `online.rs`), and on request.
//!
//! This is a safeguard for honest users who do not want edited gear in online play. It is not protection against someone who changes
//! this open source program, or who plays without it.

use crate::edit::{self, Patch};
use crate::save::{
    BOX_OFFSET, BOX_SLOTS, EQUIP_LEN, EQUIP_OFFSET, EQUIP_SLOTS, POUCH_OFFSET, POUCH_SLOTS, SAVE_LEN, Save, WORN_OFFSET, WORN_SLOTS,
    WORN_TALISMAN_OFFSET, WORN_WEAPON_OFFSET,
};
use std::collections::BTreeMap;

type Record = [u8; EQUIP_LEN];

/// What a command did to one equipment box slot.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    /// The slot was empty and a piece was put in it.
    Made(Record),
    /// The slot held this (`original`) and was changed to `current`.
    Changed { original: Record, current: Record },
}

/// What the debug commands have added to one hunter.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ledger {
    /// Zenny added (negative if the commands took some away).
    zenny: i64,
    /// Resource Points added (negative if the commands took some away).
    points: i64,
    /// Items added to the item box by id (negative: taken away).
    items: BTreeMap<u16, i64>,
    pieces: BTreeMap<usize, Piece>,
}

/// What a purge did, in words for the status line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Report {
    pub zenny: u32,
    pub points: u32,
    pub items: u32,
    pub made: usize,
    pub restored: usize,
    /// Things that could not be taken out, and why.
    pub kept: Vec<String>,
}

impl Report {
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if self.zenny > 0 {
            parts.push(format!("{}z", self.zenny));
        }
        if self.points > 0 {
            parts.push(format!("{} resource point(s)", self.points));
        }
        if self.items > 0 {
            parts.push(format!("{} item(s)", self.items));
        }
        if self.made > 0 {
            parts.push(format!("{} piece(s) of gear", self.made));
        }
        if self.restored > 0 {
            parts.push(format!("{} changed record(s) put back", self.restored));
        }
        let mut text = if parts.is_empty() {
            "nothing to take out".to_string()
        } else {
            format!("took out {}", parts.join(", "))
        };
        if !self.kept.is_empty() {
            text += &format!("; kept: {}", self.kept.join(", "));
        }
        text
    }
}

fn be16(d: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([d[o], d[o + 1]])
}

fn record_at(d: &[u8], slot: usize) -> Record {
    let at = EQUIP_OFFSET + slot * EQUIP_LEN;
    d[at..at + EQUIP_LEN].try_into().expect("a record is 16 bytes")
}

fn item_counts(save: &Save) -> BTreeMap<u16, i64> {
    let mut out = BTreeMap::new();
    for s in &save.item_box {
        *out.entry(s.id).or_insert(0) += i64::from(s.count);
    }
    out
}

impl Ledger {
    pub fn is_empty(&self) -> bool {
        self.zenny == 0 && self.points == 0 && self.items.is_empty() && self.pieces.is_empty()
    }

    /// Note what a command changed: `before` and `after` are the hunter's whole data block either side of it.
    pub fn record(&mut self, before: &[u8], after: &[u8]) {
        if before.len() != SAVE_LEN || after.len() != SAVE_LEN {
            return;
        }
        let (Ok(b), Ok(a)) = (Save::parse(before), Save::parse(after)) else {
            return;
        };
        self.zenny += i64::from(a.zenny) - i64::from(b.zenny);
        self.points += i64::from(a.resource_points) - i64::from(b.resource_points);
        let (was, now) = (item_counts(&b), item_counts(&a));
        for id in was.keys().chain(now.keys()).copied().collect::<std::collections::BTreeSet<_>>() {
            let delta = now.get(&id).copied().unwrap_or(0) - was.get(&id).copied().unwrap_or(0);
            if delta != 0 {
                let total = self.items.entry(id).or_insert(0);
                *total += delta;
                if *total == 0 {
                    self.items.remove(&id);
                }
            }
        }
        for slot in 0..EQUIP_SLOTS {
            let (old, new) = (record_at(before, slot), record_at(after, slot));
            if old == new {
                continue;
            }
            if new[0] == 0 {
                // emptied (by hand or by a command): nothing of ours is there any more
                self.pieces.remove(&slot);
                continue;
            }
            let entry = match self.pieces.remove(&slot) {
                Some(Piece::Made(_)) => Piece::Made(new),
                Some(Piece::Changed { original, .. }) => Piece::Changed { original, current: new },
                None if old[0] == 0 => Piece::Made(new),
                None => Piece::Changed {
                    original: old,
                    current: new,
                },
            };
            self.pieces.insert(slot, entry);
        }
    }

    /// The patches that take the recorded changes out of `live`, and a report. Nothing is changed in `live` itself.
    pub fn purge(&self, live: &[u8]) -> (Vec<Patch>, Report) {
        let mut report = Report::default();
        let mut patches = Vec::new();
        if live.len() != SAVE_LEN {
            return (patches, report);
        }
        let mut data = live.to_vec();
        let put = |patch: Patch, data: &mut Vec<u8>, patches: &mut Vec<Patch>| {
            edit::apply(data, &patch);
            patches.push(patch);
        };
        if let Ok(save) = Save::parse(live)
            && self.zenny > 0
        {
            let take = self.zenny.min(i64::from(save.zenny)) as u32;
            if take > 0 {
                put(edit::set_zenny(save.zenny - take), &mut data, &mut patches);
                report.zenny = take;
            }
        }
        if let Ok(save) = Save::parse(&data)
            && self.points > 0
        {
            let take = self.points.min(i64::from(save.resource_points)) as u32;
            if take > 0 {
                put(edit::set_resource_points(save.resource_points - take), &mut data, &mut patches);
                report.points = take;
            }
        }
        for (&id, &added) in self.items.iter().filter(|(_, n)| **n > 0) {
            let mut left = added as u32;
            // the item box first, then the pouch: it may have been taken on a quest
            for (base, slots) in [(BOX_OFFSET, BOX_SLOTS), (POUCH_OFFSET, POUCH_SLOTS)] {
                for slot in 0..slots {
                    let at = base + slot * 4;
                    if left == 0 {
                        break;
                    }
                    if be16(&data, at) != id || be16(&data, at + 2) == 0 {
                        continue;
                    }
                    let have = u32::from(be16(&data, at + 2));
                    let take = have.min(left);
                    left -= take;
                    report.items += take;
                    let remaining = (have - take) as u16;
                    let (new_id, bytes) = if remaining == 0 { (0u16, 0u16) } else { (id, remaining) };
                    let mut bytes_out = new_id.to_be_bytes().to_vec();
                    bytes_out.extend(bytes.to_be_bytes());
                    put(
                        Patch {
                            offset: at,
                            bytes: bytes_out,
                        },
                        &mut data,
                        &mut patches,
                    );
                }
            }
        }
        for (&slot, piece) in &self.pieces {
            let now = record_at(&data, slot);
            let at = EQUIP_OFFSET + slot * EQUIP_LEN;
            match piece {
                Piece::Made(made) => {
                    if now[0] != made[0] || now[2..4] != made[2..4] {
                        report.kept.push(format!("slot {slot} is another piece now"));
                        continue;
                    }
                    if let Some(why) = unequip(&mut data, slot, &mut patches) {
                        report.kept.push(why);
                        continue;
                    }
                    put(
                        Patch {
                            offset: at,
                            bytes: vec![0; EQUIP_LEN],
                        },
                        &mut data,
                        &mut patches,
                    );
                    report.made += 1;
                }
                Piece::Changed { original, current } => {
                    if now != *current {
                        report.kept.push(format!("slot {slot} changed again since"));
                        continue;
                    }
                    put(
                        Patch {
                            offset: at,
                            bytes: original.to_vec(),
                        },
                        &mut data,
                        &mut patches,
                    );
                    report.restored += 1;
                }
            }
        }
        (patches, report)
    }

    /// The ledger as the text kept in its file.
    pub fn format(&self) -> String {
        let hex = |r: &Record| r.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let mut out = String::new();
        if self.zenny != 0 {
            out += &format!("zenny {}\n", self.zenny);
        }
        if self.points != 0 {
            out += &format!("points {}\n", self.points);
        }
        for (id, n) in &self.items {
            out += &format!("item {id} {n}\n");
        }
        for (slot, piece) in &self.pieces {
            match piece {
                Piece::Made(r) => out += &format!("made {slot} {}\n", hex(r)),
                Piece::Changed { original, current } => out += &format!("changed {slot} {} {}\n", hex(original), hex(current)),
            }
        }
        out
    }

    /// Read the file's text; lines that do not make sense are skipped.
    pub fn parse(text: &str) -> Ledger {
        fn rec(s: &str) -> Option<Record> {
            if s.len() != EQUIP_LEN * 2 {
                return None;
            }
            let bytes: Option<Vec<u8>> = (0..EQUIP_LEN)
                .map(|i| u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok())
                .collect();
            bytes?.try_into().ok()
        }
        let mut out = Ledger::default();
        for line in text.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            match w[..] {
                ["zenny", n] => out.zenny = n.parse().unwrap_or(0),
                ["points", n] => out.points = n.parse().unwrap_or(0),
                ["item", id, n] => {
                    if let (Ok(id), Ok(n)) = (id.parse(), n.parse::<i64>()) {
                        out.items.insert(id, n);
                    }
                }
                ["made", slot, r] => {
                    if let (Ok(slot), Some(r)) = (slot.parse::<usize>(), rec(r))
                        && slot < EQUIP_SLOTS
                    {
                        out.pieces.insert(slot, Piece::Made(r));
                    }
                }
                ["changed", slot, a, b] => {
                    if let (Ok(slot), Some(original), Some(current)) = (slot.parse::<usize>(), rec(a), rec(b))
                        && slot < EQUIP_SLOTS
                    {
                        out.pieces.insert(slot, Piece::Changed { original, current });
                    }
                }
                _ => {}
            }
        }
        out
    }
}

/// Take a piece off if it is worn: armor and a talisman by clearing the pointer, a weapon by wearing another weapon from the box.
/// `Some(reason)` when it cannot be done (the only weapon).
fn unequip(data: &mut Vec<u8>, slot: usize, patches: &mut Vec<Patch>) -> Option<String> {
    let slot16 = slot as u16;
    let mut put = |offset: usize, bytes: Vec<u8>, data: &mut Vec<u8>| {
        let patch = Patch { offset, bytes };
        edit::apply(data, &patch);
        patches.push(patch);
    };
    for i in 0..WORN_SLOTS {
        if be16(data, WORN_OFFSET + 2 * i) == slot16 {
            put(WORN_OFFSET + 2 * i, vec![0xff, 0xff], data);
        }
    }
    if be16(data, WORN_TALISMAN_OFFSET) == slot16 {
        put(WORN_TALISMAN_OFFSET, vec![0xff, 0xff], data);
        // the copy of the worn talisman's record before the pointers: kind 6 and zeros when none is worn
        let mut blank = vec![0u8; EQUIP_LEN];
        blank[0] = 6;
        put(WORN_OFFSET - EQUIP_LEN, blank, data);
    }
    if be16(data, WORN_WEAPON_OFFSET) == slot16 {
        let kind = data[EQUIP_OFFSET + slot * EQUIP_LEN];
        let other = (0..EQUIP_SLOTS).filter(|&s| s != slot).find(|&s| {
            let k = data[EQUIP_OFFSET + s * EQUIP_LEN];
            k >= 7 && k == kind
        });
        let other = other.or_else(|| {
            (0..EQUIP_SLOTS)
                .filter(|&s| s != slot)
                .find(|&s| data[EQUIP_OFFSET + s * EQUIP_LEN] >= 7)
        });
        match other {
            Some(s) => put(WORN_WEAPON_OFFSET, (s as u16).to_be_bytes().to_vec(), data),
            None => return Some(format!("the weapon in slot {slot} is worn and there is no other to wear")),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block() -> Vec<u8> {
        let mut d = vec![0u8; SAVE_LEN];
        // a name so that it parses, and no worn gear
        d[0x2b..0x2b + 4].copy_from_slice(b"Test");
        for o in [WORN_WEAPON_OFFSET, WORN_TALISMAN_OFFSET]
            .into_iter()
            .chain((0..WORN_SLOTS).map(|i| WORN_OFFSET + 2 * i))
        {
            d[o..o + 2].copy_from_slice(&[0xff, 0xff]);
        }
        d
    }

    fn apply_all(d: &mut [u8], patches: &[Patch]) {
        for p in patches {
            edit::apply(d, p);
        }
    }

    fn after_commands(before: &[u8]) -> Vec<u8> {
        let mut d = before.to_vec();
        edit::apply(&mut d, &edit::set_zenny(5000));
        let p = edit::set_box_item(&d, 205, 20).unwrap();
        edit::apply(&mut d, &p);
        let (_, p) = edit::new_piece(&d, 5, 1).unwrap();
        edit::apply(&mut d, &p);
        d
    }

    fn start() -> Vec<u8> {
        let mut d = block();
        edit::apply(&mut d, &edit::set_zenny(100));
        let p = edit::set_box_item(&d, 205, 3).unwrap();
        edit::apply(&mut d, &p);
        d
    }

    #[test]
    fn what_a_command_added_is_taken_out_again() {
        let before = start();
        let after = after_commands(&before);
        let mut ledger = Ledger::default();
        ledger.record(&before, &after);
        assert!(!ledger.is_empty());
        let (patches, report) = ledger.purge(&after);
        let mut data = after.clone();
        apply_all(&mut data, &patches);
        let save = Save::parse(&data).unwrap();
        assert_eq!(save.zenny, 100, "back to what it was");
        assert_eq!(save.item_count(205), 3, "the 17 added are gone, the 3 the hunter had stay");
        assert!(save.equipment_box.is_empty(), "the made piece is gone");
        assert_eq!((report.zenny, report.items, report.made), (4900, 17, 1));
        assert_eq!(report.summary(), "took out 4900z, 17 item(s), 1 piece(s) of gear");
    }

    #[test]
    fn resource_points_added_are_taken_out_again_but_not_those_earned() {
        let mut before = start();
        edit::apply(&mut before, &edit::set_resource_points(14));
        let mut after = before.clone();
        edit::apply(&mut after, &edit::set_resource_points(5014));
        let mut ledger = Ledger::default();
        ledger.record(&before, &after);
        assert_eq!(ledger.format(), "points 5000\n");
        assert_eq!(Ledger::parse(&ledger.format()), ledger);
        // 6 points were earned and 500 spent meanwhile: what can be taken is never more than there is
        let mut now = after.clone();
        edit::apply(&mut now, &edit::set_resource_points(3));
        let (patches, report) = ledger.purge(&now);
        let mut data = now.clone();
        apply_all(&mut data, &patches);
        assert_eq!(Save::parse(&data).unwrap().resource_points, 0);
        assert_eq!(report.points, 3);
        assert_eq!(report.summary(), "took out 3 resource point(s)");
        // with the points as they were, it is back to 14
        let (patches, report) = ledger.purge(&after);
        let mut data = after.clone();
        apply_all(&mut data, &patches);
        assert_eq!(Save::parse(&data).unwrap().resource_points, 14);
        assert_eq!(report.points, 5000);
    }

    #[test]
    fn only_what_is_still_there_is_taken() {
        let before = start();
        let mut ledger = Ledger::default();
        let after = after_commands(&before);
        ledger.record(&before, &after);
        // the hunter spent most of the zenny and used up some items since
        let mut now = after.clone();
        edit::apply(&mut now, &edit::set_zenny(40));
        let p = edit::set_box_item(&now, 205, 2).unwrap();
        edit::apply(&mut now, &p);
        let (patches, report) = ledger.purge(&now);
        apply_all(&mut now, &patches);
        let save = Save::parse(&now).unwrap();
        assert_eq!((save.zenny, save.item_count(205)), (0, 0), "never below zero");
        assert_eq!((report.zenny, report.items), (40, 2));
    }

    #[test]
    fn items_taken_to_the_pouch_are_taken_out_of_it_too() {
        let before = block();
        let mut after = before.clone();
        let p = edit::set_box_item(&after, 205, 10).unwrap();
        edit::apply(&mut after, &p);
        let mut ledger = Ledger::default();
        ledger.record(&before, &after);
        // 6 moved to the pouch (slot 0), 4 stayed in the box
        let mut now = after.clone();
        let p = edit::set_box_item(&now, 205, 4).unwrap();
        edit::apply(&mut now, &p);
        now[POUCH_OFFSET..POUCH_OFFSET + 4].copy_from_slice(&[0, 205, 0, 6]);
        let (patches, report) = ledger.purge(&now);
        apply_all(&mut now, &patches);
        let save = Save::parse(&now).unwrap();
        assert_eq!((save.item_count(205), report.items), (0, 10));
    }

    #[test]
    fn a_worn_made_piece_is_taken_off_first() {
        let before = block();
        let mut after = before.clone();
        let (slot, p) = edit::new_piece(&after, 5, 1).unwrap();
        edit::apply(&mut after, &p);
        after[WORN_OFFSET + 8..WORN_OFFSET + 10].copy_from_slice(&(slot as u16).to_be_bytes());
        let mut ledger = Ledger::default();
        ledger.record(&before, &after);
        let (patches, report) = ledger.purge(&after);
        let mut data = after.clone();
        apply_all(&mut data, &patches);
        assert_eq!(be16(&data, WORN_OFFSET + 8), 0xffff, "taken off");
        assert_eq!(data[EQUIP_OFFSET], 0);
        assert_eq!(report.made, 1);
    }

    #[test]
    fn a_worn_weapon_is_swapped_for_another_and_the_only_one_is_kept() {
        let before = block();
        let mut after = before.clone();
        let (slot, p) = edit::new_piece(&after, 7, 5).unwrap();
        edit::apply(&mut after, &p);
        after[WORN_WEAPON_OFFSET..WORN_WEAPON_OFFSET + 2].copy_from_slice(&(slot as u16).to_be_bytes());
        let mut ledger = Ledger::default();
        ledger.record(&before, &after);
        let (patches, report) = ledger.purge(&after);
        assert!(patches.is_empty(), "no other weapon to wear: {report:?}");
        assert!(report.kept[0].contains("no other to wear"), "{:?}", report.kept);
        // with a weapon of the hunter's own in the box, that one is worn
        let mut with_own = before.clone();
        let own = EQUIP_OFFSET + 4 * EQUIP_LEN;
        with_own[own..own + 4].copy_from_slice(&[7, 0, 0, 1]);
        let mut after = with_own.clone();
        let (slot, p) = edit::new_piece(&after, 7, 5).unwrap();
        edit::apply(&mut after, &p);
        after[WORN_WEAPON_OFFSET..WORN_WEAPON_OFFSET + 2].copy_from_slice(&(slot as u16).to_be_bytes());
        let mut ledger = Ledger::default();
        ledger.record(&with_own, &after);
        let (patches, _) = ledger.purge(&after);
        let mut data = after.clone();
        apply_all(&mut data, &patches);
        assert_eq!(be16(&data, WORN_WEAPON_OFFSET), 4, "the hunter's own weapon");
        assert_eq!(data[EQUIP_OFFSET + slot * EQUIP_LEN], 0);
    }

    #[test]
    fn a_changed_record_is_put_back_unless_it_changed_again() {
        let mut before = block();
        before[EQUIP_OFFSET..EQUIP_OFFSET + 8].copy_from_slice(&[6, 0, 0, 1, 0x25, 0x0a, 0, 0]);
        let mut after = before.clone();
        after[EQUIP_OFFSET + 1] = 3; // three gem slots
        let mut ledger = Ledger::default();
        ledger.record(&before, &after);
        let (patches, report) = ledger.purge(&after);
        let mut data = after.clone();
        apply_all(&mut data, &patches);
        assert_eq!(data[EQUIP_OFFSET + 1], 0);
        assert_eq!(report.restored, 1);
        // a jewel socketed since: the record is not the one the command left, so it stays
        let mut later = after.clone();
        later[EQUIP_OFFSET + 9] = 0x91;
        let (patches, report) = ledger.purge(&later);
        assert!(patches.is_empty());
        assert_eq!(report.kept.len(), 1);
    }

    #[test]
    fn several_commands_add_up_and_the_original_of_a_piece_is_kept() {
        let mut ledger = Ledger::default();
        let mut d = block();
        for n in [10, 5] {
            let before = d.clone();
            let p = edit::set_box_item(&d, 205, n + if n == 10 { 0 } else { 10 }).unwrap();
            edit::apply(&mut d, &p);
            ledger.record(&before, &d);
        }
        assert_eq!(ledger.items.get(&205), Some(&15));
        // an item given and then set back to nothing nets to nothing
        let before = d.clone();
        let p = edit::set_box_item(&d, 205, 0).unwrap();
        edit::apply(&mut d, &p);
        ledger.record(&before, &d);
        assert!(ledger.is_empty());
    }

    #[test]
    fn the_ledger_round_trips_through_its_file_and_bad_lines_are_skipped() {
        let before = start();
        let after = after_commands(&before);
        let mut ledger = Ledger::default();
        ledger.record(&before, &after);
        let mut changed = after.clone();
        changed[EQUIP_OFFSET + 3 * EQUIP_LEN] = 0;
        let text = ledger.format();
        assert_eq!(Ledger::parse(&text), ledger);
        assert_eq!(Ledger::parse(&format!("nonsense\nzenny x\nitem 1\nmade 5000 00\n{text}")), ledger);
        assert!(Ledger::parse("").is_empty());
    }

    #[test]
    fn a_block_of_the_wrong_size_changes_nothing() {
        let mut ledger = Ledger::default();
        ledger.record(&[0; 10], &[1; 10]);
        assert!(ledger.is_empty());
        assert!(Ledger::default().purge(&[0; 10]).0.is_empty());
    }
}

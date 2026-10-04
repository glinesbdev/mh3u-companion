//! Changes to the save data, as byte patches. Used by the debug tools to set up situations in the live game (zenny,
//! materials) so features can be tested without hours of play. Nothing here touches a file or memory; a patch is just
//! "these bytes at this offset", to be applied by whoever holds the live memory.

use crate::save::{BOX_OFFSET, BOX_SLOTS, EQUIP_LEN, EQUIP_OFFSET, EQUIP_SLOTS, SAVE_LEN, ZENNY_OFFSET};
use anyhow::{Result, bail};

/// The most of one item a stack holds.
pub const MAX_STACK: u16 = 99;
/// Kept well under any plausible in-game wallet limit.
pub const MAX_ZENNY: u32 = 999_999;

/// New contents for part of the save block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patch {
    pub offset: usize,
    pub bytes: Vec<u8>,
}

/// Apply a patch to a copy of the save block (so later patches see the earlier ones).
pub fn apply(data: &mut [u8], patch: &Patch) {
    data[patch.offset..patch.offset + patch.bytes.len()].copy_from_slice(&patch.bytes);
}

/// Set the wallet to `amount` (limited to `MAX_ZENNY`).
pub fn set_zenny(amount: u32) -> Patch {
    let bytes = amount.min(MAX_ZENNY).to_be_bytes();
    Patch {
        offset: ZENNY_OFFSET,
        bytes: bytes[1..].to_vec(),
    }
}

fn be16(d: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([d[o], d[o + 1]])
}

/// Set the item box's count of `id` to `count` (limited to `MAX_STACK`; 0 removes the item). Uses the item's existing stack,
/// or the first empty slot.
pub fn set_box_item(data: &[u8], id: u16, count: u16) -> Result<Patch> {
    if data.len() != SAVE_LEN {
        bail!("not a save block");
    }
    let count = count.min(MAX_STACK);
    let slot_of = |i: usize| BOX_OFFSET + i * 4;
    let existing = (0..BOX_SLOTS).find(|&i| be16(data, slot_of(i)) == id && be16(data, slot_of(i) + 2) > 0);
    let slot = match existing {
        Some(i) => i,
        None if count == 0 => bail!("that item is not in the box"),
        None => (0..BOX_SLOTS)
            .find(|&i| be16(data, slot_of(i) + 2) == 0)
            .ok_or_else(|| anyhow::anyhow!("the item box is full"))?,
    };
    let (id, count) = if count == 0 { (0, 0) } else { (id, count) };
    let mut bytes = id.to_be_bytes().to_vec();
    bytes.extend(count.to_be_bytes());
    Ok(Patch {
        offset: slot_of(slot),
        bytes,
    })
}

/// The 16 bytes of an equipment box record (`slot` counts from 0).
pub fn equipment_record(data: &[u8], slot: usize) -> Result<&[u8]> {
    if data.len() != SAVE_LEN {
        bail!("not a save block");
    }
    if slot >= EQUIP_SLOTS {
        bail!("the equipment box has slots 0 to {}", EQUIP_SLOTS - 1);
    }
    let at = EQUIP_OFFSET + slot * EQUIP_LEN;
    Ok(&data[at..at + EQUIP_LEN])
}

/// Overwrite `bytes` inside an equipment box record, starting `offset` bytes into it.
pub fn poke_equipment(data: &[u8], slot: usize, offset: usize, bytes: &[u8]) -> Result<Patch> {
    equipment_record(data, slot)?;
    if bytes.is_empty() || offset + bytes.len() > EQUIP_LEN {
        bail!(
            "a record is {EQUIP_LEN} bytes: {} byte(s) at offset {offset} do not fit",
            bytes.len()
        );
    }
    Ok(Patch {
        offset: EQUIP_OFFSET + slot * EQUIP_LEN + offset,
        bytes: bytes.to_vec(),
    })
}

/// The first unused equipment box slot (a record whose kind byte is 0).
pub fn first_empty_equipment(data: &[u8]) -> Option<usize> {
    (0..EQUIP_SLOTS).find(|&slot| data[EQUIP_OFFSET + slot * EQUIP_LEN] == 0)
}

/// The most skills a talisman has (pairs of skill id and points from byte 4). A third pair lands on the decoration sockets: the game
/// then shows a filled slot with no jewel that cannot be emptied (seen with `talisman a +20, b +10, c +5`).
pub const TALISMAN_PAIRS: usize = 2;
/// Equipment kind of a talisman.
const TALISMAN_KIND: u8 = 6;

/// A new talisman in the first empty slot, with these skills (id, points). Its first four bytes (kind, state, id) are copied from a
/// talisman already in the box, since what the state and id bytes mean is not known; the rest of the record is zero but the pairs.
/// Returns the slot and the patch.
pub fn new_talisman(data: &[u8], skills: &[(u8, i8)]) -> Result<(usize, Patch)> {
    if skills.is_empty() || skills.len() > TALISMAN_PAIRS {
        bail!("a talisman takes 1 to {TALISMAN_PAIRS} skills");
    }
    let model = (0..EQUIP_SLOTS)
        .map(|slot| equipment_record(data, slot))
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .find(|r| r[0] == TALISMAN_KIND)
        .ok_or_else(|| anyhow::anyhow!("the box has no talisman to copy the kind and id bytes from; use equip to write a record"))?;
    let slot = first_empty_equipment(data).ok_or_else(|| anyhow::anyhow!("the equipment box is full"))?;
    let mut record = [0u8; EQUIP_LEN];
    record[..4].copy_from_slice(&model[..4]);
    for (k, &(id, points)) in skills.iter().enumerate() {
        record[4 + 2 * k] = id;
        record[5 + 2 * k] = points as u8;
    }
    Ok((
        slot,
        Patch {
            offset: EQUIP_OFFSET + slot * EQUIP_LEN,
            bytes: record.to_vec(),
        },
    ))
}

/// How many of `id` the item box holds.
pub fn box_count(data: &[u8], id: u16) -> u16 {
    (0..BOX_SLOTS)
        .map(|i| BOX_OFFSET + i * 4)
        .filter(|&o| be16(data, o) == id)
        .map(|o| be16(data, o + 2))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::save::Save;

    fn set(d: &mut [u8], id: u16, count: u16) {
        let patch = set_box_item(d, id, count).unwrap();
        apply(d, &patch);
    }

    #[test]
    fn equipment_records_are_read_and_poked_inside_their_slot() {
        let mut d = vec![0u8; SAVE_LEN];
        let at = |slot: usize| EQUIP_OFFSET + slot * EQUIP_LEN;
        d[at(3)] = TALISMAN_KIND;
        d[at(3) + 1] = 9;
        d[at(3) + 2..at(3) + 4].copy_from_slice(&7u16.to_be_bytes());
        assert_eq!(equipment_record(&d, 3).unwrap()[..4], [6, 9, 0, 7]);
        assert!(equipment_record(&d, 1000).is_err());
        let patch = poke_equipment(&d, 3, 4, &[0x25, 0x0a]).unwrap();
        assert_eq!(patch.offset, at(3) + 4);
        assert!(poke_equipment(&d, 3, 15, &[1, 2]).is_err(), "runs past the record");
        assert!(poke_equipment(&d, 3, 0, &[]).is_err());
    }

    #[test]
    fn a_new_talisman_copies_the_header_of_one_in_the_box_and_takes_the_first_empty_slot() {
        let mut d = vec![0u8; SAVE_LEN];
        let at = |slot: usize| EQUIP_OFFSET + slot * EQUIP_LEN;
        d[at(0)] = 5; // some armor in slot 0
        assert!(new_talisman(&d, &[(0x25, 10)]).is_err(), "no talisman to copy from");
        d[at(1)..at(1) + 4].copy_from_slice(&[6, 2, 0, 1]);
        d[at(1) + 4..at(1) + 6].copy_from_slice(&[0x25, 0x0a]);
        let (slot, patch) = new_talisman(&d, &[(0x25, 10), (0x30, -3)]).unwrap();
        assert_eq!(slot, 2);
        assert_eq!(patch.offset, at(2));
        assert_eq!(patch.bytes, [6, 2, 0, 1, 0x25, 10, 0x30, 0xfd, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert!(new_talisman(&d, &[]).is_err());
        assert!(new_talisman(&d, &[(1, 1); 3]).is_err());
    }

    #[test]
    fn zenny_patch_is_three_big_endian_bytes_and_clamped() {
        assert_eq!(
            set_zenny(50_000),
            Patch {
                offset: 0x49,
                bytes: vec![0x00, 0xc3, 0x50]
            }
        );
        let mut d = fixture!("05-live/live_block.bin");
        apply(&mut d, &set_zenny(123_456));
        assert_eq!(Save::parse(&d).unwrap().zenny, 123_456);
        apply(&mut d, &set_zenny(u32::MAX));
        assert_eq!(Save::parse(&d).unwrap().zenny, MAX_ZENNY);
    }

    #[test]
    fn changes_an_existing_stack_in_place() {
        let mut d = fixture!("05-live/live_block.bin");
        assert_eq!(box_count(&d, 175), 8); // Honey, after the capture's 3 were moved to the pouch
        set(&mut d, 175, 40);
        assert_eq!(box_count(&d, 175), 40);
        set(&mut d, 175, 500);
        assert_eq!(box_count(&d, 175), MAX_STACK);
        let save = Save::parse(&d).unwrap();
        assert_eq!(save.item_box.iter().filter(|s| s.id == 175).count(), 1, "no duplicate stack");
    }

    #[test]
    fn adds_a_new_item_in_the_first_empty_slot_and_can_remove_it() {
        let mut d = fixture!("05-live/live_block.bin");
        let before = Save::parse(&d).unwrap().item_box.len();
        let patch = set_box_item(&d, 1000, 60).unwrap(); // an item this box does not hold
        assert_eq!(box_count(&d, 1000), 0);
        apply(&mut d, &patch);
        assert_eq!(box_count(&d, 1000), 60);
        assert_eq!(Save::parse(&d).unwrap().item_box.len(), before + 1);
        set(&mut d, 1000, 0);
        assert_eq!(box_count(&d, 1000), 0);
        assert_eq!(Save::parse(&d).unwrap().item_box.len(), before);
        assert!(set_box_item(&d, 1000, 0).is_err(), "removing something that is absent is an error");
    }

    #[test]
    fn successive_patches_use_different_empty_slots() {
        let mut d = fixture!("05-live/live_block.bin");
        for id in [214, 215, 216] {
            let patch = set_box_item(&d, id, 10).unwrap();
            apply(&mut d, &patch);
        }
        assert_eq!((box_count(&d, 214), box_count(&d, 215), box_count(&d, 216)), (10, 10, 10));
    }

    #[test]
    fn a_full_box_is_an_error() {
        let mut d = fixture!("05-live/live_block.bin");
        for i in 0..BOX_SLOTS {
            let o = BOX_OFFSET + i * 4;
            d[o..o + 4].copy_from_slice(&[0, (i % 200 + 1) as u8, 0, 1]);
        }
        assert!(set_box_item(&d, 999, 5).is_err());
    }
}

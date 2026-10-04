use anyhow::{Result, bail};

pub const SAVE_LEN: usize = 35364;
const NAME_OFFSET: usize = 0x2b;
const NAME_LEN: usize = 0x15;
pub(crate) const ZENNY_OFFSET: usize = 0x49;
const POUCH_OFFSET: usize = 0xd0;
const POUCH_SLOTS: usize = 24;
pub(crate) const BOX_OFFSET: usize = 0x1b0;
pub(crate) const BOX_SLOTS: usize = 1000;
const EQUIP_OFFSET: usize = 0x1150;
const EQUIP_SLOTS: usize = 1000;
const EQUIP_LEN: usize = 16;
/// u16 pointer to the equipment box slot of the worn weapon; 0xffff = none.
const WORN_WEAPON_OFFSET: usize = 0xc0;
/// Five u16 pointers (body, arms, waist, legs, head) into the equipment box slots; 0xffff = nothing worn.
const WORN_OFFSET: usize = 0xc2;
const WORN_SLOTS: usize = 5;
/// One u16 per monster from here; entry `n` is for the monster with name id `n + 6`: how many times it was killed or captured.
/// The guild card: play time in seconds (u32, right after the zenny), and the quests done in the village and in the guild hall (one
/// byte each, in the guild card's own record). Found by comparing saves with the numbers on the guild card, which matched (docs/formats.md).
const PLAY_SECONDS_OFFSET: usize = 0x4c;
const VILLAGE_QUESTS_OFFSET: usize = 0x7a4d;
const GUILD_QUESTS_OFFSET: usize = 0x7a51;
const HUNTED_OFFSET: usize = 0x57a0;
const HUNTED_FIRST_MONSTER: u16 = 6;
const HUNTED_COUNT: usize = 90;

/// A stack of items: `id` indexes the game's item table, `count` is the quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemStack {
    pub id: u16,
    pub count: u16,
}

/// One equipment box slot (16 bytes). `kind` selects the equipment category (armor slot,
/// weapon type, talisman) and `id` indexes that category's name table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Equipment {
    /// Position in the equipment box; worn gear is stored as a pointer to this.
    pub slot: u16,
    pub kind: u8,
    /// Byte 1; its meaning (slot/upgrade state) isn't decoded yet.
    pub upgrade: u8,
    pub id: u16,
    /// Bytes 4..16: a talisman's skills (see [`Equipment::talisman_skills`]); the rest is not decoded.
    pub raw_tail: [u8; 12],
}

impl Equipment {
    /// A talisman's skills as (skill tree id, points): `(id, points)` byte pairs from byte 4, up to a zero id. Only the first pair
    /// has been seen (one Pawn Talisman: Auto-Guard +10, confirmed in game), so further pairs are a guess; anything but a
    /// talisman has none.
    pub fn talisman_skills(&self) -> Vec<(u8, i8)> {
        if self.kind != 6 {
            return Vec::new();
        }
        self.raw_tail[..4]
            .chunks(2)
            .take_while(|p| p[0] != 0)
            .map(|p| (p[0], p[1] as i8))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Save {
    pub hunter_name: String,
    pub zenny: u32,
    /// Time played, in seconds.
    pub play_seconds: u32,
    /// Quests done in the village, and in the guild hall (the guild card's two counts).
    pub village_quests: u8,
    pub guild_quests: u8,
    pub pouch: Vec<ItemStack>,
    pub item_box: Vec<ItemStack>,
    pub equipment_box: Vec<Equipment>,
    /// Monsters hunted, from monster 6 on (see `times_hunted`).
    pub hunted: Vec<u16>,
    /// Equipment box slots currently worn: the weapon first, then armor.
    pub worn_slots: Vec<u16>,
}

fn be16(d: &[u8], o: usize) -> u16 {
    u16::from_be_bytes([d[o], d[o + 1]])
}

/// Read `slots` consecutive (id, count) pairs, dropping empty slots (count 0).
fn read_stacks(d: &[u8], offset: usize, slots: usize) -> Vec<ItemStack> {
    (0..slots)
        .map(|i| ItemStack {
            id: be16(d, offset + i * 4),
            count: be16(d, offset + i * 4 + 2),
        })
        .filter(|s| s.count > 0)
        .collect()
}

fn read_equipment(d: &[u8]) -> Vec<Equipment> {
    (0..EQUIP_SLOTS)
        .map(|i| {
            let o = EQUIP_OFFSET + i * EQUIP_LEN;
            Equipment {
                slot: i as u16,
                kind: d[o],
                upgrade: d[o + 1],
                id: be16(d, o + 2),
                raw_tail: d[o + 4..o + EQUIP_LEN].try_into().unwrap(),
            }
        })
        .filter(|e| e.kind != 0)
        .collect()
}

impl Save {
    /// True if the pouch and box together hold every material of the recipe.
    pub fn can_craft(&self, recipe: &crate::recipes::Recipe) -> bool {
        recipe.materials.iter().all(|m| self.item_count(m.id) >= m.count as u32)
    }

    /// True if this equipment box entry is currently worn.
    pub fn is_worn(&self, e: &Equipment) -> bool {
        self.worn_slots.contains(&e.slot)
    }

    /// True if the equipment box holds this exact piece.
    pub fn owns_equipment(&self, kind: u8, id: u16) -> bool {
        self.equipment_box.iter().any(|e| e.kind == kind && e.id == id)
    }

    /// How many times the monster with this name id (the index in the game's monster names) has been killed or captured. Zero
    /// for monsters the table does not cover.
    pub fn times_hunted(&self, monster: u16) -> u16 {
        monster
            .checked_sub(HUNTED_FIRST_MONSTER)
            .and_then(|i| self.hunted.get(usize::from(i)))
            .copied()
            .unwrap_or(0)
    }

    /// Total quantity of an item across the pouch and the item box.
    pub fn item_count(&self, id: u16) -> u32 {
        self.pouch
            .iter()
            .chain(&self.item_box)
            .filter(|s| s.id == id)
            .map(|s| s.count as u32)
            .sum()
    }

    /// Parse the contents of the `user1` save file.
    /// The play time as the guild card shows it: `1 h 54 min`.
    pub fn play_time(&self) -> String {
        let minutes = self.play_seconds / 60;
        format!("{} h {:02} min", minutes / 60, minutes % 60)
    }

    pub fn parse(d: &[u8]) -> Result<Save> {
        if d.len() != SAVE_LEN {
            bail!("unexpected save size {} (expected {SAVE_LEN})", d.len());
        }
        let name_bytes = &d[NAME_OFFSET..NAME_OFFSET + NAME_LEN];
        let end = name_bytes.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
        Ok(Save {
            hunter_name: String::from_utf8_lossy(&name_bytes[..end]).into_owned(),
            zenny: u32::from_be_bytes([0, d[ZENNY_OFFSET], d[ZENNY_OFFSET + 1], d[ZENNY_OFFSET + 2]]),
            play_seconds: u32::from_be_bytes([
                d[PLAY_SECONDS_OFFSET],
                d[PLAY_SECONDS_OFFSET + 1],
                d[PLAY_SECONDS_OFFSET + 2],
                d[PLAY_SECONDS_OFFSET + 3],
            ]),
            village_quests: d[VILLAGE_QUESTS_OFFSET],
            guild_quests: d[GUILD_QUESTS_OFFSET],
            pouch: read_stacks(d, POUCH_OFFSET, POUCH_SLOTS),
            item_box: read_stacks(d, BOX_OFFSET, BOX_SLOTS),
            equipment_box: read_equipment(d),
            hunted: (0..HUNTED_COUNT).map(|i| be16(d, HUNTED_OFFSET + 2 * i)).collect(),
            worn_slots: std::iter::once(be16(d, WORN_WEAPON_OFFSET))
                .chain((0..WORN_SLOTS).map(|i| be16(d, WORN_OFFSET + i * 2)))
                .filter(|&s| s != 0xffff)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::Recipe;

    fn stack(id: u16, count: u16) -> ItemStack {
        ItemStack { id, count }
    }

    fn save(pouch: Vec<ItemStack>, item_box: Vec<ItemStack>) -> Save {
        Save {
            hunter_name: String::new(),
            zenny: 0,
            play_seconds: 0,
            village_quests: 0,
            guild_quests: 0,
            pouch,
            item_box,
            equipment_box: Vec::new(),
            hunted: Vec::new(),
            worn_slots: Vec::new(),
        }
    }

    #[test]
    fn the_guild_card_fields_are_read_from_their_places() {
        let mut d = vec![0u8; SAVE_LEN];
        d[PLAY_SECONDS_OFFSET..PLAY_SECONDS_OFFSET + 4].copy_from_slice(&6863u32.to_be_bytes());
        d[VILLAGE_QUESTS_OFFSET] = 6;
        d[GUILD_QUESTS_OFFSET] = 1;
        let s = Save::parse(&d).unwrap();
        assert_eq!((s.play_seconds, s.village_quests, s.guild_quests), (6863, 6, 1));
        assert_eq!(s.play_time(), "1 h 54 min");
        assert_eq!(Save { play_seconds: 59, ..s }.play_time(), "0 h 00 min");
    }

    /// Real saves, with the numbers read off the guild cards in the game: one hunter has 2 h 10 min and no quests; the other 2 h 20 min,
    /// 8 village and 2 guild quests (snapshot 11), and a few quests fewer in snapshot 09.
    #[test]
    fn the_guild_card_numbers_of_real_saves_are_where_they_should_be() {
        let read = |snapshot: &str, user: &str| {
            std::fs::read(format!("{}/../../snapshots/{snapshot}/{user}", env!("CARGO_MANIFEST_DIR")))
                .ok()
                .map(|d| Save::parse(&d).unwrap())
        };
        let card = |s: &Save| (s.play_time(), s.village_quests, s.guild_quests);
        if let Some(shamus) = read("09-hr1-worntester", "user1") {
            assert_eq!(card(&shamus), ("2 h 10 min".to_string(), 0, 0));
        }
        if let Some(earlier) = read("09-hr1-worntester", "user2") {
            assert_eq!(card(&earlier), ("1 h 49 min".to_string(), 6, 1));
        }
        if let Some(later) = read("11-hall-quest", "user2") {
            assert_eq!(card(&later), ("2 h 20 min".to_string(), 8, 2));
        }
        if let Some(latest) = read("12-hall-quest-2", "user2") {
            assert_eq!(card(&latest), ("2 h 40 min".to_string(), 8, 3));
        }
    }

    #[test]
    fn a_talisman_record_gives_its_skill_and_points() {
        let mut tail = [0; 12];
        tail[..2].copy_from_slice(&[0x25, 0x0a]);
        let e = |kind| Equipment {
            slot: 0,
            kind,
            upgrade: 0,
            id: 1,
            raw_tail: tail,
        };
        assert_eq!(e(6).talisman_skills(), vec![(0x25, 10)]);
        assert!(e(1).talisman_skills().is_empty(), "armor has no talisman skills");
    }

    #[test]
    fn hunt_counts_are_indexed_by_monster_name_id() {
        let mut d = vec![0u8; SAVE_LEN];
        d[HUNTED_OFFSET + 2 * 6 + 1] = 1; // entry 6: monster 12, Great Jaggi
        d[HUNTED_OFFSET + 2 * 36 + 1] = 3; // entry 36: monster 42, Arzuros
        let save = Save::parse(&d).unwrap();
        assert_eq!(save.times_hunted(12), 1);
        assert_eq!(save.times_hunted(42), 3);
        assert_eq!(save.times_hunted(10), 0);
        assert_eq!(save.times_hunted(3), 0, "below the first monster in the table");
        assert_eq!(save.times_hunted(500), 0, "past the table");
    }

    #[test]
    fn counts_span_pouch_and_box() {
        let s = save(vec![stack(214, 1)], vec![stack(214, 2), stack(216, 3)]);
        assert_eq!(s.item_count(214), 3);
        assert_eq!(s.item_count(999), 0);
    }

    #[test]
    fn can_craft_needs_every_material() {
        let recipe = Recipe {
            materials: vec![stack(214, 3), stack(216, 3)],
            flag: 1,
            tier: 1,
        };
        assert!(save(vec![stack(214, 1)], vec![stack(214, 2), stack(216, 3)]).can_craft(&recipe));
        assert!(!save(vec![], vec![stack(214, 3), stack(216, 2)]).can_craft(&recipe));
    }

    fn worn(save: &Save) -> Vec<(u8, u16)> {
        let mut w: Vec<_> = save
            .equipment_box
            .iter()
            .filter(|e| save.is_worn(e))
            .map(|e| (e.kind, e.id))
            .collect();
        w.sort();
        w
    }

    #[test]
    fn parses_worn_gear_from_real_save() {
        // Taken after taking off everything except the Leather Headgear (slot 12); the Iron Sword (slot 0) is the weapon.
        let save = Save::parse(&fixture!("03-latest/user1")).unwrap();
        assert_eq!(save.worn_slots, vec![0, 12]);
        assert_eq!(worn(&save), vec![(5, 1), (7, 1)]);
    }

    #[test]
    fn reads_the_hunter_name_up_to_the_first_nul() {
        let mut d = vec![0u8; SAVE_LEN];
        d[NAME_OFFSET..NAME_OFFSET + 6].copy_from_slice(b"Tester");
        d[NAME_OFFSET + 7] = b'x'; // after the terminator, ignored
        assert_eq!(Save::parse(&d).unwrap().hunter_name, "Tester");
    }

    #[test]
    fn parses_worn_bow_from_second_save_slot() {
        // The hunter in the second save slot has the Hunter's Bow I (kind 17, id 33, slot 11) equipped plus all five leather pieces.
        let save = Save::parse(&fixture!("04-worntester/user2")).unwrap();
        assert!(!save.hunter_name.is_empty());
        assert_eq!(save.worn_slots[0], 11);
        assert_eq!(worn(&save), vec![(1, 1), (2, 1), (3, 1), (4, 1), (5, 1), (17, 33)]);
    }
}

use anyhow::{Result, bail};

pub const SAVE_LEN: usize = 35364;
const NAME_OFFSET: usize = 0x2b;
const NAME_LEN: usize = 0x15;
pub(crate) const ZENNY_OFFSET: usize = 0x49;
pub(crate) const POUCH_OFFSET: usize = 0xd0;
pub(crate) const POUCH_SLOTS: usize = 24;
pub(crate) const BOX_OFFSET: usize = 0x1b0;
pub(crate) const BOX_SLOTS: usize = 1000;
pub(crate) const EQUIP_OFFSET: usize = 0x1150;
pub(crate) const EQUIP_SLOTS: usize = 1000;
pub(crate) const EQUIP_LEN: usize = 16;
/// u16 pointer to the equipment box slot of the worn weapon; 0xffff = none.
pub(crate) const WORN_WEAPON_OFFSET: usize = 0xc0;
/// Five u16 pointers (body, arms, waist, legs, head) into the equipment box slots; 0xffff = nothing worn.
pub(crate) const WORN_OFFSET: usize = 0xc2;
pub(crate) const WORN_SLOTS: usize = 5;
/// The u16 pointer, right after the five armor ones, to the worn talisman's equipment box slot; 0xffff = none. The 16 bytes before the
/// pointers (from 0xb0) are a copy of the worn talisman's record (kind 6 and zeros when none is worn). Found by comparing a save with
/// a talisman worn (slot 17) and one without.
pub(crate) const WORN_TALISMAN_OFFSET: usize = 0xcc;
/// The guild card: play time in seconds (u32, right after the zenny), and the quests done in the village and in the guild hall (one
/// byte each, in the guild card's own record). Found by comparing saves with the numbers on the guild card, which matched (docs/formats.md).
const PLAY_SECONDS_OFFSET: usize = 0x4c;
/// The guild card's weapon usage: twelve u16 counts of the quests done with each weapon type, first for the village (from here) and then
/// for the guild hall (twelve u16 from `0x7b60`). The screen shows the two added together. Found by doing a Sword & Shield hall quest
/// after four with the great sword (only the second type's hall count moved), then a hammer hall quest (only the third moved). The first
/// three types are certain (great sword, sword & shield, hammer); the order of the rest is the weapon menu's by guess and not checked.
const WEAPON_USAGE_VILLAGE: usize = 0x7b48;
const WEAPON_USAGE_GUILD: usize = 0x7b60;
/// The equipment kind of each of the twelve types, in that order.
pub const WEAPON_USAGE_KINDS: [u8; 12] = [7, 8, 9, 10, 11, 13, 14, 15, 16, 17, 18, 19];

/// The guild card's title is two big-endian u32 from here: the first is a word (its number in the game's `CardTitle` text, "Noob" is 1 and
/// "Fledgling" 4); in the second the top byte joins the words (0: nothing, else the word numbered 612 plus it: 6 is "to", 1 "of", 2 "and")
/// and the rest is the second word ("Hunter" is 6, "Excited" 51). Found by changing the title from "Fledgling Hunter" to "Noob to Excited".
const CARD_TITLE: usize = 0x7a28;
/// The greeting on the card: text up to a NUL from here. Its longest length is not known.
const CARD_GREETING: usize = 0x7ad0;
const GREETING_MAX: usize = 48;
/// The hunter rank: a big-endian u16 at 0x5a48, next to the quest points at 0x5a46. 0 for a hunter who never went to the guild hall,
/// 1 for WornTester until the urgent quest "The Fisherman's Friend", 2 after it (the card agreed each time).
const HUNTER_RANK_OFFSET: usize = 0x5a48;
const VILLAGE_QUESTS_OFFSET: usize = 0x7a4d;
const GUILD_QUESTS_OFFSET: usize = 0x7a51;
/// One u16 per monster from here; entry `n` is for the monster with name id `n + 6`: how many times it was killed or captured.
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

impl Equipment {
    /// A talisman's gem slots (0 to 3): the record's second byte. Found by writing 3 into it in the running game, which gave the
    /// talisman 3 slots; a Pawn Talisman has 0. Anything but a talisman has none (its second byte is its upgrade state).
    pub fn talisman_slots(&self) -> u8 {
        if self.kind == 6 { self.upgrade.min(3) } else { 0 }
    }

    /// The decorations socketed in a talisman, as numbers into the game's decoration table (see `decorations`), one per slot in
    /// order. Seen so far: slot 1 of a 3-slot talisman held `0x0091` at bytes 8-9; slot 2 held `0x0015` at bytes 10-11 (confirmed); slot 3 held `0x0097` at bytes 12-13 (confirmed). Anything but a talisman has none here (see
    /// [`Equipment::decorations`]).
    pub fn talisman_decorations(&self) -> Vec<u16> {
        if self.kind != 6 {
            return Vec::new();
        }
        self.raw_tail[4..10]
            .chunks(2)
            .take(usize::from(self.talisman_slots()))
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect()
    }

    /// The decorations socketed in an armor piece or talisman, numbers into the game's decoration table, empty sockets skipped. An armor
    /// piece keeps them in the same place as a talisman (bytes 8-9 for a jewel in its first slot, seen on a Leather Headgear).
    pub fn decorations(&self) -> Vec<u16> {
        if !(1..=6).contains(&self.kind) {
            return Vec::new();
        }
        self.raw_tail[4..10]
            .chunks(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .filter(|&c| c != 0)
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
    /// The hunter rank (HR) on the guild card: 0 before the guild hall, then 1 and up.
    pub hunter_rank: u16,
    /// Quests done with each weapon type, village and guild together, in the order of [`WEAPON_USAGE_KINDS`].
    pub weapon_uses: [u32; 12],
    /// The guild card's title as numbers into the game's title words: (first word, joining word, second word); see [`CARD_TITLE`].
    pub card_title: (u32, u8, u32),
    /// The guild card's greeting.
    pub greeting: String,
    pub pouch: Vec<ItemStack>,
    pub item_box: Vec<ItemStack>,
    pub equipment_box: Vec<Equipment>,
    /// Monsters hunted, from monster 6 on (see `times_hunted`).
    pub hunted: Vec<u16>,
    /// Equipment box slots currently worn: the weapon first, then armor.
    pub worn_slots: Vec<u16>,
    /// The equipment box slot of the worn talisman.
    pub worn_talisman: Option<u16>,
}

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
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
        self.worn_slots.contains(&e.slot) || self.worn_talisman == Some(e.slot)
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
    /// The highest rank of quest the hunter's hunter rank gives access to: low rank up to HR2, high rank for HR3 to HR5, G rank from HR6
    /// (the tiers a player described: low rank 1★ and 2★ hall quests, high rank 3★ to 5★, G rank 6★ to 8★).
    pub fn reached_rank(&self) -> crate::drops::Rank {
        match self.hunter_rank {
            0..=2 => crate::drops::Rank::Low,
            3..=5 => crate::drops::Rank::High,
            _ => crate::drops::Rank::G,
        }
    }

    /// The weapon type the hunter has done the most quests with, as (equipment kind, quests); `None` before the first quest.
    pub fn most_used_weapon(&self) -> Option<(u8, u32)> {
        self.weapon_uses
            .iter()
            .enumerate()
            .filter(|&(_, &n)| n > 0)
            .max_by_key(|&(i, &n)| (n, std::cmp::Reverse(i)))
            .map(|(i, &n)| (WEAPON_USAGE_KINDS[i], n))
    }

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
            hunter_rank: be16(d, HUNTER_RANK_OFFSET),
            weapon_uses: std::array::from_fn(|i| {
                u32::from(be16(d, WEAPON_USAGE_VILLAGE + 2 * i)) + u32::from(be16(d, WEAPON_USAGE_GUILD + 2 * i))
            }),
            card_title: {
                let (first, second) = (be32(d, CARD_TITLE), be32(d, CARD_TITLE + 4));
                (first & 0x00ff_ffff, (second >> 24) as u8, second & 0x00ff_ffff)
            },
            greeting: {
                let text = &d[CARD_GREETING..CARD_GREETING + GREETING_MAX];
                let end = text.iter().position(|&b| b == 0).unwrap_or(GREETING_MAX);
                String::from_utf8_lossy(&text[..end]).into_owned()
            },
            pouch: read_stacks(d, POUCH_OFFSET, POUCH_SLOTS),
            item_box: read_stacks(d, BOX_OFFSET, BOX_SLOTS),
            equipment_box: read_equipment(d),
            hunted: (0..HUNTED_COUNT).map(|i| be16(d, HUNTED_OFFSET + 2 * i)).collect(),
            worn_slots: std::iter::once(be16(d, WORN_WEAPON_OFFSET))
                .chain((0..WORN_SLOTS).map(|i| be16(d, WORN_OFFSET + i * 2)))
                .filter(|&s| s != 0xffff)
                .collect(),
            worn_talisman: Some(be16(d, WORN_TALISMAN_OFFSET)).filter(|&s| s != 0xffff),
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
            hunter_rank: 0,
            weapon_uses: [0; 12],
            card_title: (0, 0, 0),
            greeting: String::new(),
            pouch,
            item_box,
            equipment_box: Vec::new(),
            hunted: Vec::new(),
            worn_slots: Vec::new(),
            worn_talisman: None,
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

    /// Snapshot 12 has the Pawn Talisman worn (equipment slot 17); in snapshot 13 it was taken off and a talisman with two skills and
    /// three slots written into slot 0.
    #[test]
    fn the_worn_talisman_is_the_slot_the_save_points_to() {
        let worn = Save::parse(&fixture!("12-hall-quest-2/user2")).unwrap();
        assert_eq!(worn.worn_talisman, Some(17));
        let pawn = worn.equipment_box.iter().find(|e| e.slot == 17).unwrap();
        assert_eq!((pawn.kind, pawn.talisman_skills(), pawn.talisman_slots()), (6, vec![(0x25, 10)], 0));
        assert!(worn.is_worn(pawn));
        let after = Save::parse(&fixture!("13-talisman/user2")).unwrap();
        assert_eq!(after.worn_talisman, None);
        let made = after.equipment_box.iter().find(|e| e.slot == 0).unwrap();
        assert_eq!(
            (made.kind, made.talisman_skills(), made.talisman_slots()),
            (6, vec![(0x25, 10), (0x63, 5)], 3)
        );
        assert!(!after.is_worn(made));
        let jewel = Save::parse(&fixture!("14-jewel/user2")).unwrap();
        let socketed = jewel.equipment_box.iter().find(|e| e.slot == 0).unwrap();
        assert_eq!(socketed.talisman_decorations(), vec![0x91, 0, 0]);
        assert_eq!(made.talisman_decorations(), vec![0, 0, 0]);
        let two = Save::parse(&fixture!("15-jewel2/user2")).unwrap();
        let both = two.equipment_box.iter().find(|e| e.slot == 0).unwrap();
        assert_eq!(both.talisman_decorations(), vec![0x91, 0x15, 0]);
        let armor = Save::parse(&fixture!("17-armor-jewel/user2")).unwrap();
        let head = armor.equipment_box.iter().find(|e| e.slot == 0).unwrap();
        assert_eq!((head.kind, head.id, head.decorations()), (5, 1, vec![0x91]));
        let hood = Save::parse(&fixture!("19-armor3/user2")).unwrap();
        let worn_head = hood.equipment_box.iter().find(|e| e.slot == 20).unwrap();
        assert_eq!((worn_head.kind, worn_head.decorations()), (5, vec![0x91, 0x91, 0x91]));
        assert!(hood.is_worn(worn_head));
        let three = Save::parse(&fixture!("16-jewel3/user2")).unwrap();
        let full = three.equipment_box.iter().find(|e| e.slot == 0).unwrap();
        assert_eq!(full.talisman_decorations(), vec![0x91, 0x15, 0x97]);
    }

    #[test]
    fn the_hunter_rank_is_read() {
        assert_eq!(
            Save::parse(&fixture!("03-latest/user1")).unwrap().hunter_rank,
            0,
            "Shamus has not been to the hall"
        );
        assert_eq!(Save::parse(&fixture!("09-hr1-worntester/user2")).unwrap().hunter_rank, 1);
        assert_eq!(
            Save::parse(&fixture!("22-hammer/user2")).unwrap().hunter_rank,
            1,
            "before The Fisherman's Friend"
        );
        assert_eq!(Save::parse(&fixture!("23-hr2/user2")).unwrap().hunter_rank, 2);
    }

    #[test]
    fn the_reached_rank_follows_the_hunter_rank() {
        use crate::drops::Rank;
        let mut s = Save::parse(&fixture!("23-hr2/user2")).unwrap();
        assert_eq!(s.reached_rank(), Rank::Low);
        for (hr, rank) in [
            (0, Rank::Low),
            (2, Rank::Low),
            (3, Rank::High),
            (5, Rank::High),
            (6, Rank::G),
            (8, Rank::G),
        ] {
            s.hunter_rank = hr;
            assert_eq!(s.reached_rank(), rank, "HR{hr}");
        }
    }

    #[test]
    fn the_guild_card_title_and_greeting_are_read() {
        let before = Save::parse(&fixture!("20-snS-quest/user2")).unwrap();
        assert_eq!((before.card_title, before.greeting.as_str()), ((4, 0, 6), "Hello there!"));
        let after = Save::parse(&fixture!("21-title/user2")).unwrap();
        assert_eq!((after.card_title, after.greeting.as_str()), ((1, 6, 51), "Bobby the boy"));
    }

    #[test]
    fn weapon_usage_adds_the_village_and_the_hall_counts() {
        // after the Sword & Shield hall quest the screen showed great sword 14 and sword & shield 1
        let after = Save::parse(&fixture!("20-snS-quest/user2")).unwrap();
        assert_eq!(after.weapon_uses[..3], [14, 1, 0]);
        assert_eq!(after.most_used_weapon(), Some((7, 14)));
        // and a hammer hall quest after that moved the third entry
        let hammer = Save::parse(&fixture!("22-hammer/user2")).unwrap();
        assert_eq!(hammer.weapon_uses[..4], [14, 1, 1, 0]);
        let before = Save::parse(&fixture!("12-hall-quest-2/user2")).unwrap();
        assert_eq!(
            before.weapon_uses[..2],
            [14, 0],
            "the same great sword count, no sword & shield yet"
        );
        // the first quests of the series: one more with each village (+2 in that save's counter) and hall quest
        let village = Save::parse(&fixture!("10-village-quest/user2")).unwrap();
        assert_eq!(village.weapon_uses[0], 12);
        assert_eq!(Save::parse(&fixture!("11-hall-quest/user2")).unwrap().weapon_uses[0], 13);
        let none = Save::parse(&fixture!("03-latest/user1")).unwrap();
        assert_eq!(
            (none.weapon_uses, none.most_used_weapon()),
            ([0; 12], None),
            "a hunter with no quests"
        );
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

//! Static game data (names, descriptions) read from the user's own game dump.

use crate::{
    arc::Arc,
    armor::{self, ArmorStats},
    gmd,
    recipes::{self, Recipe, Upgrade},
    rpx,
};
use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

/// English text for items and equipment lives in this archive, relative to the dump's game folder.
const TEXT_ARCHIVE: &str = "content/nativeCafe/arc/ID/ID_arena_eng.arc";
/// The archive with the guild card's text.
/// Where the Shakalaka masks' names (25, from the Acorn Mask) and descriptions (the same order) start in the `Facility_eng` text.
const MASK_NAMES: usize = 372;
const MASK_DESCRIPTIONS: usize = 397;
const LOBBY_ARCHIVE: &str = "content/nativeCafe/arc/ID/ID_lb_eng.arc";

/// Equipment `kind` byte -> (display name, GMD file holding that kind's piece names).
/// Names are internal to the game's files; e.g. the `Lsword` file holds the Great Swords.
const EQUIPMENT_KINDS: &[(u8, &str, &str)] = &[
    (1, "Body", "Body_eng"),
    (2, "Arms", "Arm_eng"),
    (3, "Waist", "Waist_eng"),
    (4, "Legs", "Leg_eng"),
    (5, "Head", "Helm_eng"),
    (6, "Talisman", "Acce_eng"),
    (7, "Great Sword", "Lsword_eng"),
    (8, "Sword & Shield", "Sword_eng"),
    (9, "Hammer", "Hammer_eng"),
    (10, "Lance", "Lance_eng"),
    (11, "Heavy Bowgun", "Hbg_eng"),
    (13, "Light Bowgun", "Lbg_eng"),
    (14, "Long Sword", "Lsword2_eng"),
    (15, "Switch Axe", "Axe_eng"),
    (16, "Gunlance", "Gunlance_eng"),
    (17, "Bow", "Bow_eng"),
    (18, "Dual Blades", "WSword_eng"),
    (19, "Hunting Horn", "Pipe_eng"),
];

/// The game's executable: the `.rpx` in the dump's `code` folder.
pub fn rpx_path(game_dir: &Path) -> Result<std::path::PathBuf> {
    std::fs::read_dir(game_dir.join("code"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.extension().is_some_and(|x| x == "rpx"))
        .with_context(|| format!("no .rpx in {}", game_dir.join("code").display()))
}

/// The names and descriptions of one kind of equipment, indexed by piece id.
struct EquipmentTable {
    label: &'static str,
    names: Vec<String>,
    /// Empty when the dump lacks the descriptions.
    details: Vec<String>,
}

/// A recipe that uses an item: this piece (a create or an upgrade recipe) takes `count` of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialUse {
    pub kind: u8,
    pub id: u16,
    pub count: u16,
}

pub struct GameData {
    items: Vec<String>,
    /// Text tables the game ships for descriptions; empty when a dump lacks them.
    item_details: Vec<String>,
    skill_details: Vec<String>,
    /// Names and descriptions of skill effects (Attack Up (L)), by effect id.
    effect_names: Vec<String>,
    effect_details: Vec<String>,
    monsters: Vec<String>,
    /// The Hunter's Notes (see `notes`); empty when a dump lacks them.
    hunters_notes: Vec<String>,
    recipes: HashMap<(u8, u16), Recipe>,
    upgrades: HashMap<(u8, u16), Upgrade>,
    armor: HashMap<(u8, u16), ArmorStats>,
    weapons: HashMap<(u8, u16), crate::weapons::Weapon>,
    drops: crate::drops::Drops,
    /// Names of hit zones, by (monster, row of its first table).
    zone_names: HashMap<(u16, usize), crate::zone_names::Entry>,
    /// Where items can be gathered, by item id.
    gather_spots: HashMap<u16, Vec<crate::gather_spots::Spot>>,
    /// The words a guild card title is put together from, by number (`CardTitle` in the lobby archive).
    card_title_words: Vec<String>,
    /// The lobby's facility texts, among them the Shakalaka masks' names and descriptions.
    facility_texts: Vec<String>,
    /// Carry limits and shop prices of items, by item id.
    item_extras: HashMap<u16, crate::item_extras::ItemExtras>,
    /// Songs of the hunting horns, by the horn's notes.
    horn_songs: Vec<(Vec<String>, crate::horn_songs::Song)>,
    /// Sharpness and element of the melee weapons, by (kind, id).
    weapon_extras: HashMap<(u8, u16), crate::weapon_extras::Extras>,
    /// What each decoration does, indexed by the number a save keeps for it, minus one.
    decorations: Vec<crate::decorations::Decoration>,
    /// What a shop pays for each item, by item id.
    sell_prices: Vec<u32>,
    /// Piece ids of each equipment kind's recipe table, in row order (the order of the blacksmith's menu).
    recipe_rows: HashMap<u8, Vec<u16>>,
    quests: Vec<crate::quest::Quest>,
    skills: Vec<String>,
    equipment: HashMap<u8, EquipmentTable>,
    game_dir: std::path::PathBuf,
    /// Hit zones read so far; each monster's archive is opened the first time it is asked for.
    zones: std::sync::Mutex<HashMap<u16, std::sync::Arc<Vec<crate::hitzones::Zone>>>>,
}

/// Where the English quest files are, relative to the dump's game folder.
const QUEST_DIR: &str = "content/nativeCafe/quest/us";

/// Every quest in the dump, by id. A dump without them gives none, and a file that does not read is skipped.
fn load_quests(game_dir: &Path) -> Vec<crate::quest::Quest> {
    let Ok(files) = std::fs::read_dir(game_dir.join(QUEST_DIR)) else {
        return Vec::new();
    };
    let mut quests: Vec<crate::quest::Quest> = files
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "quest"))
        .filter_map(|p| crate::quest::parse(&std::fs::read(p).ok()?).ok())
        .filter(|q| q.title != "DUMMY" && !q.title.is_empty())
        .collect();
    quests.sort_by_key(|q| q.id);
    quests
}

/// The texts of one entry of the lobby text archive (`CardTitle_eng`, `Facility_eng`), by number; empty when the dump has none.
fn lobby_texts(game_dir: &Path, entry: &str) -> Vec<String> {
    let read = || -> Result<Vec<String>> {
        let bytes = std::fs::read(game_dir.join(LOBBY_ARCHIVE))?;
        let arc = Arc::parse(&bytes)?;
        let name = format!("GUI\\font\\lobby\\{entry}");
        let entry = arc
            .entries
            .iter()
            .find(|e| e.name == name)
            .with_context(|| format!("{name} not found in the lobby archive"))?;
        Ok(gmd::parse(&arc.read(entry)?)?.into_iter().map(|w| w.trim().to_string()).collect())
    };
    read().unwrap_or_default()
}

/// Join the game's hard-wrapped text lines into one line. A line ending in a hyphen joins to the next with no space.
fn unwrap_text(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines().map(str::trim_end) {
        if !out.is_empty() && !out.ends_with('-') {
            out.push(' ');
        }
        out.push_str(line.trim_start());
    }
    out
}

/// The game's title id (US).
pub const TITLE_ID: &str = "0005000010118300";

/// A path as typed, with a leading `~` meaning the home folder.
pub fn expand_home(text: &str) -> PathBuf {
    let text = text.trim();
    if let Some(rest) = text.strip_prefix("~/").or_else(|| (text == "~").then_some(""))
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(text)
}

/// The game dump a folder is or holds: the folder itself when it has the game's `content` (a dump), else the dump inside it whose name
/// has `[Game]` and the game's title id, so that a folder of dumps works as well as one dump.
pub fn find_dump(folder: &Path) -> Option<PathBuf> {
    let is_dump = |p: &Path| p.join("content/nativeCafe").is_dir();
    if is_dump(folder) {
        return Some(folder.to_path_buf());
    }
    std::fs::read_dir(folder).ok()?.filter_map(|e| e.ok().map(|e| e.path())).find(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("[Game]") && n.contains(TITLE_ID))
            && is_dump(p)
    })
}

/// The dump named by the `MH3U_GAME_DIR` environment variable, for the tests that need the game's files: they skip when it is not set.
pub fn dump_from_env() -> Option<PathBuf> {
    find_dump(&expand_home(&std::env::var("MH3U_GAME_DIR").ok()?))
}

impl GameData {
    /// `game_dir` is the dump folder containing `code`, `content` and `meta`.
    pub fn load(game_dir: &Path) -> Result<GameData> {
        let path = game_dir.join(TEXT_ARCHIVE);
        let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
        let arc = Arc::parse(&bytes)?;
        let strings = |file: &str| -> Result<Vec<String>> {
            let want = format!("GUI\\font\\{file}");
            let entry = arc
                .entries
                .iter()
                .find(|e| e.name == want)
                .with_context(|| format!("{want} not found in archive"))?;
            gmd::parse(&arc.read(entry)?).with_context(|| want)
        };
        // names sometimes carry trailing spaces in the game's text
        let names = |file: &str| -> Result<Vec<String>> { Ok(strings(file)?.into_iter().map(|n| n.trim().to_string()).collect()) };
        // Descriptions are a nicety: a dump without them still works.
        let optional = |file: &str| {
            strings(file)
                .map(|v| v.iter().map(|t| unwrap_text(t)).collect())
                .unwrap_or_default()
        };
        let equipment = EQUIPMENT_KINDS
            .iter()
            .map(|&(kind, label, file)| {
                let table = EquipmentTable {
                    label,
                    names: names(file)?,
                    details: optional(&file.replace("_eng", "_Exp_eng")),
                };
                Ok((kind, table))
            })
            .collect::<Result<_>>()?;
        let rpx_path = rpx_path(game_dir)?;
        let rpx_bytes = std::fs::read(&rpx_path).with_context(|| format!("reading {}", rpx_path.display()))?;
        let data_section = rpx::section_at(&rpx_bytes, recipes::DATA_SECTION_ADDR)?;
        let (recipes, upgrades) = (recipes::parse(&data_section)?, recipes::parse_upgrades(&data_section)?);
        let armor = armor::parse(&data_section)?;
        let weapons = crate::weapons::parse(&data_section)?;
        let drops = crate::drops::parse(&data_section, recipes::DATA_SECTION_ADDR)?;
        let mut game = GameData {
            items: names("Item00_eng")?,
            item_details: optional("ItemDetail_eng"),
            skill_details: optional("Skill_Type_Exp_eng"),
            effect_names: names("Skill_eng").unwrap_or_default(),
            effect_details: optional("Skill_Exp_eng"),
            monsters: names("Monster_eng").unwrap_or_default(),
            hunters_notes: optional("HNote_eng"),
            recipes,
            upgrades,
            armor,
            weapons,
            drops,
            decorations: crate::decorations::parse(&data_section)?,
            weapon_extras: HashMap::new(),
            horn_songs: crate::horn_songs::parse()?,
            item_extras: HashMap::new(),
            card_title_words: lobby_texts(game_dir, "CardTitle_eng"),
            facility_texts: lobby_texts(game_dir, "Facility_eng"),
            gather_spots: HashMap::new(),
            zone_names: crate::zone_names::parse()?,
            sell_prices: crate::items::parse_sell_prices(&data_section)?,
            recipe_rows: recipes::row_order(&data_section),
            quests: load_quests(game_dir),
            skills: names("Skill_Type_eng")?,
            equipment,
            game_dir: game_dir.to_path_buf(),
            zones: Default::default(),
        };
        game.weapon_extras = crate::weapon_extras::parse()?
            .into_iter()
            .filter(|((kind, id), (name, _))| game.piece_name(*kind, *id) == Some(name.as_str()))
            .map(|(key, (_, extras))| (key, extras))
            .collect();
        game.item_extras = crate::item_extras::parse()?
            .into_iter()
            .filter(|(id, (name, _))| game.item_name(*id) == Some(name.as_str()))
            .map(|(id, (_, extras))| (id, extras))
            .collect();
        game.gather_spots = crate::gather_spots::parse()?
            .into_iter()
            .filter(|(id, (name, _))| game.item_name(*id) == Some(name.as_str()))
            .map(|(id, (_, spots))| (id, spots))
            .collect();
        Ok(game)
    }

    /// The hit zones of a monster's normal state, read from its archive on first use; empty when the dump has none for it.
    pub fn hit_zones(&self, monster: u16) -> std::sync::Arc<Vec<crate::hitzones::Zone>> {
        let mut cache = self.zones.lock().unwrap_or_else(|e| e.into_inner());
        cache
            .entry(monster)
            .or_insert_with(|| std::sync::Arc::new(self.read_zones(monster)))
            .clone()
    }

    /// The name of a hit zone (a row of the monster's first table), when the table has it for this monster and these numbers.
    pub fn zone_name(&self, monster: u16, row: usize) -> Option<&str> {
        let entry = self.zone_names.get(&(monster, row))?;
        if self.monster_name(monster)? != entry.monster {
            return None;
        }
        let zone = self.hit_zones(monster).get(row).copied()?;
        let values = [
            zone.cut,
            zone.impact,
            zone.shot,
            zone.fire,
            zone.water,
            zone.ice,
            zone.thunder,
            zone.dragon,
        ];
        (values == entry.values).then_some(entry.name.as_str())
    }

    fn read_zones(&self, monster: u16) -> Vec<crate::hitzones::Zone> {
        let path = self.game_dir.join(format!("content/nativeCafe/arc/enemy/em{monster:03}.arc"));
        let Ok(bytes) = std::fs::read(path) else { return Vec::new() };
        let Ok(arc) = Arc::parse(&bytes) else { return Vec::new() };
        let want = format!("enemy\\em{monster:03}\\em_status00");
        let Some(entry) = arc.entries.iter().find(|e| e.name == want) else {
            return Vec::new();
        };
        arc.read(entry).map(|d| crate::hitzones::parse(&d)).unwrap_or_default()
    }

    /// The pieces of a kind in the order of the game's recipe table, which is the order the blacksmith's menu lists them in.
    pub fn recipe_rows(&self, kind: u8) -> &[u16] {
        self.recipe_rows.get(&kind).map_or(&[], Vec::as_slice)
    }

    /// What a drop list is called: its method, or for a part break the part it is when  knows it ("Head break"). The part
    /// is only given when the monster has as many break lists in that rank as the table has parts, so a rank with extra lists gets
    /// "Part break N" for all.
    pub fn drop_label(&self, monster: u16, rank: crate::drops::Rank, method: crate::drops::Method) -> String {
        use crate::drops::Method;
        if let Method::Break(n) = method
            && let Some(parts) = crate::breakparts::parts(monster)
        {
            let lists = self
                .drops
                .lists_for(monster)
                .filter(|&(m, r, _)| r == rank && matches!(m, Method::Break(_)))
                .count();
            if lists == parts.len()
                && let Some(part) = parts.get(usize::from(n).wrapping_sub(1))
            {
                return format!("{} break", part.label());
            }
        }
        method.label()
    }

    /// The decoration a save's number stands for (the number counts from 1; 0 is an empty socket).
    pub fn decoration(&self, code: u16) -> Option<&crate::decorations::Decoration> {
        self.decorations.get(usize::from(code).checked_sub(1)?)
    }

    /// Every decoration, in the game's table order.
    pub fn decoration_table(&self) -> &[crate::decorations::Decoration] {
        &self.decorations
    }

    /// The skill points a set of socketed decorations (save numbers, 0 = empty) add up to, penalties as negative points.
    pub fn decoration_points(&self, codes: &[u16]) -> Vec<(u8, i8)> {
        let mut out = Vec::new();
        for d in codes.iter().filter_map(|&c| self.decoration(c)) {
            out.push((d.skill, d.points));
            out.extend(d.penalty);
        }
        out
    }

    /// The songs a hunting horn can play, from its notes in the weapon table (none for other weapons).
    pub fn horn_songs(&self, kind: u8, id: u16) -> Vec<&crate::horn_songs::Song> {
        match self.weapon_extras(kind, id).map(|e| &e.part) {
            Some(crate::weapon_extras::Part::Notes(notes)) => crate::horn_songs::songs_for(&self.horn_songs, notes),
            _ => Vec::new(),
        }
    }

    /// A melee weapon's sharpness bars and elements, when the table has the weapon under this name.
    pub fn weapon_extras(&self, kind: u8, id: u16) -> Option<&crate::weapon_extras::Extras> {
        self.weapon_extras.get(&(kind, id))
    }

    /// Where an item can be gathered (mined, picked, caught, fished); empty for items that cannot.
    pub fn gather_spots(&self, id: u16) -> &[crate::gather_spots::Spot] {
        self.gather_spots.get(&id).map_or(&[], Vec::as_slice)
    }

    /// The name of a Shakalaka mask by its number (0 is the Acorn Mask, 5 the Fluffy Mask): the game lists them from text 372.
    pub fn mask_name(&self, n: u8) -> Option<&str> {
        let name = self.facility_texts.get(MASK_NAMES + usize::from(n))?;
        (!name.is_empty() && name != "DUMMY").then_some(name.as_str())
    }

    /// What a Shakalaka mask is and does, as a paragraph (the texts after the names, in the same order).
    pub fn mask_description(&self, n: u8) -> Option<String> {
        let text = self.facility_texts.get(MASK_DESCRIPTIONS + usize::from(n))?;
        (!text.is_empty()).then(|| unwrap_text(text))
    }

    /// A guild card title in words ("Noob to Excited"), from the numbers in the save (see `Save::card_title`). `None` when the dump has
    /// no title words or a number is outside them.
    pub fn card_title(&self, title: (u32, u8, u32)) -> Option<String> {
        let word = |n: u32| self.card_title_words.get(n as usize).filter(|w| !w.is_empty() && *w != "(None)");
        let (first, join, second) = title;
        let mut parts = vec![word(first)?.as_str()];
        if join != 0 {
            parts.push(word(612 + u32::from(join))?.as_str());
        }
        if let Some(w) = word(second) {
            parts.push(w);
        }
        Some(parts.join(" "))
    }

    /// The most of an item the hunter can carry in a stack, where the table has it.
    pub fn carry_limit(&self, id: u16) -> Option<u16> {
        self.item_extras.get(&id)?.carry
    }

    /// What a shop asks for an item, if some shop sells it.
    pub fn shop_price(&self, id: u16) -> Option<u32> {
        self.item_extras.get(&id)?.buy
    }

    /// What a shop pays for an item, in zenny; `None` for an item with no value (or an id past the table).
    pub fn sell_price(&self, item: u16) -> Option<u32> {
        self.sell_prices.get(usize::from(item)).copied().filter(|&p| p > 0)
    }

    /// The quests, by id.
    pub fn quests(&self) -> &[crate::quest::Quest] {
        &self.quests
    }

    /// Every item id and name, for searching by name.
    pub fn item_names(&self) -> impl Iterator<Item = (u16, &str)> {
        self.items.iter().enumerate().map(|(i, n)| (i as u16, n.as_str()))
    }

    pub fn item_name(&self, id: u16) -> Option<&str> {
        self.items.get(id as usize).map(String::as_str)
    }

    pub fn equipment_name(&self, kind: u8, id: u16) -> Option<&str> {
        self.equipment.get(&kind)?.names.get(id as usize).map(String::as_str)
    }

    /// The name of a piece that exists: not empty and not one of the game's `DUMMY` placeholders.
    pub fn piece_name(&self, kind: u8, id: u16) -> Option<&str> {
        self.equipment_name(kind, id).filter(|n| !n.is_empty() && !n.starts_with("DUMMY"))
    }

    /// Every armor piece and weapon that exists as (kind, id, name); talismans are not included.
    pub fn equipment_pieces(&self) -> Vec<(u8, u16, &str)> {
        let mut kinds: Vec<u8> = self.equipment.keys().copied().filter(|&k| k != 6).collect();
        kinds.sort_unstable();
        kinds
            .into_iter()
            .flat_map(|k| self.piece_ids(k).filter_map(move |id| Some((k, id, self.piece_name(k, id)?))))
            .collect()
    }

    /// The ids of the pieces of one kind that exist, ascending.
    pub fn piece_ids(&self, kind: u8) -> impl Iterator<Item = u16> + '_ {
        let count = self.equipment.get(&kind).map_or(0, |t| t.names.len());
        (1..count.min(usize::from(u16::MAX)) as u16).filter(move |&id| self.piece_name(kind, id).is_some())
    }

    /// The game's description of a piece of equipment, on one line. `None` when there is none or it is a placeholder.
    pub fn equipment_description(&self, kind: u8, id: u16) -> Option<&str> {
        self.equipment
            .get(&kind)?
            .details
            .get(id as usize)
            .map(String::as_str)
            .filter(|t| !t.is_empty())
    }

    /// The game's description of an item, on one line.
    pub fn item_description(&self, id: u16) -> Option<&str> {
        self.item_details
            .get(id as usize)
            .map(String::as_str)
            .filter(|t| !t.is_empty() && *t != "(None)")
    }

    /// What a skill tree does (the text shown under the skill in the game's menus).
    pub fn skill_description(&self, id: u8) -> Option<&str> {
        self.skill_details
            .get(id as usize)
            .map(String::as_str)
            .filter(|t| !t.is_empty() && *t != "DUMMY")
    }

    /// The carve and shiny-drop lists.
    pub fn drops(&self) -> &crate::drops::Drops {
        &self.drops
    }

    /// The game's Hunter's Note on a monster, as one paragraph.
    pub fn monster_note(&self, id: u16) -> Option<&str> {
        let text = self.hunters_notes.get(crate::notes::note_of(id)?)?;
        (!text.is_empty()).then_some(text.as_str())
    }

    pub fn monster_name(&self, id: u16) -> Option<&str> {
        self.monsters
            .get(id as usize)
            .map(String::as_str)
            .filter(|n| !n.is_empty() && *n != "NO_DATA")
    }

    /// Every recipe that uses each item, worked out in one pass: item -> the pieces that take it and how many. For asking about many
    /// items; for one, `recipes_using` is simpler.
    pub fn material_uses(&self) -> HashMap<u16, Vec<MaterialUse>> {
        let mut uses: HashMap<u16, Vec<MaterialUse>> = HashMap::new();
        let create = self.recipes.iter().map(|(&key, r)| (key, &r.materials));
        let upgrade = self.upgrades.iter().map(|(&key, u)| (key, &u.materials));
        for ((kind, id), materials) in create.chain(upgrade) {
            if self.piece_name(kind, id).is_none() {
                continue;
            }
            for m in materials {
                uses.entry(m.id).or_default().push(MaterialUse { kind, id, count: m.count });
            }
        }
        uses
    }

    /// Every create or upgrade recipe that uses `item`, as (kind, piece id), in table order.
    pub fn recipes_using(&self, item: u16) -> Vec<(u8, u16)> {
        let mut found: Vec<(u8, u16)> = self
            .recipes
            .iter()
            .filter(|(_, r)| r.materials.iter().any(|m| m.id == item))
            .map(|(&key, _)| key)
            .collect();
        let upgrade_uses: Vec<(u8, u16)> = self
            .upgrades
            .iter()
            .filter(|(_, u)| u.materials.iter().any(|m| m.id == item))
            .map(|(&key, _)| key)
            .collect();
        for key in upgrade_uses {
            if !found.contains(&key) {
                found.push(key);
            }
        }
        found.sort_unstable();
        found
    }

    pub fn recipe(&self, kind: u8, id: u16) -> Option<&Recipe> {
        self.recipes.get(&(kind, id))
    }

    pub fn armor_stats(&self, kind: u8, id: u16) -> Option<&ArmorStats> {
        self.armor.get(&(kind, id))
    }

    /// What the forge charges to create an armor piece, from the game's own table.
    pub fn armor_cost(&self, kind: u8, id: u16) -> Option<u32> {
        self.armor.get(&(kind, id)).and_then(|a| a.price)
    }

    /// What the forge charges for a weapon, from the game's own table. `None` for armor and unknown weapons.
    pub fn weapon_cost(&self, kind: u8, id: u16, via: crate::weapons::Via) -> Option<u32> {
        self.weapons.get(&(kind, id)).map(|w| crate::weapons::cost(w.price, via))
    }

    /// Rarity, attack, affinity and gem slots of a weapon. `None` for armor and for weapons the tables don't cover.
    pub fn weapon_stats(&self, kind: u8, id: u16) -> Option<&crate::weapons::Weapon> {
        self.weapons.get(&(kind, id))
    }

    /// Rarity of an armor piece or weapon.
    pub fn equipment_rarity(&self, kind: u8, id: u16) -> Option<u8> {
        self.armor_stats(kind, id)
            .map(|a| a.rarity)
            .or_else(|| self.weapon_stats(kind, id).map(|w| w.rarity))
    }

    /// The ids of the skill trees that have a name (not the placeholder entries).
    pub fn skill_ids(&self) -> impl Iterator<Item = u8> + '_ {
        (1..self.skills.len().min(256)).filter_map(|i| {
            let name = self.skills[i].as_str();
            (!name.is_empty() && !name.starts_with("DUMMY") && name != "None" && name != "NO_DATA").then_some(i as u8)
        })
    }

    /// The name of a skill effect by its id, such as "Attack Up (L)".
    pub fn effect_name(&self, id: u16) -> Option<&str> {
        self.effect_names.get(usize::from(id)).map(String::as_str).filter(|n| !n.is_empty())
    }

    /// What a skill effect does, in the game's words on one line.
    pub fn effect_description(&self, id: u16) -> Option<&str> {
        self.effect_details
            .get(usize::from(id))
            .map(String::as_str)
            .filter(|t| !t.is_empty() && *t != "DUMMY")
    }

    /// Whether a skill is of use to a weapon kind (see `weaponskills`); true for the many skills that help every weapon.
    pub fn skill_suits_weapon(&self, skill: u8, weapon_kind: u8) -> bool {
        self.skill_name(skill)
            .is_none_or(|name| crate::weaponskills::suits(name, weapon_kind))
    }

    pub fn skill_name(&self, id: u8) -> Option<&str> {
        self.skills.get(id as usize).map(String::as_str)
    }

    /// The upgrade recipe and parents of a weapon, if it has an upgrade recipe.
    pub fn upgrade(&self, kind: u8, id: u16) -> Option<&Upgrade> {
        self.upgrades.get(&(kind, id)).filter(|u| !u.materials.is_empty())
    }

    /// The weapons (same kind) that `id` can be upgraded into, in id order.
    pub fn upgrade_children(&self, kind: u8, id: u16) -> Vec<u16> {
        let mut kids: Vec<u16> = self
            .upgrades
            .iter()
            .filter(|((k, child), u)| *k == kind && !u.materials.is_empty() && *child != id && u.parents.contains(&id))
            .map(|((_, child), _)| *child)
            .collect();
        kids.sort_unstable();
        kids
    }

    /// All (kind, id, name) whose name contains `needle`, case-insensitively.
    pub fn find_equipment(&self, needle: &str) -> Vec<(u8, u16, &str)> {
        let needle = needle.to_lowercase();
        let mut hits: Vec<_> = self
            .equipment
            .iter()
            .flat_map(|(&kind, t)| t.names.iter().enumerate().map(move |(i, n)| (kind, i as u16, n.as_str())))
            .filter(|(_, _, n)| n.to_lowercase().contains(&needle))
            .collect();
        hits.sort_by_key(|&(k, i, _)| (k, i));
        hits
    }

    pub fn equipment_kind_label(&self, kind: u8) -> Option<&'static str> {
        self.equipment.get(&kind).map(|t| t.label)
    }
}

#[cfg(test)]
mod tests {
    use super::{dump_from_env, expand_home, find_dump, unwrap_text};

    #[test]
    fn a_folder_of_dumps_works_as_well_as_a_dump() {
        let root = std::env::temp_dir().join(format!("mh3u-dumps-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let game = root.join("MONSTER HUNTER 3 ULTIMATE [Game] [0005000010118300]");
        let update = root.join("MONSTER HUNTER 3 ULTIMATE [Update] [0005000e10118300]");
        for dump in [&game, &update] {
            std::fs::create_dir_all(dump.join("content/nativeCafe")).unwrap();
        }
        assert_eq!(find_dump(&game), Some(game.clone()), "a dump is itself");
        assert_eq!(
            find_dump(&root),
            Some(game.clone()),
            "a folder of dumps gives the game, not the update"
        );
        assert_eq!(find_dump(&root.join("nothing")), None);
        assert_eq!(find_dump(&update.join("content")), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_leading_tilde_is_the_home_folder() {
        let home = std::env::var("HOME").unwrap_or_default();
        assert_eq!(expand_home("~/games/x"), std::path::Path::new(&home).join("games/x"));
        assert_eq!(expand_home("  ~ "), std::path::PathBuf::from(&home));
        assert_eq!(expand_home("/abs/path"), std::path::PathBuf::from("/abs/path"));
        assert_eq!(expand_home("a/~/b"), std::path::PathBuf::from("a/~/b"));
    }

    #[test]
    fn hard_wrapped_text_becomes_one_line() {
        assert_eq!(
            unwrap_text("An introductory text for first-\ntime combiners. Improves your\ncombination success rate."),
            "An introductory text for first-time combiners. Improves your combination success rate."
        );
        assert_eq!(
            unwrap_text("Head armor made from Jaggi \nparts. Inexpensive."),
            "Head armor made from Jaggi parts. Inexpensive."
        );
        assert_eq!(unwrap_text(""), "");
    }

    /// Every note in the table is the note of its monster: it contains the word the table gives (skips without a dump).
    #[test]
    fn hunters_notes_are_matched_to_their_monsters() {
        let Some(dir) = dump_from_env() else { return };
        let Ok(game) = super::GameData::load(&dir) else { return };
        for &(monster, note, word) in crate::notes::TABLE {
            let text = game
                .monster_note(monster)
                .unwrap_or_else(|| panic!("monster {monster}: no note {note}"));
            assert!(
                text.contains(word),
                "monster {monster} ({:?}): note {note} lacks {word:?}: {text}",
                game.monster_name(monster)
            );
        }
        assert!(game.monster_note(1).is_some_and(|t| t.contains("Fire-breathing female wyverns")));
        assert_eq!(game.monster_note(0), None);
    }

    /// The decoration a save held in a talisman (number 0x91) is the Tenderizer Jwl 1 (skips without a dump).
    #[test]
    fn a_decoration_number_from_a_save_names_the_jewel() {
        let Some(dir) = dump_from_env() else { return };
        let Ok(game) = super::GameData::load(&dir) else { return };
        let d = game.decoration(0x91).expect("decoration 145");
        assert_eq!(game.item_name(d.item), Some("Tenderizer Jwl 1"));
        assert_eq!((d.slots, game.skill_name(d.skill), d.points), (1, Some("Tenderizer"), 1));
        // three of them add up to what the game showed for a head piece with three: Tenderizer +3 (and -3 of the skill each costs)
        let points = game.decoration_points(&[0x91, 0x91, 0x91]);
        let tender: i32 = points.iter().filter(|&&(id, _)| id == d.skill).map(|&(_, p)| i32::from(p)).sum();
        assert_eq!(tender, 3);
        assert_eq!(game.decoration(0), None);
        // guild card titles are put together from words
        assert_eq!(game.card_title((4, 0, 6)).as_deref(), Some("Fledgling Hunter"));
        assert_eq!(game.card_title((1, 6, 51)).as_deref(), Some("Noob to Excited"));
        assert_eq!(game.card_title((99999, 0, 0)), None);
        // gathering spots joined by name
        assert_eq!(game.gather_spots.len(), 155);
        let ore = game.item_names().find(|(_, n)| *n == "Iron Ore").unwrap().0;
        assert!(game.gather_spots(ore).iter().any(|s| s.kind == "Mining"));
        assert!(game.gather_spots(0).is_empty());
        // zone names are given only where the monster and the numbers are the game's
        assert_eq!(game.zone_name(1, 0), Some("Head"));
        assert_eq!(game.zone_name(1, 99), None);
        assert_eq!(game.zone_name(0, 0), None);
        let named = (1..200u16)
            .map(|m| (0..game.hit_zones(m).len()).filter(|&r| game.zone_name(m, r).is_some()).count())
            .sum::<usize>();
        assert!(named > 300, "{named}");
        // every skill the weapon rules name exists in the game's text
        for name in crate::weaponskills::listed() {
            assert!(game.skills.iter().any(|s| s == name), "{name}");
        }
        // carry limits and shop prices joined by name
        assert!(game.item_extras.len() > 1250, "{}", game.item_extras.len());
        let potion = game.item_names().find(|(_, n)| *n == "Potion").unwrap().0;
        assert_eq!((game.carry_limit(potion), game.shop_price(potion)), (Some(10), Some(66)));
        // sharpness and element joined by name: nearly every melee weapon has them, and Chrome Quietus is the example in the docs
        assert!(game.weapon_extras.len() > 1090, "{}", game.weapon_extras.len());
        let quietus = game
            .piece_ids(7)
            .find(|&i| game.piece_name(7, i) == Some("Chrome Quietus"))
            .unwrap();
        assert_eq!(game.weapon_extras(7, quietus).unwrap().sharpness, [22, 20, 15, 9, 15, 7, 0]);
        // the effect text lines up with the tier table
        let (_, effect) = crate::skilltiers::tiers(11).next().unwrap();
        assert_eq!(game.effect_name(effect), Some("Attack Up (L)"));
        assert!(game.effect_description(effect).is_some_and(|t| t.contains("Attack")));
        assert_eq!(game.decoration(5000), None);
    }

    /// Sell prices of real items, as a published list gives them (Rathian Scale 490, Potion 5... checked by name), skips without a dump.
    #[test]
    fn items_have_their_sell_prices() {
        let Some(dir) = dump_from_env() else { return };
        let Ok(game) = super::GameData::load(&dir) else { return };
        let price = |name: &str| game.item_names().find(|&(_, n)| n == name).and_then(|(id, _)| game.sell_price(id));
        assert_eq!(price("Rathian Scale"), Some(490));
        assert_eq!(price("Rathian Spike"), Some(2000));
        assert_eq!(price("Rathian Plate"), Some(4850));
        assert_eq!(price("No such item"), None);
    }

    /// Maximum defense of real pieces, as a published list gives it (Leather Headgear 53, Hunter's Helm 100, Jaggi Cap 56), and every
    /// armor piece with a recipe has one (skips without a dump).
    #[test]
    fn armor_maximum_defense_is_read_for_real_pieces() {
        let Some(dir) = dump_from_env() else { return };
        let Ok(game) = super::GameData::load(&dir) else { return };
        let max = |kind: u8, name: &str| {
            let id = game
                .find_equipment(name)
                .into_iter()
                .find(|&(k, _, n)| k == kind && n == name)
                .map(|(_, id, _)| id)?;
            game.armor_stats(kind, id)?.max_defense
        };
        assert_eq!(max(5, "Leather Headgear"), Some(53));
        assert_eq!(max(5, "Hunter's Helm"), Some(100));
        assert_eq!(max(5, "Jaggi Cap"), Some(56));
        for kind in [1u8, 2, 3, 4, 5] {
            let missing = game
                .piece_ids(kind)
                .filter(|&id| game.recipe(kind, id).is_some())
                .filter(|&id| game.armor_stats(kind, id).is_none_or(|s| s.max_defense.is_none()))
                .count();
            assert!(missing <= 2, "kind {kind}: {missing} pieces with a recipe have no maximum defense");
        }
    }

    /// The part names fit the break lists of the dump: each monster in the table has as many break lists as parts in at least one rank
    /// (skips without a dump).
    #[test]
    fn break_part_names_fit_the_break_lists_in_the_dump() {
        use crate::drops::{Method, Rank};
        let Some(dir) = dump_from_env() else { return };
        let Ok(game) = super::GameData::load(&dir) else { return };
        let (mut monsters, mut named) = (0, 0);
        for &monster in game.drops().monsters() {
            let Some(parts) = crate::breakparts::parts(monster) else { continue };
            monsters += 1;
            let fits = |rank| {
                game.drops()
                    .lists_for(monster)
                    .filter(|&(m, r, _)| r == rank && matches!(m, Method::Break(_)))
                    .count()
                    == parts.len()
            };
            assert!(
                Rank::ALL.iter().any(|&r| fits(r)),
                "monster {monster}: no rank has {} break lists",
                parts.len()
            );
            named += 1;
        }
        assert!(monsters >= 40 && named == monsters, "{named} of {monsters}");
        assert_eq!(game.drop_label(1, Rank::Low, Method::Break(1)), "Head break");
        assert_eq!(game.drop_label(1, Rank::Low, Method::Break(2)), "Wing break");
        assert_eq!(game.drop_label(4, Rank::High, Method::Break(3)), "Stomach break");
        assert_eq!(game.drop_label(1, Rank::Low, Method::BodyCarve), "Body carve");
        assert_eq!(
            game.drop_label(0, Rank::Low, Method::Break(1)),
            "Part break 1",
            "a monster not in the table"
        );
    }

    /// Real hit zones, as read from the dump (skipped without one): Rathian's first zone and Arzuros's five.
    #[test]
    fn monsters_have_hit_zones_in_the_dump() {
        let Some(dir) = dump_from_env() else { return };
        let Ok(game) = super::GameData::load(&dir) else { return };
        let rathian = game.hit_zones(1);
        assert_eq!(rathian.len(), 7);
        assert_eq!(rathian[0].physical(), [90, 80, 70]);
        assert_eq!(rathian[0].elements(), [0, 15, 15, 20, 35]);
        assert_eq!(game.hit_zones(42).len(), 5);
        assert!(game.hit_zones(0).is_empty());
    }

    /// The Shakalaka masks' names and descriptions are read in the game's order (skips without a dump).
    #[test]
    fn the_shakalaka_masks_are_named_in_the_games_order() {
        let Some(dir) = dump_from_env() else { return };
        let Ok(game) = super::GameData::load(&dir) else { return };
        assert_eq!(game.mask_name(0), Some("Acorn Mask"));
        assert_eq!(game.mask_name(5), Some("Fluffy Mask"));
        assert_eq!(game.mask_name(18), None, "the dummy ones have no name");
        assert!(game.mask_description(0).is_some_and(|d| d.contains("acorn")));
        assert!(game.mask_description(5).is_some());
    }
}

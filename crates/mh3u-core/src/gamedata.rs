//! Static game data (names, descriptions) read from the user's own game dump.

use crate::{
    arc::Arc,
    armor::{self, ArmorStats},
    gmd,
    recipes::{self, Recipe, Upgrade},
    rpx,
};
use anyhow::{Context, Result};
use std::{collections::HashMap, path::Path};

/// English text for items and equipment lives in this archive, relative to the dump's game folder.
const TEXT_ARCHIVE: &str = "content/nativeCafe/arc/ID/ID_arena_eng.arc";

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

pub struct GameData {
    items: Vec<String>,
    /// Text tables the game ships for descriptions; empty when a dump lacks them.
    item_details: Vec<String>,
    skill_details: Vec<String>,
    monsters: Vec<String>,
    recipes: HashMap<(u8, u16), Recipe>,
    upgrades: HashMap<(u8, u16), Upgrade>,
    armor: HashMap<(u8, u16), ArmorStats>,
    weapons: HashMap<(u8, u16), crate::weapons::Weapon>,
    drops: crate::drops::Drops,
    skills: Vec<String>,
    equipment: HashMap<u8, EquipmentTable>,
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
        Ok(GameData {
            items: names("Item00_eng")?,
            item_details: optional("ItemDetail_eng"),
            skill_details: optional("Skill_Type_Exp_eng"),
            monsters: names("Monster_eng").unwrap_or_default(),
            recipes,
            upgrades,
            armor,
            weapons,
            drops,
            skills: names("Skill_Type_eng")?,
            equipment,
        })
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

    pub fn monster_name(&self, id: u16) -> Option<&str> {
        self.monsters
            .get(id as usize)
            .map(String::as_str)
            .filter(|n| !n.is_empty() && *n != "NO_DATA")
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
    use super::unwrap_text;

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
}

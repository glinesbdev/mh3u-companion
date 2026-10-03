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

pub struct GameData {
    items: Vec<String>,
    recipes: HashMap<(u8, u16), Recipe>,
    upgrades: HashMap<(u8, u16), Upgrade>,
    armor: HashMap<(u8, u16), ArmorStats>,
    weapons: HashMap<(u8, u16), crate::weapons::Weapon>,
    skills: Vec<String>,
    equipment: Vec<(u8, &'static str, Vec<String>)>,
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
        let equipment = EQUIPMENT_KINDS
            .iter()
            .map(|&(kind, label, file)| Ok((kind, label, strings(file)?)))
            .collect::<Result<_>>()?;
        let rpx_path = rpx_path(game_dir)?;
        let rpx_bytes = std::fs::read(&rpx_path).with_context(|| format!("reading {}", rpx_path.display()))?;
        let data_section = rpx::section_at(&rpx_bytes, recipes::DATA_SECTION_ADDR)?;
        let (recipes, upgrades) = (recipes::parse(&data_section)?, recipes::parse_upgrades(&data_section)?);
        let armor = armor::parse(&data_section)?;
        let weapons = crate::weapons::parse(&data_section)?;
        Ok(GameData {
            items: strings("Item00_eng")?,
            recipes,
            upgrades,
            armor,
            weapons,
            skills: strings("Skill_Type_eng")?,
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
        let (_, _, names) = self.equipment.iter().find(|(k, ..)| *k == kind)?;
        names.get(id as usize).map(String::as_str)
    }

    pub fn recipe(&self, kind: u8, id: u16) -> Option<&Recipe> {
        self.recipes.get(&(kind, id))
    }

    pub fn armor_stats(&self, kind: u8, id: u16) -> Option<&ArmorStats> {
        self.armor.get(&(kind, id))
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

    pub fn skill_name(&self, id: u8) -> Option<&str> {
        self.skills.get(id as usize).map(String::as_str)
    }

    /// The upgrade recipe and parents of a weapon, if it has an upgrade recipe.
    pub fn upgrade(&self, kind: u8, id: u16) -> Option<&Upgrade> {
        self.upgrades.get(&(kind, id)).filter(|u| !u.materials.is_empty())
    }

    /// All (kind, id, name) whose name contains `needle`, case-insensitively.
    pub fn find_equipment(&self, needle: &str) -> Vec<(u8, u16, &str)> {
        let needle = needle.to_lowercase();
        let mut hits: Vec<_> = self
            .equipment
            .iter()
            .flat_map(|(kind, _, names)| names.iter().enumerate().map(move |(i, n)| (*kind, i as u16, n.as_str())))
            .filter(|(_, _, n)| n.to_lowercase().contains(&needle))
            .collect();
        hits.sort_by_key(|&(k, i, _)| (k, i));
        hits
    }

    pub fn equipment_kind_label(&self, kind: u8) -> Option<&'static str> {
        self.equipment.iter().find(|(k, ..)| *k == kind).map(|(_, label, _)| *label)
    }
}

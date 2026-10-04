//! Which weapons a skill is for.
//!
//! Most skills help every weapon (Attack Up, Critical Eye, defense and resistances). Some only matter to some weapons: the sharpness skills
//! to the weapons that have a sharpness bar, reload speed to bowguns, the guard skills to the weapons that block. Nothing in the game's
//! files says this (the skill text names the weapons in words only), so this is a hand-made list of skill names, written from the skill
//! descriptions (the game's, checked against Kiranico's skill page) and what a weapon can do. A skill that is not listed suits every
//! weapon. The names are the game's short skill names; a test checks that each one exists.

/// Weapon kinds (the equipment kinds of `docs/formats.md`).
const GREAT_SWORD: u8 = 7;
const SWORD_SHIELD: u8 = 8;
const HAMMER: u8 = 9;
const LANCE: u8 = 10;
const HEAVY_BOWGUN: u8 = 11;
const LIGHT_BOWGUN: u8 = 13;
const LONG_SWORD: u8 = 14;
const SWITCH_AXE: u8 = 15;
const GUNLANCE: u8 = 16;
const BOW: u8 = 17;
const DUAL_BLADES: u8 = 18;
const HUNTING_HORN: u8 = 19;

/// Weapons with a sharpness bar: the hammer and the horn are blunt.
const SHARP: &[u8] = &[GREAT_SWORD, SWORD_SHIELD, LANCE, LONG_SWORD, SWITCH_AXE, GUNLANCE, DUAL_BLADES];
/// Every weapon that is swung (not shot).
const MELEE: &[u8] = &[
    GREAT_SWORD,
    SWORD_SHIELD,
    HAMMER,
    LANCE,
    LONG_SWORD,
    SWITCH_AXE,
    GUNLANCE,
    DUAL_BLADES,
    HUNTING_HORN,
];
/// Weapons that can block.
const BLOCKERS: &[u8] = &[GREAT_SWORD, SWORD_SHIELD, LANCE, GUNLANCE];
/// Weapons with a charge (a gauge, a charged shot or a charged swing).
const CHARGERS: &[u8] = &[GREAT_SWORD, HAMMER, LONG_SWORD, SWITCH_AXE, DUAL_BLADES, BOW];
const BOWGUNS: &[u8] = &[HEAVY_BOWGUN, LIGHT_BOWGUN];
/// Everything that shoots.
const GUNNERS: &[u8] = &[HEAVY_BOWGUN, LIGHT_BOWGUN, BOW];

/// (the skill's name in the game's text, the weapons it is for).
const RULES: &[(&str, &[u8])] = &[
    ("Sharpness", SHARP),
    ("Handicraft", SHARP),
    ("Sharpener", SHARP),
    ("Fencing", SHARP),
    ("Edgemaster", SHARP),
    ("Crit Draw", MELEE),
    ("PunishDraw", MELEE),
    ("Guard", BLOCKERS),
    ("Guard Up", BLOCKERS),
    ("Auto-Guard", BLOCKERS),
    ("FastCharge", CHARGERS),
    ("Maestro", &[HUNTING_HORN]),
    ("Artillery", &[GUNLANCE, HEAVY_BOWGUN, LIGHT_BOWGUN]),
    ("Reload Spd", BOWGUNS),
    ("Loading", BOWGUNS),
    ("Rapid Fire", BOWGUNS),
    ("Recoil", BOWGUNS),
    ("Precision", BOWGUNS),
    ("Normal Up", GUNNERS),
    ("Pierce Up", GUNNERS),
    ("Pellet Up", GUNNERS),
    ("SteadyHand", GUNNERS),
    ("Normal S+", BOWGUNS),
    ("Pierce S+", BOWGUNS),
    ("Pellet S+", BOWGUNS),
    ("Crag S+", BOWGUNS),
    ("Clust S+", BOWGUNS),
    ("Slicing S+", BOWGUNS),
    ("Slime S+", BOWGUNS),
    ("Poison C+", &[BOW]),
    ("Para C+", &[BOW]),
    ("Sleep C+", &[BOW]),
    ("Power C+", &[BOW]),
    ("C.Range C+", &[BOW]),
    ("Exhaust C+", &[BOW]),
    ("Slime C+", &[BOW]),
];

/// Whether a skill (by its name in the game's text) is of use to a weapon kind. A skill with no rule is of use to every weapon.
pub fn suits(skill: &str, weapon_kind: u8) -> bool {
    RULES
        .iter()
        .find(|(name, _)| *name == skill)
        .is_none_or(|(_, kinds)| kinds.contains(&weapon_kind))
}

/// The skill names that have a rule, for checking them against the game's text.
pub fn listed() -> impl Iterator<Item = &'static str> {
    RULES.iter().map(|(name, _)| *name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skill_suits_the_weapons_that_can_use_it() {
        assert!(suits("Sharpness", GREAT_SWORD));
        assert!(!suits("Sharpness", HAMMER), "a hammer has no sharpness");
        assert!(!suits("Sharpness", LIGHT_BOWGUN));
        assert!(suits("Reload Spd", HEAVY_BOWGUN));
        assert!(!suits("Reload Spd", LONG_SWORD));
        assert!(suits("Poison C+", BOW) && !suits("Poison C+", HEAVY_BOWGUN));
        assert!(suits("Guard", LANCE) && !suits("Guard", HUNTING_HORN));
        assert!(suits("Maestro", HUNTING_HORN) && !suits("Maestro", HAMMER));
        // a skill with no rule helps everyone
        assert!(suits("Attack", BOW) && suits("Attack", HAMMER));
        assert!(suits("Not a skill", GREAT_SWORD));
    }

    #[test]
    fn no_skill_is_listed_twice() {
        let mut names: Vec<&str> = listed().collect();
        names.sort_unstable();
        let total = names.len();
        names.dedup();
        assert_eq!(names.len(), total);
    }
}

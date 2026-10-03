//! Which pieces the blacksmith offers, as far as it has been worked out.
//!
//! Observed in play (see `docs/formats.md`): a piece appears once the monster its materials come from has been hunted (killed or
//! captured) at least once, and holding the materials without hunting does nothing (pieces whose materials were only given with a
//! debug command stayed off the list; Great Jaggi armor appeared when the first Great Jaggi quest was done). The first material
//! of the recipe is the one that counts. Higher-rank variants (S, X, ...) want materials that drop only in high or G rank, so a
//! hunt in low rank does not open them. The game keeps one count per monster, not per rank, so for those the rule can only say
//! that a high-rank hunt is needed.

use crate::{
    drops::{Drops, Rank},
    recipes::Recipe,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unlock {
    /// Starting gear, always on offer.
    Starter,
    /// On offer: this monster (name id) drops the first material in low rank and has been hunted.
    Hunted(u16),
    /// Not on offer yet: hunt one of these monsters (name ids), who drop the first material in this rank.
    NeedsHunt(Rank, Vec<u16>),
    /// A monster that drops the first material has been hunted, but only in this higher rank; a hunt in that rank is needed.
    NeedsRank(Rank),
    /// Village and event pieces (Yukumo armor and the like), unlocked some other way.
    Special,
    /// The first material is not a monster drop (an ore, a bug, a fish...), so the rule cannot say.
    Unknown(u16),
}

/// Judge one recipe. `hunted(monster)` is how many times the monster (name id) has been killed or captured.
pub fn unlock(recipe: &Recipe, drops: &Drops, hunted: impl Fn(u16) -> u16) -> Unlock {
    if recipe.flag == 1 {
        return Unlock::Starter;
    }
    let Some(first) = recipe.materials.first().map(|m| m.id).filter(|_| recipe.tier != 0) else {
        return Unlock::Special;
    };
    // The lowest rank each monster drops the item in.
    let mut sources: Vec<(u16, Rank)> = Vec::new();
    for (monster, rank, ..) in drops.sources(first) {
        match sources.iter_mut().find(|(m, _)| *m == monster) {
            Some((_, r)) => *r = (*r).min(rank),
            None => sources.push((monster, rank)),
        }
    }
    if sources.is_empty() {
        return Unlock::Unknown(first);
    }
    if let Some(&(monster, _)) = sources.iter().find(|&&(m, r)| r == Rank::Low && hunted(m) > 0) {
        return Unlock::Hunted(monster);
    }
    if let Some(rank) = sources.iter().filter(|&&(m, _)| hunted(m) > 0).map(|&(_, r)| r).min() {
        return Unlock::NeedsRank(rank);
    }
    let lowest = sources.iter().map(|&(_, r)| r).min().unwrap_or(Rank::Low);
    let mut monsters: Vec<u16> = sources.iter().filter(|&&(_, r)| r == lowest).map(|&(m, _)| m).collect();
    monsters.sort_unstable();
    Unlock::NeedsHunt(lowest, monsters)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gamedata::GameData, save::Save};

    fn game() -> Option<GameData> {
        let dir = std::fs::read_dir(format!("{}/games/wiiu", std::env::var("HOME").ok()?))
            .ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("[Game]") && n.contains("10118300"))
            })?;
        GameData::load(&dir).ok()
    }

    fn judge(game: &GameData, save: &Save, kind: u8, name: &str) -> Unlock {
        let id = (1..2000u16)
            .find(|&id| game.equipment_name(kind, id) == Some(name))
            .unwrap_or_else(|| panic!("no {name}"));
        unlock(game.recipe(kind, id).unwrap(), game.drops(), |m| save.times_hunted(m))
    }

    /// A WornTester save just before a captured Arzuros, then just after: the blacksmith went from not offering Arzuros armor to
    /// offering all ten pieces and the Jawblade, and the Jaggi armor (from an earlier Great Jaggi quest) was on offer in both.
    #[test]
    fn arzuros_pieces_appear_with_the_first_arzuros_hunted() {
        let (Some(game), before, after) = (game(), fixture!("07-before-quest2/user2"), fixture!("08-after-quest2/user2")) else {
            return;
        };
        let (before, after) = (Save::parse(&before).unwrap(), Save::parse(&after).unwrap());
        for (kind, name) in [
            (5, "Arzuros Helm"),
            (1, "Arzuros Mail"),
            (2, "Arzuros Guards"),
            (3, "Arzuros Coat"),
            (4, "Arzuros Leggings"),
            (7, "Jawblade"),
        ] {
            assert!(
                matches!(judge(&game, &before, kind, name), Unlock::NeedsHunt(Rank::Low, _)),
                "{name} before"
            );
            assert_eq!(judge(&game, &after, kind, name), Unlock::Hunted(42), "{name} after");
        }
        for save in [&before, &after] {
            assert_eq!(judge(&game, save, 5, "Jaggi Helm"), Unlock::Hunted(12));
            assert_eq!(judge(&game, save, 3, "Ludroth Faulds"), Unlock::Hunted(17));
            assert!(matches!(judge(&game, save, 5, "Lagiacrus Helm"), Unlock::NeedsHunt(Rank::Low, _)));
        }
        // the high-rank variants of a hunted monster want a high-rank hunt
        assert_eq!(judge(&game, &after, 5, "Jaggi Helm S"), Unlock::NeedsRank(Rank::High));
    }

    /// The start of the game on the main hunter: the Jaggi legs were offered beyond the starting gear.
    #[test]
    fn the_jaggi_legs_are_predicted_at_the_start_of_the_game() {
        let (Some(game), save) = (game(), fixture!("03-latest/user1")) else {
            return;
        };
        let save = Save::parse(&save).unwrap();
        for name in ["Jaggi Greaves", "Jaggi Leggings"] {
            assert!(matches!(judge(&game, &save, 4, name), Unlock::Hunted(_)), "{name}");
        }
        assert!(matches!(judge(&game, &save, 4, "Rhenoplos Greaves"), Unlock::NeedsHunt(..)));
    }

    #[test]
    fn starting_gear_and_special_pieces_ignore_the_hunts() {
        use crate::save::ItemStack;
        let recipe = |flag, tier, id| Recipe {
            materials: vec![ItemStack { id, count: 2 }],
            flag,
            tier,
        };
        let drops = crate::drops::Drops::default();
        assert_eq!(unlock(&recipe(1, 1, 214), &drops, |_| 9), Unlock::Starter);
        assert_eq!(unlock(&recipe(0, 0, 214), &drops, |_| 9), Unlock::Special);
        assert_eq!(
            unlock(&recipe(0, 2, 214), &drops, |_| 9),
            Unlock::Unknown(214),
            "an ore: no monster drops it"
        );
    }
}

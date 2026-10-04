//! Which body part each part-break reward list is.
//!
//! The game's data was searched for this and does not say (see `docs/ideas.md`): the lists are numbered, and no table naming the part
//! each one is was found. So this is a small table kept by hand: for each monster, the kinds of part its break lists are, in the order
//! of the lists. It was made by matching each list's items against a published monster database (the same items, the same order in every
//! rank), so a name can be off, and a monster missing from the table has no names. A monster's names are only used when the number of
//! break lists in the rank is the table's number (see `GameData::drop_label`).

/// A kind of body part that can be broken. The same kinds are used for every monster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Head,
    Wrist,
    Back,
    Wing,
    FrontLeg,
    Jaw,
    Tail,
    Hump,
    Horn,
    Stomach,
    Tusk,
    Spike,
    Sponge,
    Lantern,
    OralCavity,
    Chest,
    Fin,
    BackLeg,
    Shoulder,
    TailEnd,
}

impl Part {
    pub fn label(self) -> &'static str {
        match self {
            Part::Head => "Head",
            Part::Wrist => "Wrist",
            Part::Back => "Back",
            Part::Wing => "Wing",
            Part::FrontLeg => "Front leg",
            Part::Jaw => "Jaw",
            Part::Tail => "Tail",
            Part::Hump => "Hump",
            Part::Horn => "Horn",
            Part::Stomach => "Stomach",
            Part::Tusk => "Tusk",
            Part::Spike => "Spike",
            Part::Sponge => "Sponge",
            Part::Lantern => "Lantern",
            Part::OralCavity => "Oral cavity",
            Part::Chest => "Chest",
            Part::Fin => "Fin",
            Part::BackLeg => "Back leg",
            Part::Shoulder => "Shoulder",
            Part::TailEnd => "Tail end",
        }
    }
}

/// Monster id (in the name table) and the parts its break lists are, in order.
const TABLE: &[(u16, &[Part])] = &[
    (1, &[Part::Head, Part::Wing]),                                  // Rathian
    (2, &[Part::Head, Part::Wing]),                                  // Rathalos
    (3, &[Part::Head, Part::Wing]),                                  // Qurupeco
    (4, &[Part::Tail, Part::Head, Part::Stomach]),                   // Gigginox
    (5, &[Part::Tusk, Part::Spike]),                                 // Barioth
    (6, &[Part::Horn]),                                              // Diablos
    (8, &[Part::FrontLeg]),                                          // Barroth
    (9, &[Part::Jaw, Part::Tail]),                                   // Uragaan
    (12, &[Part::Head]),                                             // Great Jaggi
    (14, &[Part::Head]),                                             // Great Baggi
    (15, &[Part::Head, Part::FrontLeg, Part::Chest, Part::Back]),    // Lagiacrus
    (16, &[Part::Head, Part::Sponge]),                               // Royal Ludroth
    (18, &[Part::Lantern]),                                          // Gobul
    (19, &[Part::Head, Part::BackLeg, Part::Chest, Part::Fin]),      // Agnaktor
    (20, &[Part::Chest, Part::Back, Part::Tail]),                    // Ceadeus
    (25, &[Part::Tusk, Part::Back, Part::Wrist]),                    // Jhen Mohran
    (41, &[Part::Horn, Part::FrontLeg]),                             // Zinogre
    (42, &[Part::Wrist]),                                            // Arzuros
    (43, &[Part::Head]),                                             // Lagombi
    (44, &[Part::Back]),                                             // Volvidon
    (45, &[Part::Head]),                                             // Great Wroggi
    (46, &[Part::Head, Part::Hump]),                                 // Duramboros
    (47, &[Part::FrontLeg, Part::OralCavity]),                       // Nibelsnarf
    (51, &[Part::Head, Part::Wing]),                                 // Crimson Qurupeco
    (52, &[Part::Tail, Part::Head, Part::Stomach]),                  // Baleful Gigginox
    (53, &[Part::Tusk, Part::Spike]),                                // Sand Barioth
    (54, &[Part::FrontLeg]),                                         // Jade Barroth
    (55, &[Part::Jaw, Part::Tail]),                                  // Steel Uragaan
    (56, &[Part::Head, Part::Sponge]),                               // Purple Ludroth
    (58, &[Part::Horn]),                                             // Black Diablos
    (59, &[Part::Wing, Part::Head, Part::Tail]),                     // Nargacuga
    (62, &[Part::Head, Part::Wing]),                                 // Pink Rathian
    (63, &[Part::Head, Part::Wing]),                                 // Gold Rathian
    (64, &[Part::Head, Part::Wing]),                                 // Azure Rathalos
    (66, &[Part::Wing, Part::Head, Part::Fin]),                      // Plesioth
    (67, &[Part::Wing, Part::Head, Part::Fin]),                      // Green Plesioth
    (68, &[Part::Head, Part::FrontLeg, Part::Chest, Part::Back]),    // Ivory Lagiacrus
    (70, &[Part::Chest, Part::Back, Part::Tail]),                    // Goldbeard Ceadeus
    (71, &[Part::Head]),                                             // Deviljho
    (72, &[Part::Horn, Part::FrontLeg]),                             // Stygian Zinogre
    (73, &[Part::Head, Part::Hump]),                                 // Rust Duramboros
    (75, &[Part::Head, Part::Chest, Part::Shoulder, Part::TailEnd]), // Dire Miralis
    (85, &[Part::Tusk, Part::Back, Part::Wrist]),                    // Hallowed Jhen Mohran
];

/// The parts a monster's break lists are, in the order of the lists, when known.
pub fn parts(monster: u16) -> Option<&'static [Part]> {
    TABLE.iter().find(|(m, _)| *m == monster).map(|&(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_monster_has_its_parts_in_order() {
        assert_eq!(parts(1), Some(&[Part::Head, Part::Wing][..]), "Rathian");
        assert_eq!(parts(4), Some(&[Part::Tail, Part::Head, Part::Stomach][..]), "Gigginox");
        assert_eq!(
            parts(15),
            Some(&[Part::Head, Part::FrontLeg, Part::Chest, Part::Back][..]),
            "Lagiacrus"
        );
        assert_eq!(parts(0), None);
    }

    #[test]
    fn the_table_is_in_order_without_repeats() {
        assert!(TABLE.windows(2).all(|w| w[0].0 < w[1].0));
    }
}

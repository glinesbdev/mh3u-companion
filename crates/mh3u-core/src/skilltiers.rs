//! What each skill tree does at each number of points: the tiers (+10, +15, +20 and the penalties at -10, -15, -20).
//!
//! The game's own table of this was not found in the executable (the bounded search is described in `docs/formats.md`), so this is
//! a hand-made table: for each skill tree, which effects it has and at how many points each starts. The effects themselves are the
//! game's: an effect id indexes the `Skill_eng` text (its name) and the `Skill_Exp_eng` text (its description), in the order the
//! game lists them (a tree's effects are neighbours, the positive ones first). The thresholds and which effect belongs to which
//! tree were read off a public database and checked against the game's text: every effect name matched exactly one game name, and
//! the effects of a tree come in the game's id order.
//!
//! Torso Up has no entry (its one effect is not in that text); the app handles it on its own.

/// (skill tree id, points where the effect starts: negative for a penalty, effect id), by tree then points, highest first.
const TIERS: &[(u8, i8, u16)] = &[
    (2, 10, 1),
    (2, -10, 2),
    (3, 10, 3),
    (3, -10, 4),
    (4, 10, 5),
    (4, -10, 6),
    (5, 15, 8),
    (5, 10, 7),
    (5, -10, 9),
    (6, 10, 10),
    (7, 10, 208),
    (8, 10, 220),
    (9, 10, 206),
    (10, 10, 248),
    (11, 20, 77),
    (11, 15, 76),
    (11, 10, 75),
    (11, -10, 78),
    (11, -15, 79),
    (11, -20, 80),
    (12, 20, 83),
    (12, 15, 82),
    (12, 10, 81),
    (12, -10, 84),
    (12, -15, 85),
    (12, -20, 86),
    (13, 15, 14),
    (13, 10, 13),
    (13, -10, 15),
    (13, -15, 16),
    (14, 15, 18),
    (14, 10, 17),
    (14, -10, 19),
    (14, -15, 20),
    (15, 10, 87),
    (15, -10, 88),
    (16, 10, 242),
    (17, 10, 193),
    (17, -10, 194),
    (18, 20, 27),
    (18, 15, 26),
    (18, 10, 25),
    (18, -10, 28),
    (18, -15, 29),
    (18, -20, 30),
    (19, 10, 238),
    (20, 10, 239),
    (21, 10, 240),
    (22, 10, 241),
    (23, 15, 188),
    (23, 10, 187),
    (23, -10, 189),
    (23, -15, 190),
    (24, 15, 90),
    (24, 10, 89),
    (25, 15, 134),
    (25, 10, 133),
    (26, 10, 182),
    (27, 10, 126),
    (27, -10, 128),
    (28, 10, 130),
    (28, -10, 132),
    (29, 10, 170),
    (29, -10, 171),
    (30, 10, 243),
    (30, -10, 244),
    (31, 15, 163),
    (31, 10, 162),
    (31, -10, 164),
    (32, 10, 184),
    (33, 10, 246),
    (34, 10, 11),
    (34, -10, 12),
    (35, 15, 34),
    (35, 10, 33),
    (35, -10, 35),
    (36, 10, 37),
    (37, 10, 38),
    (38, 15, 64),
    (38, 10, 63),
    (38, -10, 65),
    (39, 15, 222),
    (39, 10, 221),
    (39, -10, 223),
    (40, 15, 225),
    (40, 10, 224),
    (40, -10, 226),
    (41, 15, 228),
    (41, 10, 227),
    (41, -10, 229),
    (42, 15, 231),
    (42, 10, 230),
    (42, -10, 232),
    (43, 15, 234),
    (43, 10, 233),
    (43, -10, 235),
    (44, 10, 66),
    (44, -10, 67),
    (45, 15, 97),
    (45, 10, 96),
    (45, -10, 99),
    (46, 15, 103),
    (46, 10, 102),
    (46, -10, 105),
    (47, 15, 109),
    (47, 10, 108),
    (47, -10, 111),
    (48, 15, 115),
    (48, 10, 114),
    (48, -10, 117),
    (49, 15, 121),
    (49, 10, 120),
    (49, -10, 123),
    (50, 10, 21),
    (50, -10, 22),
    (51, 10, 23),
    (52, 10, 24),
    (53, 10, 31),
    (53, -10, 32),
    (54, 10, 185),
    (55, 10, 201),
    (56, 20, 41),
    (56, 15, 40),
    (56, 10, 39),
    (56, -10, 42),
    (56, -15, 43),
    (56, -20, 44),
    (57, 10, 172),
    (58, 10, 205),
    (59, 20, 47),
    (59, 15, 46),
    (59, 10, 45),
    (59, -10, 48),
    (59, -15, 49),
    (59, -20, 50),
    (60, 15, 174),
    (60, 10, 173),
    (60, -10, 175),
    (60, -15, 176),
    (61, 10, 51),
    (62, 10, 52),
    (63, 10, 53),
    (64, 10, 54),
    (65, 15, 56),
    (65, 10, 55),
    (66, 15, 58),
    (66, 10, 57),
    (67, 15, 60),
    (67, 10, 59),
    (68, 15, 62),
    (68, 10, 61),
    (69, 10, 236),
    (70, 10, 264),
    (71, 10, 209),
    (72, 10, 210),
    (73, 10, 211),
    (74, 10, 212),
    (75, 10, 213),
    (76, 10, 237),
    (77, 10, 265),
    (78, 10, 214),
    (79, 15, 216),
    (79, 10, 215),
    (80, 10, 68),
    (81, 10, 153),
    (81, -10, 154),
    (82, 20, 93),
    (82, 10, 92),
    (83, 10, 168),
    (83, -10, 169),
    (84, 15, 70),
    (84, 10, 69),
    (84, -10, 71),
    (84, -15, 72),
    (85, 15, 74),
    (85, 10, 73),
    (86, 15, 178),
    (86, 10, 177),
    (86, -10, 179),
    (87, 10, 245),
    (88, 15, 157),
    (88, 10, 155),
    (88, -15, 160),
    (89, 10, 161),
    (90, 15, 138),
    (90, 10, 137),
    (90, -10, 139),
    (91, 10, 141),
    (92, 15, 143),
    (92, 10, 142),
    (92, -10, 144),
    (92, -15, 145),
    (93, 10, 247),
    (94, 10, 262),
    (95, 15, 147),
    (95, 10, 146),
    (95, -10, 148),
    (95, -15, 149),
    (96, 15, 181),
    (96, 10, 180),
    (97, 15, 191),
    (97, 10, 263),
    (98, 10, 192),
    (99, 15, 152),
    (99, 10, 151),
    (100, 10, 94),
    (101, 10, 219),
    (102, 10, 91),
    (103, 10, 186),
    (104, 10, 207),
    (105, 10, 195),
    (105, -10, 196),
    (106, 15, 198),
    (106, 10, 197),
    (107, 10, 199),
    (107, -10, 200),
    (108, 10, 253),
    (109, 15, 166),
    (109, 10, 165),
    (109, -10, 167),
    (110, 10, 204),
    (111, 15, 218),
    (111, 10, 217),
    (112, 15, 250),
    (112, 10, 249),
    (113, 10, 251),
    (114, 10, 203),
    (115, 10, 202),
    (116, 10, 257),
    (117, 10, 252),
    (118, 10, 254),
    (119, 10, 255),
    (120, 10, 256),
    (121, 10, 258),
    (122, 10, 259),
    (123, 10, 260),
];

/// The effect a total of `points` gives in a tree: the highest positive tier reached, or the lowest penalty tier reached.
pub fn effect_for(tree: u8, points: i32) -> Option<u16> {
    let tiers = || TIERS.iter().filter(move |t| t.0 == tree);
    if points > 0 {
        tiers()
            .filter(|t| t.1 > 0 && i32::from(t.1) <= points)
            .min_by_key(|t| -t.1)
            .map(|t| t.2)
    } else {
        tiers()
            .filter(|t| t.1 < 0 && i32::from(t.1) >= points)
            .max_by_key(|t| -t.1)
            .map(|t| t.2)
    }
}

/// A tree's tiers as (points, effect id), the highest positive first, then the penalties from the mildest down.
pub fn tiers(tree: u8) -> impl Iterator<Item = (i8, u16)> {
    TIERS.iter().filter(move |t| t.0 == tree).map(|t| (t.1, t.2))
}

/// The number of points at which a tree's first effect starts (almost always 10), if it has any.
pub fn first_tier(tree: u8) -> Option<i8> {
    tiers(tree).filter(|t| t.0 > 0).map(|t| t.0).min()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_total_gives_the_highest_tier_it_reaches() {
        // Attack: Up (S) 10, Up (M) 15, Up (L) 20, Down (S) -10, Down (M) -15, Down (L) -20
        assert_eq!(effect_for(11, 9), None);
        assert_eq!(effect_for(11, 10), Some(75));
        assert_eq!(effect_for(11, 14), Some(75));
        assert_eq!(effect_for(11, 15), Some(76));
        assert_eq!(effect_for(11, 25), Some(77));
        assert_eq!(effect_for(11, -9), None);
        assert_eq!(effect_for(11, -10), Some(78));
        assert_eq!(effect_for(11, -17), Some(79));
        assert_eq!(effect_for(11, -30), Some(80));
        assert_eq!(effect_for(11, 0), None);
    }

    #[test]
    fn every_tree_has_ordered_tiers() {
        let trees: std::collections::BTreeSet<u8> = TIERS.iter().map(|t| t.0).collect();
        assert!(trees.len() > 100);
        for tree in trees {
            let t: Vec<_> = tiers(tree).collect();
            assert!(t.iter().all(|&(p, e)| p != 0 && e > 0 && e < 336));
            let mut ids: Vec<_> = t.iter().map(|x| x.1).collect();
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), t.len(), "tree {tree} lists an effect twice");
            assert!(first_tier(tree).is_some() || t.iter().all(|x| x.0 < 0), "tree {tree}");
        }
    }
}

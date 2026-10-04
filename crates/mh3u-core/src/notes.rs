//! The Hunter's Notes: the game's description of each monster, in the text archive (`GUI\font\HNote_eng`).
//!
//! Notes 0 to 72 are monsters, in the order of the in-game journal (small monsters first, then large ones, a species before its
//! subspecies), which is not the order of the monster name table; the rest are help pages. Nothing in the data says which note is which
//! monster, so this table was made by reading the notes: the one that describes a species, or says "a subspecies of X" or names the
//! monster. Each entry carries a word that its note must contain (empty when the note names nothing, like "Relatively docile
//! herbivores..." for Aptonoth); a test checks that on the real text.

/// (monster id in the name table, note number, a word the note contains or "").
pub const TABLE: &[(u16, usize, &str)] = &[
    (1, 36, "Rathian"),
    (2, 39, "Rathalos"),
    (3, 28, ""),
    (4, 44, "Gigginox"),
    (5, 46, "Barioth"),
    (6, 42, "Diablos"),
    (7, 67, "Deviljho"),
    (8, 30, "Barroth"),
    (9, 32, ""),
    (10, 10, "Jaggi"),
    (11, 11, "Jaggia"),
    (12, 22, "Jaggi"),
    (13, 12, "Baggi"),
    (14, 23, "Jaggi"),
    (15, 52, "Lagiacrus"),
    (16, 48, "Royal Ludroth"),
    (17, 14, "Ludroth"),
    (18, 50, "Gobul"),
    (19, 55, "Agnaktor"),
    (20, 65, "Ceadeus"),
    (21, 15, "Uroktor"),
    (22, 16, ""),
    (23, 2, ""),
    (24, 70, ""),
    (25, 68, ""),
    (26, 9, "Giggi"),
    (27, 0, ""),
    (28, 3, ""),
    (29, 17, "Rhenoplos"),
    (30, 6, ""),
    (31, 7, ""),
    (32, 8, ""),
    (33, 5, ""),
    (34, 1, ""),
    (35, 9, "Giggi"),
    (36, 4, ""),
    (41, 60, ""),
    (42, 25, ""),
    (43, 26, "Lagombi"),
    (44, 27, ""),
    (45, 24, "Wroggi"),
    (46, 34, ""),
    (47, 51, ""),
    (48, 13, ""),
    (49, 20, ""),
    (50, 18, "Gargwa"),
    (51, 29, "Qurupeco"),
    (52, 45, "Gigginox"),
    (53, 47, "Barioth"),
    (54, 31, "Barroth"),
    (55, 33, "Uragaan"),
    (56, 49, "Royal Ludroth"),
    (57, 56, "Agnaktor"),
    (58, 43, "Diablos"),
    (59, 57, ""),
    (60, 58, "Nargacuga"),
    (61, 59, "Nargacuga"),
    (62, 37, "Pink Rathian"),
    (63, 38, "Rathian"),
    (64, 40, "Rathalos"),
    (65, 41, "Rathalos"),
    (66, 62, "Plesioth"),
    (67, 63, "Plesioth"),
    (68, 53, "Lagiacrus"),
    (69, 54, "Lagiacrus"),
    (70, 66, "Ceadeus"),
    (71, 72, "Deviljho"),
    (72, 61, "Zinogre"),
    (73, 35, "Duramboros"),
    (74, 64, "Brachydios"),
    (75, 71, ""),
    (76, 19, ""),
    (77, 21, ""),
    (78, 20, ""),
    (85, 69, "Jhen Mohran"),
];

/// The note number of a monster.
pub fn note_of(monster: u16) -> Option<usize> {
    TABLE.iter().find(|&&(m, ..)| m == monster).map(|&(_, n, _)| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_in_order_and_names_each_monster_once() {
        assert!(TABLE.windows(2).all(|w| w[0].0 < w[1].0));
        assert_eq!(note_of(1), Some(36), "Rathian");
        assert_eq!(note_of(62), Some(37), "Pink Rathian");
        assert_eq!(note_of(0), None);
        assert!(TABLE.iter().all(|&(_, n, _)| n <= 72), "only the monster notes");
    }
}

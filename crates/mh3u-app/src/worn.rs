//! What the hunter's worn armor adds up to: defense, resistances, gem slots and skill points.

use mh3u_core::armor::ArmorStats;

/// The skill tree id of Torso Up, which doubles the skill points of the body piece.
pub const TORSO_UP: u8 = 1;
/// Equipment kind of body armor.
const BODY: u8 = 1;
/// Points at which a skill's first effect starts, and where its penalty starts.
pub const ACTIVE_AT: i32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillTotal {
    pub id: u8,
    pub points: i32,
    /// (armor kind, points from that piece after any Torso Up doubling).
    pub parts: Vec<(u8, i32)>,
}

impl SkillTotal {
    /// The first effect of the skill is on (10 points or more).
    pub fn active(&self) -> bool {
        self.points >= ACTIVE_AT
    }

    /// The skill's penalty is on (-10 points or less).
    pub fn penalty(&self) -> bool {
        self.points <= -ACTIVE_AT
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Summary {
    /// Sum of the pieces' base defense (before any upgrading).
    pub defense: u32,
    /// The same fully upgraded (a piece with no known maximum counts at its base defense).
    pub max_defense: u32,
    /// Fire, water, thunder, ice, dragon.
    pub resist: [i32; 5],
    /// Gem slots on the armor, not counting what is gemmed into them.
    pub gem_slots: u32,
    /// Skill totals, strongest first.
    pub skills: Vec<SkillTotal>,
    /// Torso Up is active, so the body piece counts double.
    pub torso_doubled: bool,
}

/// Add up armor pieces given as (equipment kind, stats). With Torso Up active (10 or more points), the body piece's points count
/// twice, for every skill but Torso Up itself.
pub fn summarize(pieces: &[(u8, &ArmorStats)]) -> Summary {
    let mut out = Summary::default();
    for &(_, a) in pieces {
        out.defense += u32::from(a.defense);
        out.max_defense += u32::from(a.max_defense.unwrap_or(a.defense));
        out.gem_slots += u32::from(a.slots);
        for (total, &r) in out.resist.iter_mut().zip(&a.resist) {
            *total += i32::from(r);
        }
    }
    let torso_points: i32 = pieces
        .iter()
        .flat_map(|(_, a)| a.skills.iter())
        .filter(|&&(id, _)| id == TORSO_UP)
        .map(|&(_, p)| i32::from(p))
        .sum();
    out.torso_doubled = torso_points >= ACTIVE_AT;

    let mut totals: Vec<SkillTotal> = Vec::new();
    for &(kind, a) in pieces {
        for &(id, pts) in &a.skills {
            let counted = if out.torso_doubled && kind == BODY && id != TORSO_UP {
                2 * i32::from(pts)
            } else {
                i32::from(pts)
            };
            match totals.iter_mut().find(|t| t.id == id) {
                Some(t) => {
                    t.points += counted;
                    t.parts.push((kind, counted));
                }
                None => totals.push(SkillTotal {
                    id,
                    points: counted,
                    parts: vec![(kind, counted)],
                }),
            }
        }
    }
    totals.sort_by_key(|t| (std::cmp::Reverse(t.points), t.id));
    out.skills = totals;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(defense: u8, slots: u8, resist: [i8; 5], skills: &[(u8, i8)]) -> ArmorStats {
        ArmorStats {
            defense,
            rarity: 1,
            slots,
            gender: None,
            class: None,
            resist,
            skills: skills.to_vec(),
            price: None,
            max_defense: Some(defense + 40),
        }
    }

    #[test]
    fn adds_up_defense_slots_resistances_and_skill_points() {
        let head = piece(10, 1, [1, -2, 0, 0, 3], &[(11, 4), (2, -2)]);
        let legs = piece(12, 2, [0, 1, 0, -1, 0], &[(11, 7), (13, 3)]);
        let s = summarize(&[(5, &head), (4, &legs)]);
        assert_eq!((s.defense, s.gem_slots, s.resist), (22, 3, [1, -1, 0, -1, 3]));
        assert_eq!(s.max_defense, 22 + 80, "each test piece gains 40");
        let mut unknown = piece(5, 0, [0; 5], &[]);
        unknown.max_defense = None;
        assert_eq!(
            summarize(&[(5, &unknown)]).max_defense,
            5,
            "a piece with no known maximum counts at its base"
        );
        assert_eq!(
            s.skills[0],
            SkillTotal {
                id: 11,
                points: 11,
                parts: vec![(5, 4), (4, 7)]
            }
        );
        assert!(s.skills[0].active());
        let poison = s.skills.iter().find(|t| t.id == 2).unwrap();
        assert_eq!(poison.points, -2);
        assert!(!poison.penalty(), "-2 is above the -10 where the penalty starts");
        assert!(!s.torso_doubled);
    }

    #[test]
    fn skills_are_listed_strongest_first() {
        let a = piece(0, 0, [0; 5], &[(5, 3), (6, 9), (7, -4)]);
        let order: Vec<u8> = summarize(&[(5, &a)]).skills.iter().map(|t| t.id).collect();
        assert_eq!(order, [6, 5, 7]);
    }

    #[test]
    fn torso_up_doubles_the_body_piece_but_not_itself() {
        let body = piece(0, 0, [0; 5], &[(11, 4), (TORSO_UP, 4)]);
        let legs = piece(0, 0, [0; 5], &[(TORSO_UP, 6), (11, 2)]);
        let s = summarize(&[(1, &body), (4, &legs)]);
        assert!(s.torso_doubled, "4 + 6 = 10 torso points");
        let attack = s.skills.iter().find(|t| t.id == 11).unwrap();
        assert_eq!(attack.points, 4 * 2 + 2);
        let torso = s.skills.iter().find(|t| t.id == TORSO_UP).unwrap();
        assert_eq!(torso.points, 10, "Torso Up itself is not doubled");
    }

    #[test]
    fn torso_up_below_ten_points_does_nothing() {
        let body = piece(0, 0, [0; 5], &[(11, 4), (TORSO_UP, 4)]);
        let s = summarize(&[(1, &body)]);
        assert!(!s.torso_doubled);
        assert_eq!(s.skills.iter().find(|t| t.id == 11).unwrap().points, 4);
    }

    #[test]
    fn nothing_worn_adds_up_to_nothing() {
        assert_eq!(summarize(&[]), Summary::default());
    }
}

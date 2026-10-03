//! The build manager's search: which head, body, arms, waist and legs pieces (and a talisman) reach the skill points you asked for.
//!
//! The search walks the slots one after another and gives up on a branch as soon as the points still to come cannot make up what is
//! missing. Only pieces that touch a wanted skill are tried, plus one filler per slot (the sturdiest piece that does no harm) so
//! the rest of the set is not left bare. The body piece goes last, because Torso Up doubles it (see `worn`).

use crate::worn::{self, TORSO_UP};
use mh3u_core::armor::{ArmorClass, ArmorStats, Gender};

/// Which armor the search may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pool {
    /// What the equipment box holds.
    Owned,
    /// Plus what the blacksmith is offering.
    OnOffer,
    /// Every piece in the game, for planning ahead: pieces you cannot get yet are marked as such.
    All,
}

impl Pool {
    pub fn next(self) -> Pool {
        match self {
            Pool::Owned => Pool::OnOffer,
            Pool::OnOffer => Pool::All,
            Pool::All => Pool::Owned,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Pool::Owned => "owned only",
            Pool::OnOffer => "owned + on offer",
            Pool::All => "everything (planning)",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Pool::Owned => "owned",
            Pool::OnOffer => "offered",
            Pool::All => "all",
        }
    }
}

/// A skill and the points wanted in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub skill: u8,
    pub points: i32,
}

/// A piece the search may use. `kind` is the equipment kind: 1 body, 2 arms, 3 waist, 4 legs, 5 head, 6 talisman.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub kind: u8,
    pub id: u16,
    /// In the equipment box now (otherwise it is only on offer at the blacksmith).
    pub owned: bool,
    pub stats: ArmorStats,
}

/// One set that reaches every target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Indexes into the pool, one per filled slot (the talisman, when used, is the last).
    pub pieces: Vec<usize>,
    pub defense: u32,
    pub owned: usize,
}

/// The order the slots are searched in: the body last.
const ORDER: [u8; 6] = [5, 2, 3, 4, 6, 1];
const BODY: u8 = 1;
/// Pieces tried per slot, best first.
const PER_SLOT: usize = 30;
/// Sturdy pieces tried in a slot on top of those.
const STURDY_EXTRA: usize = 10;

/// Sets that reach every target, sturdiest first (then those needing the fewest pieces still to be made). At most `limit`.
pub fn search(pool: &[Candidate], targets: &[Target], limit: usize) -> Vec<Found> {
    if targets.is_empty() || limit == 0 {
        return Vec::new();
    }
    let points_in = |c: &Candidate, skill: u8| -> i32 {
        c.stats
            .skills
            .iter()
            .filter(|&&(id, _)| id == skill)
            .map(|&(_, p)| i32::from(p))
            .sum()
    };
    let wanted = |c: &Candidate| targets.iter().any(|t| points_in(c, t.skill) != 0);
    let hurts = |c: &Candidate| targets.iter().any(|t| points_in(c, t.skill) < 0);
    let helps_torso = targets.iter().any(|t| t.skill != TORSO_UP);

    // The pieces tried in each slot, in search order.
    let mut slots: Vec<Vec<usize>> = Vec::new();
    for &kind in &ORDER {
        let of_kind: Vec<usize> = (0..pool.len()).filter(|&i| pool[i].kind == kind).collect();
        let relevance = |i: usize| -> i32 {
            let c = &pool[i];
            let own: i32 = targets.iter().map(|t| points_in(c, t.skill).max(0)).sum();
            own + if helps_torso && kind != BODY {
                points_in(c, TORSO_UP).max(0)
            } else {
                0
            }
        };
        let mut chosen: Vec<usize> = of_kind.iter().copied().filter(|&i| wanted(&pool[i]) || relevance(i) > 0).collect();
        chosen.sort_by_key(|&i| (std::cmp::Reverse(relevance(i)), std::cmp::Reverse(pool[i].stats.defense), i));
        // The best by relevance, plus a few of the sturdiest of the rest: a set is ranked by defense, so those can matter more.
        let mut rest = chosen.split_off(PER_SLOT.min(chosen.len()));
        rest.sort_by_key(|&i| (std::cmp::Reverse(pool[i].stats.defense), i));
        chosen.extend(rest.into_iter().take(STURDY_EXTRA));
        // sturdiest first, so good sets turn up early and the sets still to come can be judged against them
        chosen.sort_by_key(|&i| (std::cmp::Reverse(pool[i].stats.defense), i));
        let filler = of_kind
            .iter()
            .copied()
            .filter(|i| !chosen.contains(i) && !hurts(&pool[*i]))
            .max_by_key(|&i| (pool[i].stats.defense, pool[i].owned, std::cmp::Reverse(i)));
        chosen.extend(filler);
        slots.push(chosen);
    }
    // Where a slot has nothing to offer, leave it empty; the talisman is always optional.
    let empty = usize::MAX;
    for (s, list) in slots.iter_mut().enumerate() {
        if list.is_empty() || ORDER[s] == 6 {
            list.push(empty);
        }
    }

    // For each step, the most the slots from there on can still add to each target (the body counts twice, as Torso Up may double it).
    let steps = ORDER.len();
    let mut ahead = vec![vec![0i32; targets.len()]; steps + 1];
    for s in (0..steps).rev() {
        for (t, target) in targets.iter().enumerate() {
            let best = slots[s]
                .iter()
                .filter(|&&i| i != empty)
                .map(|&i| points_in(&pool[i], target.skill))
                .max()
                .unwrap_or(0)
                .max(0);
            let factor = if ORDER[s] == BODY && target.skill != TORSO_UP { 2 } else { 1 };
            ahead[s][t] = ahead[s + 1][t] + best * factor;
        }
    }

    // The most defense the slots from each step on can still add.
    let mut sturdy = vec![0u32; steps + 1];
    for s in (0..steps).rev() {
        let best = slots[s]
            .iter()
            .filter(|&&i| i != empty)
            .map(|&i| u32::from(pool[i].stats.defense))
            .max()
            .unwrap_or(0);
        sturdy[s] = sturdy[s + 1] + best;
    }

    let mut out: Vec<Found> = Vec::new();
    let mut picked: Vec<usize> = Vec::new();
    let mut so_far = vec![0i32; targets.len()];
    let mut search = Walk {
        pool,
        targets,
        slots: &slots,
        ahead: &ahead,
        sturdy: &sturdy,
        empty,
        limit,
        // once `limit` sets are in hand, a branch that cannot beat the weakest of them is dropped
        floor: 0,
    };
    search.walk(0, 0, &mut picked, &mut so_far, &mut out);
    rank(&mut out);
    out.truncate(limit);
    out
}

struct Walk<'a> {
    pool: &'a [Candidate],
    targets: &'a [Target],
    slots: &'a [Vec<usize>],
    ahead: &'a [Vec<i32>],
    sturdy: &'a [u32],
    empty: usize,
    limit: usize,
    floor: u32,
}

impl Walk<'_> {
    fn walk(&mut self, step: usize, defense: u32, picked: &mut Vec<usize>, so_far: &mut Vec<i32>, out: &mut Vec<Found>) {
        let (pool, targets, empty) = (self.pool, self.targets, self.empty);
        if step == self.slots.len() {
            let parts: Vec<(u8, &ArmorStats)> = picked.iter().map(|&i| (pool[i].kind, &pool[i].stats)).collect();
            let summary = worn::summarize(&parts);
            let reached = targets.iter().all(|t| {
                summary
                    .skills
                    .iter()
                    .find(|s| s.id == t.skill)
                    .is_some_and(|s| s.points >= t.points)
            });
            if reached {
                out.push(Found {
                    pieces: picked.clone(),
                    defense: summary.defense,
                    owned: picked.iter().filter(|&&i| pool[i].owned).count(),
                });
                // keep the list from growing without bound
                if out.len() >= self.limit * 8 {
                    rank(out);
                    out.truncate(self.limit);
                    self.floor = out.last().map_or(0, |f| f.defense);
                }
            }
            return;
        }
        for &i in &self.slots[step] {
            let own_defense = if i == empty { 0 } else { u32::from(pool[i].stats.defense) };
            if self.floor > 0 && defense + own_defense + self.sturdy[step + 1] < self.floor {
                continue;
            }
            // the body is searched last, so the running totals before it carry no doubling
            let gain: Vec<i32> = targets
                .iter()
                .map(|t| {
                    if i == empty {
                        0
                    } else {
                        pool[i]
                            .stats
                            .skills
                            .iter()
                            .filter(|&&(id, _)| id == t.skill)
                            .map(|&(_, p)| i32::from(p))
                            .sum()
                    }
                })
                .collect();
            let is_body = ORDER[step] == BODY;
            let reachable = targets.iter().enumerate().all(|(t, target)| {
                let own = if is_body && target.skill != TORSO_UP {
                    2 * gain[t].max(0)
                } else {
                    gain[t]
                };
                so_far[t] + own + self.ahead[step + 1][t] >= target.points
            });
            if !reachable {
                continue;
            }
            for (t, g) in gain.iter().enumerate() {
                so_far[t] += g;
            }
            if i != empty {
                picked.push(i);
            }
            self.walk(step + 1, defense + own_defense, picked, so_far, out);
            if i != empty {
                picked.pop();
            }
            for (t, g) in gain.iter().enumerate() {
                so_far[t] -= g;
            }
        }
    }
}

/// Sturdiest first, then the fewest pieces still to get, then the fewest pieces overall.
fn rank(found: &mut Vec<Found>) {
    found.sort_by_key(|f| {
        (
            std::cmp::Reverse(f.defense),
            std::cmp::Reverse(f.owned),
            f.pieces.len(),
            f.pieces.clone(),
        )
    });
    found.dedup_by(|a, b| a.pieces == b.pieces);
}

/// The wanted skills and the options, kept in a small text file per hunter: `skill ID POINTS`, `offered 0|1`, `talisman 0|1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub targets: Vec<Target>,
    /// Use pieces the blacksmith is offering as well as the ones you own.
    pub pool: Pool,
    /// Try the talismans you own.
    pub use_talisman: bool,
    /// Only pieces this gender can wear (`None`: any).
    pub gender: Option<Gender>,
    /// Only pieces for this class (`None`: any).
    pub class: Option<ArmorClass>,
}

/// Whether the filters let a piece through. Pieces for both genders (or classes) always pass, and so does a piece whose flags
/// are unknown.
pub fn usable(stats: &ArmorStats, gender: Option<Gender>, class: Option<ArmorClass>) -> bool {
    let gender_ok = match (gender, stats.gender) {
        (Some(want), Some(have)) => have == Gender::Both || have == want,
        _ => true,
    };
    let class_ok = match (class, stats.class) {
        (Some(want), Some(have)) => have == ArmorClass::Both || have == want,
        _ => true,
    };
    gender_ok && class_ok
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            targets: Vec::new(),
            pool: Pool::OnOffer,
            use_talisman: true,
            gender: None,
            class: None,
        }
    }
}

impl Settings {
    pub fn parse(text: &str) -> Settings {
        let mut out = Settings::default();
        for line in text.lines() {
            let mut words = line.split_whitespace();
            match (words.next(), words.next(), words.next()) {
                (Some("skill"), Some(id), Some(points)) => {
                    if let (Ok(skill), Ok(points)) = (id.parse(), points.parse())
                        && !out.targets.iter().any(|t: &Target| t.skill == skill)
                    {
                        out.targets.push(Target { skill, points });
                    }
                }
                // `offered 0|1` is how an earlier version kept this
                (Some("offered"), Some(v), _) => out.pool = if v == "0" { Pool::Owned } else { Pool::OnOffer },
                (Some("pool"), Some(v), _) => {
                    out.pool = match v {
                        "owned" => Pool::Owned,
                        "all" => Pool::All,
                        _ => Pool::OnOffer,
                    }
                }
                (Some("talisman"), Some(v), _) => out.use_talisman = v != "0",
                (Some("gender"), Some(v), _) => {
                    out.gender = match v {
                        "male" => Some(Gender::Male),
                        "female" => Some(Gender::Female),
                        _ => None,
                    }
                }
                (Some("class"), Some(v), _) => {
                    out.class = match v {
                        "blademaster" => Some(ArmorClass::Blademaster),
                        "gunner" => Some(ArmorClass::Gunner),
                        _ => None,
                    }
                }
                _ => {}
            }
        }
        out
    }

    pub fn format(&self) -> String {
        let mut text: String = self.targets.iter().map(|t| format!("skill {} {}\n", t.skill, t.points)).collect();
        text += &format!("pool {}\ntalisman {}\n", self.pool.name(), u8::from(self.use_talisman));
        text += match self.gender {
            Some(Gender::Male) => "gender male\n",
            Some(Gender::Female) => "gender female\n",
            _ => "",
        };
        text += match self.class {
            Some(ArmorClass::Blademaster) => "class blademaster\n",
            Some(ArmorClass::Gunner) => "class gunner\n",
            _ => "",
        };
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(defense: u8, skills: &[(u8, i8)]) -> ArmorStats {
        ArmorStats {
            defense,
            rarity: 1,
            slots: 0,
            gender: None,
            class: None,
            resist: [0; 5],
            skills: skills.to_vec(),
            price: None,
        }
    }

    fn piece(kind: u8, id: u16, defense: u8, skills: &[(u8, i8)]) -> Candidate {
        Candidate {
            kind,
            id,
            owned: true,
            stats: stats(defense, skills),
        }
    }

    const ATTACK: u8 = 11;
    const POISON: u8 = 2;

    /// Five slots with a plain, sturdy piece each and one Attack piece in head, arms and waist.
    fn pool() -> Vec<Candidate> {
        let mut pool = Vec::new();
        for kind in [5, 1, 2, 3, 4] {
            pool.push(piece(kind, 1, 10, &[]));
        }
        pool.push(piece(5, 2, 8, &[(ATTACK, 4)]));
        pool.push(piece(2, 2, 7, &[(ATTACK, 3)]));
        pool.push(piece(3, 2, 6, &[(ATTACK, 3)]));
        pool
    }

    fn ids(pool: &[Candidate], found: &Found) -> Vec<(u8, u16)> {
        let mut v: Vec<(u8, u16)> = found.pieces.iter().map(|&i| (pool[i].kind, pool[i].id)).collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn finds_the_sets_that_reach_the_points_and_fills_the_other_slots() {
        let pool = pool();
        let found = search(&pool, &[Target { skill: ATTACK, points: 10 }], 10);
        assert_eq!(found.len(), 1, "only head + arms + waist together reach 10");
        assert_eq!(ids(&pool, &found[0]), [(1, 1), (2, 2), (3, 2), (4, 1), (5, 2)]);
        assert_eq!(
            found[0].defense,
            10 + 7 + 6 + 10 + 8,
            "the body and legs are the sturdy plain pieces"
        );
    }

    #[test]
    fn nothing_is_found_when_the_points_cannot_be_reached() {
        let pool = pool();
        assert!(search(&pool, &[Target { skill: ATTACK, points: 11 }], 10).is_empty());
        assert!(search(&pool, &[Target { skill: POISON, points: 1 }], 10).is_empty());
        assert!(search(&pool, &[], 10).is_empty());
    }

    #[test]
    fn sturdier_sets_come_first() {
        let mut pool = pool();
        pool.push(piece(5, 3, 20, &[(ATTACK, 4)]));
        let found = search(&pool, &[Target { skill: ATTACK, points: 10 }], 10);
        assert_eq!(found.len(), 2);
        assert!(found[0].defense > found[1].defense);
        assert_eq!(pool[found[0].pieces[0]].id, 3, "the sturdier head leads");
    }

    #[test]
    fn a_piece_that_costs_a_wanted_skill_is_not_used_as_filler() {
        let mut pool = pool();
        pool.push(piece(4, 9, 90, &[(ATTACK, -3)]));
        let found = search(&pool, &[Target { skill: ATTACK, points: 10 }], 10);
        assert_eq!(found.len(), 1);
        assert!(!ids(&pool, &found[0]).contains(&(4, 9)));
    }

    #[test]
    fn torso_up_doubles_the_body_piece_in_the_search() {
        let mut pool: Vec<Candidate> = [5, 2, 3, 4].iter().map(|&kind| piece(kind, 1, 10, &[])).collect();
        pool.push(piece(1, 2, 5, &[(ATTACK, 5)]));
        let target = [Target { skill: ATTACK, points: 10 }];
        // the body's 5 points alone are not enough
        assert!(search(&pool, &target, 20).is_empty());
        // with 10 points of Torso Up on the head the body counts double
        pool.push(piece(5, 3, 5, &[(TORSO_UP, 10)]));
        let found = search(&pool, &target, 20);
        assert_eq!(found.len(), 1);
        assert_eq!(ids(&pool, &found[0]), [(1, 2), (2, 1), (3, 1), (4, 1), (5, 3)]);
    }

    #[test]
    fn a_talisman_adds_its_points_and_is_optional() {
        let mut pool = pool();
        pool.push(piece(6, 1, 0, &[(ATTACK, 10)]));
        let found = search(&pool, &[Target { skill: ATTACK, points: 10 }], 20);
        assert!(found.iter().any(|f| f.pieces.iter().any(|&i| pool[i].kind == 6)));
        assert!(
            found.iter().any(|f| f.pieces.iter().all(|&i| pool[i].kind != 6)),
            "the armor-only set is still there"
        );
    }

    /// Every slot full of pieces that carry the wanted skill: the case where nearly every combination reaches the goal.
    #[test]
    fn a_big_pool_where_almost_everything_qualifies_is_still_quick() {
        let mut pool = Vec::new();
        for kind in [5, 1, 2, 3, 4] {
            for id in 0..300u16 {
                pool.push(piece(
                    kind,
                    id,
                    1 + (id * 7 % 90) as u8,
                    &[(ATTACK, 2 + (id % 3) as i8), (POISON, (id % 5) as i8 - 2)],
                ));
            }
        }
        let started = std::time::Instant::now();
        let found = search(&pool, &[Target { skill: ATTACK, points: 10 }], 300);
        assert_eq!(found.len(), 300);
        assert!(found.windows(2).all(|w| w[0].defense >= w[1].defense), "sturdiest first");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn the_result_list_is_capped() {
        let mut pool = pool();
        for id in 10..20 {
            pool.push(piece(5, id, id as u8, &[(ATTACK, 4)]));
        }
        assert_eq!(search(&pool, &[Target { skill: ATTACK, points: 10 }], 3).len(), 3);
    }

    #[test]
    fn settings_round_trip_and_ignore_bad_lines() {
        let s = Settings {
            targets: vec![Target { skill: 11, points: 10 }, Target { skill: 37, points: 15 }],
            pool: Pool::All,
            use_talisman: true,
            gender: Some(Gender::Female),
            class: Some(ArmorClass::Gunner),
        };
        assert_eq!(Settings::parse(&s.format()), s);
        let parsed = Settings::parse("skill x 3\nskill 5 10\nskill 5 20\nnonsense\n");
        assert_eq!(
            parsed.targets,
            [Target { skill: 5, points: 10 }],
            "bad and repeated lines are skipped"
        );
        assert_eq!(Settings::parse(""), Settings::default());
    }

    #[test]
    fn the_filters_let_pieces_for_both_through() {
        let mut a = stats(1, &[]);
        a.gender = Some(Gender::Female);
        a.class = Some(ArmorClass::Both);
        assert!(usable(&a, None, None));
        assert!(usable(&a, Some(Gender::Female), Some(ArmorClass::Gunner)));
        assert!(!usable(&a, Some(Gender::Male), None));
        a.gender = Some(Gender::Both);
        a.class = Some(ArmorClass::Blademaster);
        assert!(usable(&a, Some(Gender::Male), Some(ArmorClass::Blademaster)));
        assert!(!usable(&a, None, Some(ArmorClass::Gunner)));
        a.gender = None;
        assert!(usable(&a, Some(Gender::Male), None), "unknown flags are not held against a piece");
    }

    #[test]
    fn an_older_settings_file_still_reads() {
        assert_eq!(Settings::parse("offered 0\n").pool, Pool::Owned);
        assert_eq!(Settings::parse("offered 1\n").pool, Pool::OnOffer);
        assert_eq!(Settings::parse("pool all\n").pool, Pool::All);
        assert_eq!(Pool::Owned.next().next().next(), Pool::Owned);
    }
}

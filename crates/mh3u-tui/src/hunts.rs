//! The hunt plan: which monsters to hunt, in which rank, to get the materials the wishlist is still short of.
//!
//! Hunts are chosen one at a time, each time the monster and rank that cover the most of what is still missing (then the best
//! chances), until nothing more can be covered. So the first step is the best single hunt, and later steps pick up what it left.

use mh3u_core::drops::{Rank, Source};

/// Which ranks the plan may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankFilter {
    All,
    Only(Rank),
}

impl RankFilter {
    pub fn next(self) -> RankFilter {
        match self {
            RankFilter::All => RankFilter::Only(Rank::Low),
            RankFilter::Only(Rank::Low) => RankFilter::Only(Rank::High),
            RankFilter::Only(Rank::High) => RankFilter::Only(Rank::G),
            RankFilter::Only(Rank::G) => RankFilter::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RankFilter::All => "all ranks",
            RankFilter::Only(rank) => rank.label(),
        }
    }

    fn allows(self, rank: Rank) -> bool {
        match self {
            RankFilter::All => true,
            RankFilter::Only(only) => only == rank,
        }
    }
}

/// One missing material a hunt can cover, and the best way that hunt gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cover {
    pub item: u16,
    /// How many are still missing.
    pub missing: u32,
    /// The best chance among this monster's drops of the item in this rank.
    pub best: Source,
}

/// One hunt: this monster in this rank covers these materials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub monster: u16,
    pub rank: Rank,
    pub covers: Vec<Cover>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    pub steps: Vec<Step>,
    /// Missing materials no monster in the allowed ranks drops (they come from gathering, the shop or another rank).
    pub unsourced: Vec<u16>,
}

/// Plan the hunts for `missing` (item, how many are missing). `sources(item)` lists every way to get an item; `hunt_worthy` says
/// whether a monster may be hunted at all (it has a name the player knows).
pub fn plan<'a>(
    missing: &[(u16, u32)],
    sources: impl Fn(u16) -> &'a [Source],
    allowed: RankFilter,
    hunt_worthy: impl Fn(u16) -> bool,
) -> Plan {
    // For every (monster, rank): item -> the best source of it.
    let mut options: Vec<((u16, Rank), Vec<Cover>)> = Vec::new();
    let mut unsourced = Vec::new();
    for &(item, need) in missing {
        let mut any = false;
        for s in sources(item).iter().filter(|s| allowed.allows(s.rank) && hunt_worthy(s.monster)) {
            any = true;
            let key = (s.monster, s.rank);
            let at = options.iter().position(|(k, _)| *k == key).unwrap_or_else(|| {
                options.push((key, Vec::new()));
                options.len() - 1
            });
            let covers = &mut options[at].1;
            match covers.iter_mut().find(|c| c.item == item) {
                Some(c) if s.percent > c.best.percent => c.best = *s,
                Some(_) => {}
                None => covers.push(Cover {
                    item,
                    missing: need,
                    best: *s,
                }),
            }
        }
        if !any {
            unsourced.push(item);
        }
    }

    let mut steps = Vec::new();
    let mut left: Vec<u16> = missing.iter().map(|&(item, _)| item).filter(|i| !unsourced.contains(i)).collect();
    while !left.is_empty() {
        // the option covering the most of what is left, then with the best total chance, then the lowest monster id
        let best = options
            .iter()
            .map(|((monster, rank), covers)| {
                let useful: Vec<Cover> = covers.iter().copied().filter(|c| left.contains(&c.item)).collect();
                let chance: u32 = useful.iter().map(|c| u32::from(c.best.percent)).sum();
                ((*monster, *rank), useful, chance)
            })
            .filter(|(_, useful, _)| !useful.is_empty())
            .max_by_key(|((monster, rank), useful, chance)| (useful.len(), *chance, std::cmp::Reverse((*monster, *rank))));
        let Some(((monster, rank), mut covers, _)) = best else { break };
        covers.sort_by_key(|c| (std::cmp::Reverse(c.best.percent), c.item));
        left.retain(|item| !covers.iter().any(|c| c.item == *item));
        steps.push(Step { monster, rank, covers });
    }
    Plan { steps, unsourced }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mh3u_core::drops::Method;
    use std::collections::HashMap;

    fn src(monster: u16, rank: Rank, method: Method, percent: u8) -> Source {
        Source {
            monster,
            rank,
            method,
            percent,
        }
    }

    fn run(table: &HashMap<u16, Vec<Source>>, missing: &[(u16, u32)], allowed: RankFilter) -> Plan {
        plan(missing, |item| table.get(&item).map_or(&[], Vec::as_slice), allowed, |_| true)
    }

    /// Item 1 and 2 drop from monster 10 (low rank); item 2 and 3 from monster 20; item 4 only from monster 30 in high rank.
    fn table() -> HashMap<u16, Vec<Source>> {
        HashMap::from([
            (1, vec![src(10, Rank::Low, Method::BodyCarve, 30)]),
            (
                2,
                vec![src(10, Rank::Low, Method::TailCarve, 20), src(20, Rank::Low, Method::BodyCarve, 50)],
            ),
            (3, vec![src(20, Rank::Low, Method::Shiny, 5)]),
            (4, vec![src(30, Rank::High, Method::Capture, 10)]),
        ])
    }

    #[test]
    fn the_first_hunt_covers_the_most_and_later_ones_pick_up_the_rest() {
        let plan = run(&table(), &[(1, 2), (2, 1), (3, 1), (4, 1)], RankFilter::All);
        let steps: Vec<(u16, Rank, Vec<u16>)> = plan
            .steps
            .iter()
            .map(|s| (s.monster, s.rank, s.covers.iter().map(|c| c.item).collect()))
            .collect();
        // monster 10 covers items 1, 2 (chances 30 + 20 = 50), monster 20 covers 2, 3 (50 + 5 = 55): 20 wins the tie on count
        assert_eq!(steps[0].0, 20);
        assert_eq!(steps[0].2, [2, 3]);
        assert_eq!(steps[1], (10, Rank::Low, vec![1]));
        assert_eq!(steps[2], (30, Rank::High, vec![4]));
        assert!(plan.unsourced.is_empty());
    }

    #[test]
    fn a_rank_filter_drops_what_that_rank_cannot_give() {
        let plan = run(&table(), &[(1, 1), (4, 1)], RankFilter::Only(Rank::Low));
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.unsourced, [4], "item 4 only drops in high rank");
        let high = run(&table(), &[(1, 1), (4, 1)], RankFilter::Only(Rank::High));
        assert_eq!(high.unsourced, [1]);
        assert_eq!(high.steps[0].monster, 30);
    }

    #[test]
    fn a_cover_keeps_the_best_chance_among_the_monsters_drops() {
        let plan = run(&table(), &[(2, 3)], RankFilter::All);
        let cover = plan.steps[0].covers[0];
        assert_eq!(plan.steps[0].monster, 20, "50% beats 20%");
        assert_eq!((cover.item, cover.missing, cover.best.percent), (2, 3, 50));
        let two = HashMap::from([(
            7,
            vec![src(1, Rank::Low, Method::BodyCarve, 10), src(1, Rank::Low, Method::Capture, 40)],
        )]);
        let plan = run(&two, &[(7, 1)], RankFilter::All);
        assert_eq!(plan.steps[0].covers[0].best.method, Method::Capture);
    }

    #[test]
    fn monsters_that_may_not_be_hunted_are_left_out() {
        let t = table();
        let plan = plan(&[(1, 1)], |i| t.get(&i).map_or(&[], Vec::as_slice), RankFilter::All, |m| m != 10);
        assert!(plan.steps.is_empty());
        assert_eq!(plan.unsourced, [1]);
    }

    #[test]
    fn nothing_missing_is_an_empty_plan_and_the_filter_cycles() {
        assert_eq!(run(&table(), &[], RankFilter::All), Plan::default());
        let mut f = RankFilter::All;
        for _ in 0..4 {
            f = f.next();
        }
        assert_eq!(f, RankFilter::All);
    }
}

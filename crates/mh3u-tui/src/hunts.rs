//! The hunt plan: which monsters to hunt, in which rank, to get the materials the wishlist is still short of.
//!
//! Hunts are chosen one at a time, each time the monster and rank that cover the most of what is still missing (then the best
//! chances), until nothing more can be covered. So the first step is the best single hunt, and later steps pick up what it left.

use mh3u_core::drops::{Method, Rank, Source};

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

/// Where a step of the plan takes you.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    /// Hunt this monster (name id) in this rank and carve, capture or break it.
    Monster { monster: u16, rank: Rank },
    /// Do this quest (id) and take its rewards.
    Quest(u16),
}

/// How a material comes from a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    Drop(Method),
    /// A quest reward: from the main or the second box, in this quantity.
    Reward {
        second_box: bool,
        quantity: u8,
    },
}

/// One missing material a step can cover, and the best way that step gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cover {
    pub item: u16,
    /// How many are still missing.
    pub missing: u32,
    /// The best chance in percent. For a quest reward 0 means every time.
    pub percent: u8,
    pub how: How,
}

impl Cover {
    /// The chance as a number to add up and compare: a reward given every time counts as 100.
    fn chance(&self) -> u32 {
        match self.how {
            How::Reward { .. } if self.percent == 0 => 100,
            _ => u32::from(self.percent),
        }
    }
}

/// One step: this monster in this rank, or this quest, covers these materials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub origin: Origin,
    pub covers: Vec<Cover>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    pub steps: Vec<Step>,
    /// Missing materials no hunt or quest in the allowed ranks gives (they come from gathering, the shop or another rank).
    pub unsourced: Vec<u16>,
}

/// What a quest gives: for each item, its best chance in one of the boxes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestOffer {
    pub id: u16,
    /// (item, percent, quantity, second box).
    pub rewards: Vec<(u16, u8, u8, bool)>,
}

/// Plan the steps for `missing` (item, how many are missing). `sources(item)` lists every monster drop of an item and `hunt_worthy`
/// says whether a monster may be hunted at all (it has a name the player knows). `quests` are used only when every rank is allowed:
/// a quest has no rank of its own that is known.
pub fn plan<'a>(
    missing: &[(u16, u32)],
    sources: impl Fn(u16) -> &'a [Source],
    quests: &[QuestOffer],
    allowed: RankFilter,
    hunt_worthy: impl Fn(u16) -> bool,
) -> Plan {
    let mut options: Vec<(Origin, Vec<Cover>)> = Vec::new();
    let mut offer = |origin: Origin, cover: Cover| {
        let at = options.iter().position(|(o, _)| *o == origin).unwrap_or_else(|| {
            options.push((origin, Vec::new()));
            options.len() - 1
        });
        let covers = &mut options[at].1;
        match covers.iter_mut().find(|c| c.item == cover.item) {
            Some(c) if cover.chance() > c.chance() => *c = cover,
            Some(_) => {}
            None => covers.push(cover),
        }
    };
    let mut unsourced = Vec::new();
    for &(item, need) in missing {
        let mut any = false;
        for s in sources(item).iter().filter(|s| allowed.allows(s.rank) && hunt_worthy(s.monster)) {
            any = true;
            let origin = Origin::Monster {
                monster: s.monster,
                rank: s.rank,
            };
            offer(
                origin,
                Cover {
                    item,
                    missing: need,
                    percent: s.percent,
                    how: How::Drop(s.method),
                },
            );
        }
        if allowed == RankFilter::All {
            for q in quests {
                for &(reward, percent, quantity, second_box) in q.rewards.iter().filter(|r| r.0 == item) {
                    any = true;
                    offer(
                        Origin::Quest(q.id),
                        Cover {
                            item,
                            missing: need,
                            percent,
                            how: How::Reward { second_box, quantity },
                        },
                    );
                    let _ = reward;
                }
            }
        }
        if !any {
            unsourced.push(item);
        }
    }

    let mut steps = Vec::new();
    let mut left: Vec<u16> = missing.iter().map(|&(item, _)| item).filter(|i| !unsourced.contains(i)).collect();
    while !left.is_empty() {
        // the option covering the most of what is left, then with the best total chance, then the first in order
        let best = options
            .iter()
            .map(|(origin, covers)| {
                let useful: Vec<Cover> = covers.iter().copied().filter(|c| left.contains(&c.item)).collect();
                let chance: u32 = useful.iter().map(Cover::chance).sum();
                (*origin, useful, chance)
            })
            .filter(|(_, useful, _)| !useful.is_empty())
            .max_by_key(|(origin, useful, chance)| (useful.len(), *chance, std::cmp::Reverse(*origin)));
        let Some((origin, mut covers, _)) = best else { break };
        covers.sort_by_key(|c| (std::cmp::Reverse(c.chance()), c.item));
        left.retain(|item| !covers.iter().any(|c| c.item == *item));
        steps.push(Step { origin, covers });
    }
    Plan { steps, unsourced }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        plan(missing, |item| table.get(&item).map_or(&[], Vec::as_slice), &[], allowed, |_| true)
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

    fn monster(step: &Step) -> (u16, Rank) {
        match step.origin {
            Origin::Monster { monster, rank } => (monster, rank),
            Origin::Quest(id) => panic!("quest {id}, not a monster"),
        }
    }

    #[test]
    fn the_first_hunt_covers_the_most_and_later_ones_pick_up_the_rest() {
        let plan = run(&table(), &[(1, 2), (2, 1), (3, 1), (4, 1)], RankFilter::All);
        let steps: Vec<((u16, Rank), Vec<u16>)> = plan
            .steps
            .iter()
            .map(|s| (monster(s), s.covers.iter().map(|c| c.item).collect()))
            .collect();
        // monster 10 covers items 1, 2 (chances 30 + 20 = 50), monster 20 covers 2, 3 (50 + 5 = 55): 20 wins the tie on count
        assert_eq!(steps[0], ((20, Rank::Low), vec![2, 3]));
        assert_eq!(steps[1], ((10, Rank::Low), vec![1]));
        assert_eq!(steps[2], ((30, Rank::High), vec![4]));
        assert!(plan.unsourced.is_empty());
    }

    #[test]
    fn a_rank_filter_drops_what_that_rank_cannot_give() {
        let plan = run(&table(), &[(1, 1), (4, 1)], RankFilter::Only(Rank::Low));
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.unsourced, [4], "item 4 only drops in high rank");
        let high = run(&table(), &[(1, 1), (4, 1)], RankFilter::Only(Rank::High));
        assert_eq!(high.unsourced, [1]);
        assert_eq!(monster(&high.steps[0]).0, 30);
    }

    #[test]
    fn a_cover_keeps_the_best_chance_among_the_monsters_drops() {
        let plan = run(&table(), &[(2, 3)], RankFilter::All);
        let cover = plan.steps[0].covers[0];
        assert_eq!(monster(&plan.steps[0]).0, 20, "50% beats 20%");
        assert_eq!((cover.item, cover.missing, cover.percent), (2, 3, 50));
        let two = HashMap::from([(
            7,
            vec![src(1, Rank::Low, Method::BodyCarve, 10), src(1, Rank::Low, Method::Capture, 40)],
        )]);
        let plan = run(&two, &[(7, 1)], RankFilter::All);
        assert_eq!(plan.steps[0].covers[0].how, How::Drop(Method::Capture));
    }

    #[test]
    fn monsters_that_may_not_be_hunted_are_left_out() {
        let t = table();
        let plan = plan(
            &[(1, 1)],
            |i| t.get(&i).map_or(&[], Vec::as_slice),
            &[],
            RankFilter::All,
            |m| m != 10,
        );
        assert!(plan.steps.is_empty());
        assert_eq!(plan.unsourced, [1]);
    }

    fn quest(id: u16, rewards: &[(u16, u8, u8, bool)]) -> QuestOffer {
        QuestOffer {
            id,
            rewards: rewards.to_vec(),
        }
    }

    fn with_quests(missing: &[(u16, u32)], quests: &[QuestOffer], allowed: RankFilter) -> Plan {
        let t = table();
        plan(missing, |i| t.get(&i).map_or(&[], Vec::as_slice), quests, allowed, |_| true)
    }

    #[test]
    fn a_quest_that_gives_several_materials_is_one_step_and_covers_what_no_monster_drops() {
        // item 9 is an ore: no monster drops it; the quest gives 9 (every time) and 1
        let quests = [quest(500, &[(9, 0, 3, false), (1, 40, 1, true)])];
        let plan = with_quests(&[(1, 1), (9, 2)], &quests, RankFilter::All);
        assert!(plan.unsourced.is_empty(), "the quest supplies the ore");
        assert_eq!(plan.steps[0].origin, Origin::Quest(500), "two items beat monster 10's one");
        let covers = &plan.steps[0].covers;
        assert_eq!(covers[0].item, 9, "an every-time reward counts as 100% and comes first");
        assert_eq!(
            covers[0].how,
            How::Reward {
                second_box: false,
                quantity: 3
            }
        );
        assert_eq!(
            covers[1].how,
            How::Reward {
                second_box: true,
                quantity: 1
            }
        );
        assert_eq!(plan.steps.len(), 1);
    }

    #[test]
    fn quests_are_left_out_when_one_rank_is_chosen_because_a_quest_has_no_rank_of_its_own() {
        let quests = [quest(500, &[(9, 0, 3, false)])];
        let low = with_quests(&[(9, 1)], &quests, RankFilter::Only(Rank::Low));
        assert_eq!(low.unsourced, [9]);
        assert!(low.steps.is_empty());
    }

    #[test]
    fn a_monster_hunt_still_wins_when_it_gives_more_than_the_quests_do() {
        let quests = [quest(500, &[(1, 5, 1, false)])];
        let plan = with_quests(&[(2, 1), (3, 1)], &quests, RankFilter::All);
        assert_eq!(monster(&plan.steps[0]), (20, Rank::Low), "monster 20 covers items 2 and 3");
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

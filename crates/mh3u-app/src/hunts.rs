//! The hunt plan: which monsters to hunt, in which rank, to get the materials the wishlist is still short of.
//!
//! Hunts are chosen one at a time, until nothing more can be covered. With the goal of **fewest steps** each time it is the monster and rank
//! that cover the most of what is still missing (then the one that needs the fewest runs), so the first step is the best single hunt and later
//! steps pick up what it left. With the goal of **fewest runs** it is the hunt that gives the most per run: a hunt takes only the materials
//! it gives quickly, and leaves a rare one to a hunt that gives it better. Each material also gets an estimate of how many runs it takes (see
//! [`expected_runs`]).

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

/// What the plan tries to keep small.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Goal {
    /// The fewest hunts and quests: each step is the one covering the most that is still missing.
    #[default]
    FewestSteps,
    /// The fewest runs in all: each step is the one that gives the most per run, even when it covers little.
    FewestRuns,
}

impl Goal {
    pub fn next(self) -> Goal {
        match self {
            Goal::FewestSteps => Goal::FewestRuns,
            Goal::FewestRuns => Goal::FewestSteps,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Goal::FewestSteps => "fewest steps",
            Goal::FewestRuns => "fewest runs",
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

/// How many times a list of drops is rolled in one run. These are assumptions, not read from the game: a large monster is carved three
/// times, its tail once, a capture pays out twice and a break once. Shiny drops are counted once.
fn drop_rolls(method: Method) -> u32 {
    match method {
        Method::BodyCarve => 3,
        Method::TailCarve | Method::Shiny | Method::Break(_) => 1,
        Method::Capture => 2,
    }
}

/// How many times a quest's reward box is rolled (an assumption too). A reward with chance 0 is given every time, once.
const MAIN_BOX_ROLLS: u32 = 3;
const SECOND_BOX_ROLLS: u32 = 1;

/// Runs of a hunt or quest it takes on average to get `missing` of a material: the number missing over what one run gives on
/// average (rolls x chance x quantity), rounded up, at least one. `None` when a run never gives it.
pub fn expected_runs(missing: u32, percent: u8, how: How) -> Option<u32> {
    let (rolls, chance, quantity) = match how {
        How::Drop(method) => (drop_rolls(method), f64::from(percent) / 100.0, 1),
        How::Reward { .. } if percent == 0 => (1, 1.0, 1),
        How::Reward { second_box, .. } => (
            if second_box { SECOND_BOX_ROLLS } else { MAIN_BOX_ROLLS },
            f64::from(percent) / 100.0,
            1,
        ),
    };
    let quantity = match how {
        How::Reward { quantity, .. } => u32::from(quantity.max(1)),
        How::Drop(_) => quantity,
    };
    let per_run = f64::from(rolls) * chance * f64::from(quantity);
    (per_run > 0.0).then(|| ((f64::from(missing) / per_run).ceil() as u32).max(1))
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
    /// Runs it takes on average to get what is missing from this step alone (`None`: never).
    pub runs: Option<u32>,
}

impl Cover {
    /// The chance as a number to add up and compare: a reward given every time counts as 100.
    fn chance(&self) -> u32 {
        match self.how {
            How::Reward { .. } if self.percent == 0 => 100,
            _ => u32::from(self.percent),
        }
    }

    /// Fewer runs is better, then a higher chance. A way that never gives it is the worst.
    fn better_than(&self, other: &Cover) -> bool {
        let (mine, theirs) = (self.runs.unwrap_or(u32::MAX), other.runs.unwrap_or(u32::MAX));
        mine < theirs || (mine == theirs && self.chance() > other.chance())
    }
}

/// One step: this monster in this rank, or this quest, covers these materials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub origin: Origin,
    pub covers: Vec<Cover>,
}

impl Step {
    /// Runs this step takes on average to give everything it covers: the slowest of its materials, since one run rolls them all.
    pub fn runs(&self) -> u32 {
        runs_of(&self.covers)
    }
}

fn runs_of(covers: &[Cover]) -> u32 {
    covers.iter().map(|c| c.runs.unwrap_or(0)).max().unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    pub steps: Vec<Step>,
    /// Missing materials no hunt or quest in the allowed ranks gives (they come from gathering, the shop or another rank).
    pub unsourced: Vec<u16>,
}

impl Plan {
    /// Runs the whole plan takes on average, step after step.
    pub fn total_runs(&self) -> u32 {
        self.steps.iter().map(Step::runs).sum()
    }
}

/// What a quest gives: for each item, its best chance in one of the boxes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestOffer {
    pub id: u16,
    /// The rank the quest is in, when it is known (see `mh3u_core::quest::Quest::rank`). A quest of unknown rank is used only when every
    /// rank is allowed.
    pub rank: Option<Rank>,
    /// (item, percent, quantity, second box).
    pub rewards: Vec<(u16, u8, u8, bool)>,
}

/// Plan the steps for `missing` (item, how many are missing). `sources(item)` lists every monster drop of an item and `hunt_worthy`
/// says whether a monster may be hunted at all (it has a name the player knows). A quest is used when every rank is allowed, or when
/// its own rank is one of the allowed ones.
pub fn plan<'a>(
    missing: &[(u16, u32)],
    sources: impl Fn(u16) -> &'a [Source],
    quests: &[QuestOffer],
    allowed: RankFilter,
    goal: Goal,
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
            Some(c) if cover.better_than(c) => *c = cover,
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
                    runs: expected_runs(need, s.percent, How::Drop(s.method)),
                },
            );
        }
        {
            for q in quests
                .iter()
                .filter(|q| allowed == RankFilter::All || q.rank.is_some_and(|r| allowed.allows(r)))
            {
                for &(reward, percent, quantity, second_box) in q.rewards.iter().filter(|r| r.0 == item) {
                    any = true;
                    offer(
                        Origin::Quest(q.id),
                        Cover {
                            item,
                            missing: need,
                            percent,
                            how: How::Reward { second_box, quantity },
                            runs: expected_runs(need, percent, How::Reward { second_box, quantity }),
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
        let useful_of = |covers: &[Cover]| -> Vec<Cover> { covers.iter().copied().filter(|c| left.contains(&c.item)).collect() };
        let best = match goal {
            // the option covering the most of what is left, then the one that needs the fewest runs, then the first in order
            Goal::FewestSteps => options
                .iter()
                .map(|(origin, covers)| {
                    let useful = useful_of(covers);
                    let runs = runs_of(&useful);
                    (*origin, useful, runs)
                })
                .filter(|(_, useful, _)| !useful.is_empty())
                .max_by_key(|(origin, useful, runs)| (useful.len(), std::cmp::Reverse(*runs), std::cmp::Reverse(*origin))),
            // the cheapest per material: for each option, its quickest k materials cost the runs of the slowest of them
            Goal::FewestRuns => options
                .iter()
                .filter_map(|(origin, covers)| {
                    let mut useful = useful_of(covers);
                    useful.sort_by_key(|c| (c.runs.unwrap_or(0), c.item));
                    (1..=useful.len())
                        .map(|k| {
                            let taken = useful[..k].to_vec();
                            let runs = runs_of(&taken);
                            (*origin, taken, runs)
                        })
                        // cost per material as a fraction (runs / k): compare by cross-multiplying, then more materials, then order
                        .min_by(|a, b| {
                            let (ka, kb) = (a.1.len() as u32, b.1.len() as u32);
                            (a.2 * kb).cmp(&(b.2 * ka)).then(kb.cmp(&ka)).then(a.0.cmp(&b.0))
                        })
                })
                .min_by(|a, b| {
                    let (ka, kb) = (a.1.len() as u32, b.1.len() as u32);
                    (a.2 * kb).cmp(&(b.2 * ka)).then(kb.cmp(&ka)).then(a.0.cmp(&b.0))
                }),
        };
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
        plan(
            missing,
            |item| table.get(&item).map_or(&[], Vec::as_slice),
            &[],
            allowed,
            Goal::FewestSteps,
            |_| true,
        )
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
        // monsters 10 and 20 each cover two items; 20 would need 20 runs for the 5% shiny drop, 10 only 5 for the 20% tail carve
        assert_eq!(steps[0], ((10, Rank::Low), vec![1, 2]));
        // then item 4 (about 5 runs) before item 3 (about 20)
        assert_eq!(steps[1], ((30, Rank::High), vec![4]));
        assert_eq!(steps[2], ((20, Rank::Low), vec![3]));
        assert!(plan.unsourced.is_empty());
        assert_eq!(plan.total_runs(), 5 + 5 + 20);
    }

    #[test]
    fn the_fewest_runs_goal_leaves_a_rare_material_to_a_hunt_that_gives_it_better() {
        // monster 10 gives item 1 (30%, 2 runs for 2) and item 2 only as a 5% shiny (20 runs); monster 20 gives item 2 at 50% (1 run)
        let t: HashMap<u16, Vec<Source>> = HashMap::from([
            (1, vec![src(10, Rank::Low, Method::BodyCarve, 30)]),
            (
                2,
                vec![src(10, Rank::Low, Method::Shiny, 5), src(20, Rank::Low, Method::BodyCarve, 50)],
            ),
        ]);
        let go = |goal| {
            plan(
                &[(1, 2), (2, 1)],
                |i| t.get(&i).map_or(&[], Vec::as_slice),
                &[],
                RankFilter::All,
                goal,
                |_| true,
            )
        };
        // fewest steps: item 2 has a better way (monster 20), but monster 10 covers both items, so it is one step... of 20 runs
        // (the cover for item 2 at monster 10 is kept as the best of that option)
        let steps = go(Goal::FewestSteps);
        assert_eq!(steps.steps.len(), 1);
        assert_eq!(steps.total_runs(), 20);
        // fewest runs: monster 10 for item 1 (3 runs) and monster 20 for item 2 (1 run)
        let runs = go(Goal::FewestRuns);
        assert_eq!(runs.steps.len(), 2);
        assert!(
            runs.total_runs() < steps.total_runs(),
            "{} < {}",
            runs.total_runs(),
            steps.total_runs()
        );
        let items: Vec<(Origin, Vec<u16>)> = runs
            .steps
            .iter()
            .map(|s| (s.origin, s.covers.iter().map(|c| c.item).collect()))
            .collect();
        assert!(items.contains(&(
            Origin::Monster {
                monster: 10,
                rank: Rank::Low
            },
            vec![1]
        )));
        assert!(items.contains(&(
            Origin::Monster {
                monster: 20,
                rank: Rank::Low
            },
            vec![2]
        )));
        // with nothing to split the two goals agree
        let one = |goal| {
            plan(
                &[(1, 2)],
                |i| t.get(&i).map_or(&[], Vec::as_slice),
                &[],
                RankFilter::All,
                goal,
                |_| true,
            )
        };
        assert_eq!(one(Goal::FewestSteps), one(Goal::FewestRuns));
    }

    #[test]
    fn runs_are_the_number_missing_over_what_one_run_gives_on_average() {
        let carve = How::Drop(Method::BodyCarve); // 3 rolls
        assert_eq!(expected_runs(2, 30, carve), Some(3), "0.9 per run: 2 / 0.9 = 2.2");
        assert_eq!(expected_runs(1, 50, carve), Some(1));
        assert_eq!(expected_runs(1, 5, How::Drop(Method::Shiny)), Some(20));
        assert_eq!(expected_runs(1, 10, How::Drop(Method::Capture)), Some(5), "2 rolls: 0.2 per run");
        assert_eq!(expected_runs(4, 0, carve), None, "a run never gives it");
        let always = How::Reward {
            second_box: false,
            quantity: 3,
        };
        assert_eq!(expected_runs(7, 0, always), Some(3), "given every time, 3 each: 7 / 3");
        let second = How::Reward {
            second_box: true,
            quantity: 1,
        };
        assert_eq!(expected_runs(1, 40, second), Some(3), "one roll of the second box at 40%");
        let main = How::Reward {
            second_box: false,
            quantity: 2,
        };
        assert_eq!(expected_runs(6, 25, main), Some(4), "3 rolls x 25% x 2 = 1.5 per run");
    }

    #[test]
    fn a_step_takes_as_long_as_its_slowest_material() {
        let plan = run(&table(), &[(1, 2), (2, 1)], RankFilter::All);
        let step = &plan.steps[0];
        assert_eq!(monster(step).0, 10);
        assert_eq!(step.runs(), 5, "item 1 needs 3 runs, item 2 needs 5");
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
            Goal::FewestSteps,
            |m| m != 10,
        );
        assert!(plan.steps.is_empty());
        assert_eq!(plan.unsourced, [1]);
    }

    fn quest(id: u16, rewards: &[(u16, u8, u8, bool)]) -> QuestOffer {
        QuestOffer {
            id,
            rank: None,
            rewards: rewards.to_vec(),
        }
    }

    fn with_quests(missing: &[(u16, u32)], quests: &[QuestOffer], allowed: RankFilter) -> Plan {
        let t = table();
        plan(
            missing,
            |i| t.get(&i).map_or(&[], Vec::as_slice),
            quests,
            allowed,
            Goal::FewestSteps,
            |_| true,
        )
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
    fn a_quest_of_unknown_rank_is_left_out_when_one_rank_is_chosen() {
        let quests = [quest(500, &[(9, 0, 3, false)])];
        let low = with_quests(&[(9, 1)], &quests, RankFilter::Only(Rank::Low));
        assert_eq!(low.unsourced, [9]);
        assert!(low.steps.is_empty());
    }

    #[test]
    fn a_quest_of_a_known_rank_is_used_for_that_rank_only() {
        let mut low_quest = quest(500, &[(9, 0, 3, false)]);
        low_quest.rank = Some(Rank::Low);
        let quests = [low_quest];
        let low = with_quests(&[(9, 1)], &quests, RankFilter::Only(Rank::Low));
        assert_eq!(low.steps[0].origin, Origin::Quest(500));
        let high = with_quests(&[(9, 1)], &quests, RankFilter::Only(Rank::High));
        assert_eq!(high.unsourced, [9]);
        assert_eq!(with_quests(&[(9, 1)], &quests, RankFilter::All).steps.len(), 1);
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

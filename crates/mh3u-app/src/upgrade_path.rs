//! The cheapest way to get a weapon from what you own.
//!
//! A weapon is got by making it from scratch (if it has a create recipe) or by upgrading one of its parents, which uses the
//! parent up. A parent you own costs nothing to start from; one you do not has to be got the same way. This picks, for the weapon,
//! the route with the least zenny in forging fees.

use mh3u_core::save::ItemStack;
use std::collections::HashMap;

/// What one forging step costs: the fee (`None` when the game's price is not known) and the materials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cost {
    pub zenny: Option<u32>,
    pub materials: Vec<ItemStack>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// Already in the equipment box: the start of the route.
    Owned,
    /// Made from scratch.
    Create,
    /// Upgraded from the step before it.
    Upgrade,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub weapon: u16,
    pub how: How,
    pub cost: Cost,
}

/// A route, first step to last. The last step is the weapon asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub steps: Vec<Step>,
}

impl Path {
    /// The fees of every step whose price is known.
    pub fn zenny(&self) -> u32 {
        self.steps.iter().filter_map(|s| s.cost.zenny).sum()
    }

    /// Steps that cost something but whose price is not known (their fee is left out of `zenny`).
    pub fn unknown_prices(&self) -> usize {
        self.steps.iter().filter(|s| s.how != How::Owned && s.cost.zenny.is_none()).count()
    }

    /// Every material the route needs, added up, in item id order.
    pub fn materials(&self) -> Vec<ItemStack> {
        let mut total: HashMap<u16, u16> = HashMap::new();
        for step in &self.steps {
            for m in &step.cost.materials {
                *total.entry(m.id).or_default() += m.count;
            }
        }
        let mut out: Vec<ItemStack> = total.into_iter().map(|(id, count)| ItemStack { id, count }).collect();
        out.sort_by_key(|m| m.id);
        out
    }

    /// The weapons that have to be got before the last one, first to last, leaving out any you already own.
    pub fn parents_needed(&self) -> Vec<u16> {
        let before_last = self.steps.len().saturating_sub(1);
        self.steps[..before_last]
            .iter()
            .filter(|s| s.how != How::Owned)
            .map(|s| s.weapon)
            .collect()
    }

    /// Better routes sort lower: routes with every price known first (a missing price could hide a dear step), then less zenny,
    /// then fewer steps.
    fn rank(&self) -> (usize, u32, usize) {
        (self.unknown_prices(), self.zenny(), self.steps.len())
    }
}

/// What the search needs to know about the weapons, so that it can be tried on small made-up trees.
pub struct Weapons<'a> {
    pub owned: &'a dyn Fn(u16) -> bool,
    /// The weapons `w` is upgraded from.
    pub parents: &'a dyn Fn(u16) -> Vec<u16>,
    pub create: &'a dyn Fn(u16) -> Option<Cost>,
    /// What upgrading into `w` costs (the parent is not counted).
    pub upgrade: &'a dyn Fn(u16) -> Option<Cost>,
}

/// The cheapest route to `target`, or `None` when there is none (nothing owned or makeable up its line).
pub fn cheapest(target: u16, weapons: &Weapons) -> Option<Path> {
    best(target, weapons, &mut HashMap::new(), &mut Vec::new())
}

fn best(w: u16, weapons: &Weapons, memo: &mut HashMap<u16, Option<Path>>, stack: &mut Vec<u16>) -> Option<Path> {
    if let Some(done) = memo.get(&w) {
        return done.clone();
    }
    if stack.contains(&w) {
        return None; // a loop in the data; this branch goes nowhere
    }
    let found = if (weapons.owned)(w) {
        Some(Path {
            steps: vec![Step {
                weapon: w,
                how: How::Owned,
                cost: Cost {
                    zenny: Some(0),
                    materials: Vec::new(),
                },
            }],
        })
    } else {
        stack.push(w);
        let mut options: Vec<Path> = Vec::new();
        if let Some(cost) = (weapons.create)(w) {
            options.push(Path {
                steps: vec![Step {
                    weapon: w,
                    how: How::Create,
                    cost,
                }],
            });
        }
        if let Some(cost) = (weapons.upgrade)(w) {
            for parent in (weapons.parents)(w) {
                if let Some(mut path) = best(parent, weapons, memo, stack) {
                    path.steps.push(Step {
                        weapon: w,
                        how: How::Upgrade,
                        cost: cost.clone(),
                    });
                    options.push(path);
                }
            }
        }
        stack.pop();
        options.into_iter().min_by_key(Path::rank)
    };
    memo.insert(w, found.clone());
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cost(zenny: u32, item: u16) -> Cost {
        Cost {
            zenny: Some(zenny),
            materials: vec![ItemStack { id: item, count: 2 }],
        }
    }

    /// A line 1 -> 2 -> 3 -> 4: weapon 1 can be made for 100, each upgrade costs 50, 70 and 90; weapon 4 can also be made for 400.
    struct Line {
        owned: Vec<u16>,
        create_4: Option<u32>,
    }

    impl Line {
        fn run(&self, target: u16) -> Option<Path> {
            let owned = |w: u16| self.owned.contains(&w);
            let parents = |w: u16| if w > 1 { vec![w - 1] } else { Vec::new() };
            let create = |w: u16| match w {
                1 => Some(cost(100, 10)),
                4 => self.create_4.map(|z| cost(z, 14)),
                _ => None,
            };
            let upgrade = |w: u16| match w {
                2 => Some(cost(50, 12)),
                3 => Some(cost(70, 13)),
                4 => Some(cost(90, 14)),
                _ => None,
            };
            cheapest(
                target,
                &Weapons {
                    owned: &owned,
                    parents: &parents,
                    create: &create,
                    upgrade: &upgrade,
                },
            )
        }
    }

    fn route(path: &Path) -> Vec<(u16, How)> {
        path.steps.iter().map(|s| (s.weapon, s.how)).collect()
    }

    #[test]
    fn the_line_is_climbed_from_the_first_weapon_that_can_be_made() {
        let path = Line {
            owned: vec![],
            create_4: None,
        }
        .run(4)
        .unwrap();
        assert_eq!(
            route(&path),
            [(1, How::Create), (2, How::Upgrade), (3, How::Upgrade), (4, How::Upgrade)]
        );
        assert_eq!(path.zenny(), 100 + 50 + 70 + 90);
        assert_eq!(
            path.materials().iter().map(|m| (m.id, m.count)).collect::<Vec<_>>(),
            [(10, 2), (12, 2), (13, 2), (14, 2)]
        );
    }

    #[test]
    fn making_it_from_scratch_wins_when_it_is_cheaper() {
        let path = Line {
            owned: vec![],
            create_4: Some(200),
        }
        .run(4)
        .unwrap();
        assert_eq!(route(&path), [(4, How::Create)]);
        let pricey = Line {
            owned: vec![],
            create_4: Some(900),
        }
        .run(4)
        .unwrap();
        assert_eq!(pricey.steps.len(), 4, "the long way is cheaper than 900");
    }

    #[test]
    fn a_weapon_you_own_is_where_the_route_starts_and_costs_nothing() {
        let path = Line {
            owned: vec![2],
            create_4: Some(200),
        }
        .run(4)
        .unwrap();
        assert_eq!(route(&path), [(2, How::Owned), (3, How::Upgrade), (4, How::Upgrade)]);
        assert_eq!(path.zenny(), 70 + 90, "cheaper than the 200 to make it");
        assert_eq!(
            Line {
                owned: vec![4],
                create_4: None
            }
            .run(4)
            .unwrap()
            .steps
            .len(),
            1
        );
    }

    #[test]
    fn nothing_when_no_weapon_on_the_line_can_be_got() {
        let owned = |_: u16| false;
        let parents = |w: u16| if w > 1 { vec![w - 1] } else { Vec::new() };
        let nothing = |_: u16| None;
        let upgrade = |_: u16| Some(cost(10, 1));
        let weapons = Weapons {
            owned: &owned,
            parents: &parents,
            create: &nothing,
            upgrade: &upgrade,
        };
        assert_eq!(cheapest(3, &weapons), None);
    }

    #[test]
    fn of_two_parents_the_cheaper_way_in_is_taken_and_a_loop_does_not_hang() {
        // 3 is upgraded from 1 or 2; 1 costs 500 to make, 2 costs 100; and 4 <-> 5 loop on each other
        let owned = |_: u16| false;
        let parents = |w: u16| match w {
            3 => vec![1, 2],
            4 => vec![5],
            5 => vec![4],
            _ => Vec::new(),
        };
        let create = |w: u16| match w {
            1 => Some(cost(500, 1)),
            2 => Some(cost(100, 2)),
            _ => None,
        };
        let upgrade = |_: u16| Some(cost(10, 9));
        let weapons = Weapons {
            owned: &owned,
            parents: &parents,
            create: &create,
            upgrade: &upgrade,
        };
        let path = cheapest(3, &weapons).unwrap();
        assert_eq!(route(&path), [(2, How::Create), (3, How::Upgrade)]);
        assert_eq!(cheapest(4, &weapons), None, "the loop leads nowhere");
    }

    #[test]
    fn the_parents_needed_are_the_steps_before_the_last_that_you_do_not_own() {
        let none = Line {
            owned: vec![],
            create_4: None,
        }
        .run(4)
        .unwrap();
        assert_eq!(none.parents_needed(), [1, 2, 3]);
        let some = Line {
            owned: vec![2],
            create_4: None,
        }
        .run(4)
        .unwrap();
        assert_eq!(some.parents_needed(), [3], "the owned weapon is the start, not something to get");
        let direct = Line {
            owned: vec![],
            create_4: Some(10),
        }
        .run(4)
        .unwrap();
        assert!(direct.parents_needed().is_empty());
    }

    #[test]
    fn a_route_with_every_price_known_beats_a_cheaper_looking_one_with_a_gap() {
        // weapon 2 can be made for 900, or upgraded from 1 whose price is not known
        let owned = |_: u16| false;
        let parents = |w: u16| if w == 2 { vec![1] } else { Vec::new() };
        let create = |w: u16| match w {
            1 => Some(Cost {
                zenny: None,
                materials: Vec::new(),
            }),
            2 => Some(cost(900, 3)),
            _ => None,
        };
        let upgrade = |w: u16| (w == 2).then(|| cost(10, 4));
        let weapons = Weapons {
            owned: &owned,
            parents: &parents,
            create: &create,
            upgrade: &upgrade,
        };
        let path = cheapest(2, &weapons).unwrap();
        assert_eq!((path.steps.len(), path.zenny()), (1, 900));
    }

    #[test]
    fn an_unknown_price_is_counted_not_hidden() {
        let owned = |_: u16| false;
        let parents = |_: u16| vec![1];
        let create = |w: u16| {
            (w == 1).then(|| Cost {
                zenny: None,
                materials: Vec::new(),
            })
        };
        let upgrade = |_: u16| Some(cost(40, 5));
        let weapons = Weapons {
            owned: &owned,
            parents: &parents,
            create: &create,
            upgrade: &upgrade,
        };
        let path = cheapest(2, &weapons).unwrap();
        assert_eq!((path.zenny(), path.unknown_prices()), (40, 1));
    }
}

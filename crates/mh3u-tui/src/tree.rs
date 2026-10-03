//! The weapon upgrade tree around one weapon, as rows ready to draw.
//!
//! Weapon lines are wide (a great sword has over a hundred weapons), so the tree shows the single line from the first weapon
//! down to the chosen one, with a note where other branches leave it, and then everything the chosen weapon upgrades into.

use std::collections::HashSet;

/// One line of the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: u16,
    /// The branch drawing in front of the name (`│  `, `├─ `, `└─ `).
    pub prefix: String,
    /// The weapon the tree was built around.
    pub selected: bool,
    /// The weapon already appears higher up (it can be reached from more than one parent), so it is not expanded again.
    pub repeat: bool,
    /// For a weapon on the path down to the selected one: how many other weapons it upgrades into.
    pub other_branches: usize,
    /// Other weapons this one can also be upgraded from.
    pub also_from: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    pub rows: Vec<Row>,
    /// Descendants left out because the tree reached its row limit.
    pub omitted: usize,
    /// Index of the selected weapon's row.
    pub selected_row: usize,
}

const MAX_DEPTH: usize = 60;

/// Build the tree around `target`. `parents` and `children` give the ids a weapon is upgraded from and into; the tree holds at
/// most `limit` rows.
pub fn build(target: u16, parents: &dyn Fn(u16) -> Vec<u16>, children: &dyn Fn(u16) -> Vec<u16>, limit: usize) -> Tree {
    // the line of first parents from the target back to where it starts
    let mut chain = vec![target];
    let mut seen: HashSet<u16> = HashSet::from([target]);
    while chain.len() < MAX_DEPTH {
        let Some(parent) = parents(*chain.last().expect("chain starts with the target"))
            .into_iter()
            .find(|p| !seen.contains(p))
        else {
            break;
        };
        seen.insert(parent);
        chain.push(parent);
    }
    chain.reverse();

    let last = chain.len() - 1;
    let mut rows = Vec::new();
    for (i, &id) in chain.iter().enumerate() {
        let also_from = if i == 0 {
            Vec::new()
        } else {
            parents(id).into_iter().filter(|&p| p != chain[i - 1]).collect()
        };
        rows.push(Row {
            id,
            prefix: if i == 0 {
                String::new()
            } else {
                format!("{}└─ ", "   ".repeat(i - 1))
            },
            selected: i == last,
            repeat: false,
            other_branches: if i == last { 0 } else { children(id).len().saturating_sub(1) },
            also_from,
        });
    }

    let mut omitted = 0;
    let base = "   ".repeat(last);
    descend(target, &base, children, parents, &mut seen, limit, &mut rows, &mut omitted);
    Tree {
        rows,
        omitted,
        selected_row: last,
    }
}

#[allow(clippy::too_many_arguments)]
fn descend(
    id: u16,
    prefix: &str,
    children: &dyn Fn(u16) -> Vec<u16>,
    parents: &dyn Fn(u16) -> Vec<u16>,
    seen: &mut HashSet<u16>,
    limit: usize,
    rows: &mut Vec<Row>,
    omitted: &mut usize,
) {
    let kids = children(id);
    for (n, &child) in kids.iter().enumerate() {
        let is_last = n + 1 == kids.len();
        let repeat = seen.contains(&child);
        if rows.len() < limit {
            rows.push(Row {
                id: child,
                prefix: format!("{prefix}{}", if is_last { "└─ " } else { "├─ " }),
                selected: false,
                repeat,
                other_branches: 0,
                also_from: parents(child).into_iter().filter(|&p| p != id).collect(),
            });
        } else {
            *omitted += 1;
        }
        if !repeat {
            seen.insert(child);
            let next = format!("{prefix}{}", if is_last { "   " } else { "│  " });
            descend(child, &next, children, parents, seen, limit, rows, omitted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 1 -> 2 -> 3 -> {4, 5 -> 7}; 1 -> 6; 4 and 6 both lead to 8.
    fn graph() -> (HashMap<u16, Vec<u16>>, HashMap<u16, Vec<u16>>) {
        let children: HashMap<u16, Vec<u16>> = HashMap::from([
            (1, vec![2, 6]),
            (2, vec![3]),
            (3, vec![4, 5]),
            (4, vec![8]),
            (5, vec![7]),
            (6, vec![8]),
        ]);
        let mut parents: HashMap<u16, Vec<u16>> = HashMap::new();
        for (&p, kids) in &children {
            for &k in kids {
                parents.entry(k).or_default().push(p);
            }
        }
        for v in parents.values_mut() {
            v.sort_unstable();
        }
        (parents, children)
    }

    fn tree_for(target: u16, limit: usize) -> Tree {
        let (parents, children) = graph();
        build(
            target,
            &|id| parents.get(&id).cloned().unwrap_or_default(),
            &|id| children.get(&id).cloned().unwrap_or_default(),
            limit,
        )
    }

    fn drawn(t: &Tree) -> Vec<String> {
        t.rows
            .iter()
            .map(|r| {
                format!(
                    "{}{}{}",
                    r.prefix,
                    r.id,
                    if r.selected {
                        " *"
                    } else if r.repeat {
                        " (again)"
                    } else {
                        ""
                    }
                )
            })
            .collect()
    }

    #[test]
    fn shows_the_line_down_to_the_weapon_and_everything_below_it() {
        let t = tree_for(3, 100);
        assert_eq!(
            drawn(&t),
            [
                "1",
                "└─ 2",
                "   └─ 3 *",
                "      ├─ 4",
                "      │  └─ 8",
                "      └─ 5",
                "         └─ 7"
            ]
        );
        assert_eq!(t.selected_row, 2);
        assert_eq!(t.omitted, 0);
    }

    #[test]
    fn notes_where_other_branches_leave_the_line() {
        let t = tree_for(3, 100);
        assert_eq!(t.rows[0].other_branches, 1, "weapon 1 also upgrades into 6");
        assert_eq!(t.rows[1].other_branches, 0);
        assert_eq!(t.rows[2].other_branches, 0, "the selected weapon shows its children instead");
    }

    #[test]
    fn a_weapon_with_two_parents_is_expanded_once_and_lists_the_other_parent() {
        let t = tree_for(1, 100);
        let eights: Vec<_> = t.rows.iter().filter(|r| r.id == 8).collect();
        assert_eq!(eights.len(), 2);
        assert!(!eights[0].repeat && eights[1].repeat);
        assert_eq!(eights[0].also_from, [6]);
        assert_eq!(eights[1].also_from, [4]);
    }

    #[test]
    fn a_weapon_reached_through_its_second_parent_follows_the_first_parent_line() {
        let t = tree_for(8, 100);
        assert_eq!(drawn(&t), ["1", "└─ 2", "   └─ 3", "      └─ 4", "         └─ 8 *"]);
        assert_eq!(t.rows.last().unwrap().also_from, [6]);
    }

    #[test]
    fn a_first_weapon_with_no_children_is_a_single_row() {
        let (parents, children) = graph();
        let t = build(
            7,
            &|id| parents.get(&id).cloned().unwrap_or_default(),
            &|id| children.get(&id).cloned().unwrap_or_default(),
            100,
        );
        assert_eq!(t.selected_row, 4);
        assert_eq!(t.rows.last().unwrap().id, 7);
        let lone = build(99, &|_| Vec::new(), &|_| Vec::new(), 10);
        assert_eq!(lone.rows.len(), 1);
        assert!(lone.rows[0].selected);
    }

    #[test]
    fn rows_past_the_limit_are_counted_not_drawn() {
        let t = tree_for(1, 4);
        assert_eq!(t.rows.len(), 4);
        assert_eq!(t.omitted, 5, "eight rows below weapon 1 in all, three fit after its own row");
    }

    #[test]
    fn a_cycle_in_the_data_does_not_loop() {
        let t = build(
            1,
            &|id| if id == 1 { vec![2] } else { vec![1] },
            &|id| if id == 1 { vec![2] } else { vec![1] },
            50,
        );
        assert!(t.rows.len() < 10, "{:?}", drawn(&t));
    }
}

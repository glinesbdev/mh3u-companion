//! The Worn tab: what the worn gear adds up to, with a skill to look into (where its points come from, the tiers above it).

use super::*;
use crate::worn::{self as sums, SkillTotal, Summary};

/// Stands for "the jewels in the armor" where the totals take a piece's equipment kind (0 is no real kind).
pub const JEWELS: u8 = 0;
/// Equipment kind of a talisman.
const CHARM: u8 = 6;

/// The highlighted skill of the Worn tab.
#[derive(Default)]
pub struct WornTab {
    pub skills: ListState,
}

/// One thing that adds points to a skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSource {
    /// Where it is worn and what it is: `Head: Arzuros Helm`, `Head jewel: Attack Jewel 1`.
    pub label: String,
    pub points: i32,
}

/// One tier of a skill: the points where an effect starts (negative for a penalty) and the effect's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TierRow {
    pub points: i8,
    pub effect: u16,
    pub reached: bool,
}

/// What lies behind one line of the totals.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillDetail {
    pub total: i32,
    pub sources: Vec<SkillSource>,
    /// Positive tiers from the lowest, then the penalties from the mildest.
    pub tiers: Vec<TierRow>,
    /// The next tier above the total: (points where it starts, effect, points still missing).
    pub next: Option<(i8, u16, i32)>,
}

impl App {
    /// What the worn armor and charm (with their jewels) add up to.
    pub fn worn_summary(&self) -> Summary {
        let armor = self.worn_armor();
        let talisman = self.worn_talisman();
        let stats: Vec<(u8, &mh3u_core::armor::ArmorStats)> = armor
            .iter()
            .filter_map(|&(kind, e)| self.game.armor_stats(kind, e.id).map(|a| (kind, a)))
            .collect();
        let charm = talisman.map(|t| {
            let mut skills = t.talisman_skills();
            skills.extend(self.game.decoration_points(&t.talisman_decorations()));
            mh3u_core::armor::ArmorStats::talisman(skills, t.talisman_slots())
        });
        // jewels in armor are counted apart from the piece, so Torso Up does not double them
        let armor_jewels = mh3u_core::armor::ArmorStats::talisman(
            self.game
                .decoration_points(&armor.iter().flat_map(|(_, e)| e.decorations()).collect::<Vec<_>>()),
            0,
        );
        let mut counted = stats;
        counted.push((JEWELS, &armor_jewels));
        if let Some(c) = &charm {
            counted.push((CHARM, c));
        }
        sums::summarize(&counted)
    }

    /// The highlighted row of the totals, kept inside the list.
    pub fn worn_selected(&self, summary: &Summary) -> Option<usize> {
        let last = summary.skills.len().checked_sub(1)?;
        Some(self.worn.skills.selected().unwrap_or(0).min(last))
    }

    /// The skill highlighted on the Worn tab.
    pub fn worn_skill<'a>(&self, summary: &'a Summary) -> Option<&'a SkillTotal> {
        summary.skills.get(self.worn_selected(summary)?)
    }

    pub(super) fn worn_move(&mut self, step: isize) {
        let summary = self.worn_summary();
        let len = summary.skills.len();
        let at = self.worn_selected(&summary);
        self.worn.skills.select((len > 0).then(|| stepped(at, step, len)));
    }

    /// Where a skill's points come from, piece by piece and jewel by jewel, and its tiers.
    pub fn worn_skill_detail(&self, skill: u8) -> SkillDetail {
        let summary = self.worn_summary();
        let mut sources: Vec<SkillSource> = Vec::new();
        let kind_label = |kind: u8| self.game.equipment_kind_label(kind).unwrap_or("?");
        let jewel_points = |code: u16| -> i32 {
            let Some(d) = self.game.decoration(code) else { return 0 };
            let mut p = 0;
            if d.skill == skill {
                p += i32::from(d.points);
            }
            if let Some((s, pts)) = d.penalty
                && s == skill
            {
                p += i32::from(pts);
            }
            p
        };
        let jewel_name = |code: u16| {
            self.game
                .decoration(code)
                .and_then(|d| self.game.item_name(d.item))
                .unwrap_or("?")
                .to_string()
        };
        for (kind, e) in self.worn_armor() {
            if let Some(a) = self.game.armor_stats(kind, e.id) {
                let own: i32 = a.skills.iter().filter(|&&(s, _)| s == skill).map(|&(_, p)| i32::from(p)).sum();
                let doubled = summary.torso_doubled && kind == 1 && skill != sums::TORSO_UP && own != 0;
                if own != 0 {
                    sources.push(SkillSource {
                        label: format!(
                            "{}: {}{}",
                            kind_label(kind),
                            self.game.equipment_name(kind, e.id).unwrap_or("?"),
                            if doubled { " (doubled by Torso Up)" } else { "" }
                        ),
                        points: if doubled { 2 * own } else { own },
                    });
                }
            }
            for code in e.decorations() {
                let p = jewel_points(code);
                if p != 0 {
                    sources.push(SkillSource {
                        label: format!("{} jewel: {}", kind_label(kind), jewel_name(code)),
                        points: p,
                    });
                }
            }
        }
        if let Some(t) = self.worn_talisman() {
            let own: i32 = t
                .talisman_skills()
                .iter()
                .filter(|&&(s, _)| s == skill)
                .map(|&(_, p)| i32::from(p))
                .sum();
            if own != 0 {
                sources.push(SkillSource {
                    label: format!("Charm: {}", self.game.equipment_name(CHARM, t.id).unwrap_or("Talisman")),
                    points: own,
                });
            }
            for code in t.talisman_decorations() {
                let p = jewel_points(code);
                if p != 0 {
                    sources.push(SkillSource {
                        label: format!("Charm jewel: {}", jewel_name(code)),
                        points: p,
                    });
                }
            }
        }
        let total = summary.skills.iter().find(|t| t.id == skill).map_or(0, |t| t.points);
        let mut positive: Vec<(i8, u16)> = mh3u_core::skilltiers::tiers(skill).filter(|t| t.0 > 0).collect();
        positive.sort_unstable();
        let mut negative: Vec<(i8, u16)> = mh3u_core::skilltiers::tiers(skill).filter(|t| t.0 < 0).collect();
        negative.sort_unstable_by_key(|t| std::cmp::Reverse(t.0));
        let tiers: Vec<TierRow> = positive
            .iter()
            .chain(&negative)
            .map(|&(points, effect)| TierRow {
                points,
                effect,
                reached: if points > 0 {
                    total >= i32::from(points)
                } else {
                    total <= i32::from(points)
                },
            })
            .collect();
        let next = positive
            .iter()
            .find(|&&(p, _)| i32::from(p) > total)
            .map(|&(p, e)| (p, e, i32::from(p) - total));
        SkillDetail {
            total,
            sources,
            tiers,
            next,
        }
    }
}

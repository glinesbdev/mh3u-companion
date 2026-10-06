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
    /// Only the skills that are a few points short of their next tier.
    pub near: bool,
    /// The name of the template the worn pieces are compared with, when comparing.
    pub against: Option<String>,
    pub compare: ListState,
    /// The list of templates to choose from, while open.
    pub pick: Option<ListState>,
}

/// How many points short of the next tier a skill is to count as "near" (key `m`).
pub const NEAR: i32 = 5;

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

/// A jewel the hunter owns that adds to a skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedJewel {
    pub name: String,
    pub points: i32,
    pub slots: u8,
    pub owned: u32,
    /// What it costs in another skill, as text (`-1 Whim`), if anything.
    pub costs: Option<String>,
}

/// Where to put one of them: a worn piece or the charm with free gem slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub place: String,
    pub jewel: String,
    pub points: i32,
}

/// What it would take to reach a skill's next tier with the jewels the hunter owns and the gem slots that are free.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Missing {
    /// (where, free gem slots) for every worn piece and the charm with room.
    pub free: Vec<(String, u8)>,
    pub jewels: Vec<OwnedJewel>,
    /// One way to fill the free slots, best use of a slot first, until the tier is reached or nothing more fits.
    pub fills: Vec<Placement>,
    pub gained: i32,
    pub reaches: bool,
}

/// One skill in the comparison of the worn pieces with a template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompareRow {
    pub skill: u8,
    pub worn: i32,
    pub template: i32,
}

impl CompareRow {
    pub fn diff(&self) -> i32 {
        self.template - self.worn
    }
}

/// One slot of the comparison: what is worn there and what the template has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceDiff {
    pub slot: &'static str,
    pub worn: Option<String>,
    pub template: Option<String>,
}

impl PieceDiff {
    pub fn same(&self) -> bool {
        self.worn == self.template
    }
}

/// The worn armor and charm against a saved template, pieces only: jewels can be moved, so they count on neither side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Versus {
    pub template: String,
    pub rows: Vec<CompareRow>,
    pub pieces: Vec<PieceDiff>,
}

impl App {
    /// What the worn armor and charm (with their jewels) add up to.
    pub fn worn_summary(&self) -> Summary {
        self.worn_summary_with(true)
    }

    /// The same, with or without what is socketed into the armor and the charm.
    fn worn_summary_with(&self, jewels: bool) -> Summary {
        let armor = self.worn_armor();
        let talisman = self.worn_talisman();
        let stats: Vec<(u8, &mh3u_core::armor::ArmorStats)> = armor
            .iter()
            .filter_map(|&(kind, e)| self.game.armor_stats(kind, e.id).map(|a| (kind, a)))
            .collect();
        let charm = talisman.map(|t| {
            let mut skills = t.talisman_skills();
            if jewels {
                skills.extend(self.game.decoration_points(&t.talisman_decorations()));
            }
            mh3u_core::armor::ArmorStats::talisman(skills, t.talisman_slots())
        });
        // jewels in armor are counted apart from the piece, so Torso Up does not double them
        let armor_jewels = mh3u_core::armor::ArmorStats::talisman(
            if jewels {
                self.game
                    .decoration_points(&armor.iter().flat_map(|(_, e)| e.decorations()).collect::<Vec<_>>())
            } else {
                Vec::new()
            },
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
        let last = self.worn_rows(summary).len().checked_sub(1)?;
        Some(self.worn.skills.selected().unwrap_or(0).min(last))
    }

    /// The skill highlighted on the Worn tab.
    pub fn worn_skill<'a>(&self, summary: &'a Summary) -> Option<&'a SkillTotal> {
        self.worn_rows(summary).get(self.worn_selected(summary)?).copied()
    }

    /// Keys of the Worn tab (the ones for moving are the shared ones).
    pub(super) fn worn_key(&mut self, code: Key) -> bool {
        match code {
            Key::Char('m') => {
                self.worn.near = !self.worn.near;
                self.worn.skills.select(Some(0));
                self.status = if self.worn.near {
                    format!("only skills within {NEAR} points of their next tier (m shows all)")
                } else {
                    String::new()
                };
            }
            Key::Char('v') => {
                if self.worn.against.take().is_some() {
                    self.status = String::new();
                } else if self.builds.templates.is_empty() {
                    self.status = "no templates yet: save one on the Builds tab".to_string();
                } else {
                    self.worn.pick = Some(ListState::default().with_selected(Some(0)));
                }
            }
            _ => return false,
        }
        true
    }

    pub(super) fn worn_move(&mut self, step: isize) {
        if let Some(v) = self.worn_versus() {
            let at = self.worn.compare.selected();
            self.worn
                .compare
                .select((!v.rows.is_empty()).then(|| stepped(at, step, v.rows.len())));
            return;
        }
        let summary = self.worn_summary();
        let len = self.worn_rows(&summary).len();
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

    /// The skills of the Worn tab as listed: all of them, or with `near` only those a few points short of the next tier.
    pub fn worn_rows<'a>(&self, summary: &'a Summary) -> Vec<&'a SkillTotal> {
        summary
            .skills
            .iter()
            .filter(|t| {
                !self.worn.near
                    || mh3u_core::skilltiers::tiers(t.id)
                        .filter(|x| x.0 > 0)
                        .map(|x| i32::from(x.0) - t.points)
                        .filter(|&gap| gap > 0)
                        .min()
                        .is_some_and(|gap| gap <= NEAR)
            })
            .collect()
    }

    /// The gem slots still free on the worn armor and the charm.
    fn worn_free_slots(&self) -> Vec<(String, u8)> {
        let used = |codes: &[u16]| -> u8 { codes.iter().filter_map(|&c| self.game.decoration(c)).map(|d| d.slots).sum() };
        let mut out = Vec::new();
        for (kind, e) in self.worn_armor() {
            if let Some(a) = self.game.armor_stats(kind, e.id) {
                let free = a.slots.saturating_sub(used(&e.decorations()));
                if free > 0 {
                    out.push((self.game.equipment_kind_label(kind).unwrap_or("?").to_string(), free));
                }
            }
        }
        if let Some(t) = self.worn_talisman() {
            let free = t.talisman_slots().saturating_sub(used(&t.talisman_decorations()));
            if free > 0 {
                out.push(("Charm".to_string(), free));
            }
        }
        out
    }

    /// Whether the jewels the hunter owns and the free gem slots can close the gap (`gap` points) to a skill's next tier, and one way
    /// to do it. A jewel needs as many free slots on one piece as it has slots; the ones that give the most per slot go in first.
    pub fn worn_missing(&self, skill: u8, gap: i32) -> Missing {
        let mut free = self.worn_free_slots();
        let mut jewels: Vec<OwnedJewel> = Vec::new();
        for d in self.game.decoration_table() {
            if d.skill != skill || d.points <= 0 {
                continue;
            }
            let owned = self.save.item_count(d.item);
            if owned == 0 {
                continue;
            }
            jewels.push(OwnedJewel {
                name: self.game.item_name(d.item).unwrap_or("?").to_string(),
                points: i32::from(d.points),
                slots: d.slots,
                owned,
                costs: d.penalty.map(|(s, p)| format!("{p:+} {}", self.game.skill_name(s).unwrap_or("?"))),
            });
        }
        // the most points for each slot first, then the most points
        jewels.sort_by_key(|j| (std::cmp::Reverse(j.points * 12 / i32::from(j.slots)), std::cmp::Reverse(j.points)));
        let mut summary = Missing {
            free: free.clone(),
            jewels: jewels.clone(),
            ..Missing::default()
        };
        'fill: for j in &jewels {
            for _ in 0..j.owned.min(18) {
                if summary.gained >= gap {
                    break 'fill;
                }
                // the piece with the least room that still fits it
                let Some(at) = (0..free.len()).filter(|&i| free[i].1 >= j.slots).min_by_key(|&i| free[i].1) else {
                    break;
                };
                free[at].1 -= j.slots;
                summary.gained += j.points;
                summary.fills.push(Placement {
                    place: free[at].0.clone(),
                    jewel: j.name.clone(),
                    points: j.points,
                });
            }
        }
        summary.reaches = summary.gained >= gap;
        summary
    }

    /// The template the worn pieces are compared with, if one is chosen and still there.
    pub fn worn_against(&self) -> Option<&crate::templates::Template> {
        let name = self.worn.against.as_ref()?;
        self.builds.templates.iter().find(|t| &t.name == name)
    }

    /// The worn armor and charm (without jewels) against the chosen template: the skills that differ first, and each slot.
    pub fn worn_versus(&self) -> Option<Versus> {
        let template = self.worn_against()?;
        let worn = self.worn_summary_with(false);
        let theirs: Vec<(u8, mh3u_core::armor::ArmorStats)> = template
            .pieces
            .iter()
            .filter(|p| p.kind != 0 && !(7..=19).contains(&p.kind))
            .filter_map(|p| self.piece_stats(p).map(|a| (p.kind, a)))
            .collect();
        let refs: Vec<(u8, &mh3u_core::armor::ArmorStats)> = theirs.iter().map(|(k, a)| (*k, a)).collect();
        let template_sum = sums::summarize(&refs);
        let points = |s: &Summary, id: u8| s.skills.iter().find(|t| t.id == id).map_or(0, |t| t.points);
        let mut ids: Vec<u8> = worn.skills.iter().chain(&template_sum.skills).map(|t| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        let mut rows: Vec<CompareRow> = ids
            .into_iter()
            .map(|id| CompareRow {
                skill: id,
                worn: points(&worn, id),
                template: points(&template_sum, id),
            })
            .collect();
        // the biggest changes first, then what is the same by the points it has
        rows.sort_by_key(|r| {
            (
                std::cmp::Reverse(r.diff().abs()),
                std::cmp::Reverse(r.worn.max(r.template)),
                r.skill,
            )
        });
        let worn_armor = self.worn_armor();
        let mut pieces = Vec::new();
        for slot in crate::templates::Slot::ALL {
            let Some(kind) = slot.kind() else { continue };
            if (7..=19).contains(&kind) {
                continue;
            }
            let wearing = if kind == CHARM {
                self.worn_talisman().map(|t| self.charm_label(t.talisman_skills()))
            } else {
                worn_armor
                    .iter()
                    .find(|&&(k, _)| k == kind)
                    .map(|&(_, e)| self.game.equipment_name(kind, e.id).unwrap_or("?").to_string())
            };
            let planned = template.pieces.iter().find(|p| p.kind == kind).map(|p| {
                if kind == CHARM {
                    self.charm_label(p.skills.clone())
                } else {
                    self.game.equipment_name(kind, p.id).unwrap_or("?").to_string()
                }
            });
            pieces.push(PieceDiff {
                slot: slot.label(),
                worn: wearing,
                template: planned,
            });
        }
        Some(Versus {
            template: template.name.clone(),
            rows,
            pieces,
        })
    }

    /// A charm as the comparison names it: its skills, since a talisman is told apart by them.
    fn charm_label(&self, skills: Vec<(u8, i8)>) -> String {
        let names: Vec<String> = skills
            .iter()
            .map(|&(id, pts)| format!("{} {pts:+}", self.game.skill_name(id).unwrap_or("?")))
            .collect();
        if names.is_empty() {
            "Talisman".to_string()
        } else {
            format!("Talisman ({})", names.join(", "))
        }
    }

    pub fn worn_pick_open(&self) -> bool {
        self.worn.pick.is_some()
    }

    pub(super) fn worn_pick_key(&mut self, code: Key) {
        let len = self.builds.templates.len();
        let Some(state) = &mut self.worn.pick else { return };
        let at = state.selected().unwrap_or(0);
        match code {
            Key::Esc | Key::Char('q') => self.worn.pick = None,
            Key::Down | Key::Char('j') => state.select(Some((at + 1).min(len.saturating_sub(1)))),
            Key::Up | Key::Char('k') => state.select(Some(at.saturating_sub(1))),
            Key::Enter => {
                self.worn.against = self.builds.templates.get(at).map(|t| t.name.clone());
                self.worn.pick = None;
                self.worn.compare.select(Some(0));
            }
            _ => {}
        }
    }
}

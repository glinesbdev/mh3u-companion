//! The Skills tab: pick a skill and see every armor piece that has it.

use super::*;

/// One armor piece with a skill: its kind and id, and the skill points it gives (negative for a penalty).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WithSkill {
    pub kind: u8,
    pub id: u16,
    pub points: i8,
}

/// The Skills tab: the skills that some armor has, the search over them, and the pieces of the highlighted one.
pub struct SkillsTab {
    /// Skill ids that at least one piece has, in the game's order.
    skills: Vec<u8>,
    /// Armor with each skill, most points first (then head, body, arms, waist, legs, then id).
    pieces: HashMap<u8, Vec<WithSkill>>,
    /// The skills matching the search, best match first.
    pub rows: Vec<u8>,
    pub state: ListState,
    pub search: String,
    /// Only pieces you own or the blacksmith offers.
    pub reachable_only: bool,
    /// First visible line of the pieces; the drawing code keeps it inside the text.
    pub scroll: u16,
}

impl SkillsTab {
    pub(super) fn new(game: &GameData) -> SkillsTab {
        let mut pieces: HashMap<u8, Vec<WithSkill>> = HashMap::new();
        for kind in [5u8, 1, 2, 3, 4] {
            for id in game.piece_ids(kind) {
                let Some(stats) = game.armor_stats(kind, id) else { continue };
                for &(skill, points) in &stats.skills {
                    pieces.entry(skill).or_default().push(WithSkill { kind, id, points });
                }
            }
        }
        for list in pieces.values_mut() {
            list.sort_by_key(|p| (std::cmp::Reverse(p.points), kind_rank(p.kind), p.id));
        }
        let skills = game.skill_ids().filter(|id| pieces.contains_key(id)).collect();
        SkillsTab {
            skills,
            pieces,
            rows: Vec::new(),
            state: ListState::default().with_selected(Some(0)),
            search: String::new(),
            reachable_only: false,
            scroll: 0,
        }
    }

    /// The armor that has a skill.
    pub fn pieces_with(&self, skill: u8) -> &[WithSkill] {
        self.pieces.get(&skill).map_or(&[], Vec::as_slice)
    }

    pub fn selected(&self) -> Option<u8> {
        self.state.selected().and_then(|i| self.rows.get(i)).copied()
    }
}

impl App {
    /// Work out the rows again: the skills matching the search.
    pub(super) fn refresh_skills(&mut self) {
        let words: Vec<String> = self.skills.search.split_whitespace().map(str::to_lowercase).collect();
        let mut rows: Vec<(u32, u8)> = Vec::new();
        for &id in &self.skills.skills {
            let name = self.game.skill_name(id).unwrap_or("").to_lowercase();
            let mut total = 0;
            let mut matched = true;
            for w in &words {
                match search::score(w, &name) {
                    Some(s) => total += s,
                    None => {
                        matched = false;
                        break;
                    }
                }
            }
            if matched {
                rows.push((total, id));
            }
        }
        if !words.is_empty() {
            rows.sort_by_key(|&(score, id)| (std::cmp::Reverse(score), id));
        }
        self.skills.rows = rows.into_iter().map(|(_, id)| id).collect();
        let len = self.skills.rows.len();
        let at = self.skills.state.selected().unwrap_or(0).min(len.saturating_sub(1));
        self.skills.state.select((len > 0).then_some(at));
    }

    pub(super) fn skills_tab_key(&mut self, code: Key) -> bool {
        match code {
            Key::Char('/') => self.searching = true,
            Key::Char('o') => self.skills.reachable_only = !self.skills.reachable_only,
            Key::Char('x') => self.clear_search(),
            Key::PageDown => self.skills.scroll = self.skills.scroll.saturating_add(10),
            Key::PageUp => self.skills.scroll = self.skills.scroll.saturating_sub(10),
            // build for this skill
            Key::Enter => {
                let Some(skill) = self.skills.selected() else { return true };
                if !self.builds.settings.targets.iter().any(|t| t.skill == skill) {
                    self.builds.settings.targets.push(Target {
                        skill,
                        points: crate::worn::ACTIVE_AT,
                    });
                    self.builds_changed();
                }
                self.builds
                    .target_state
                    .select(self.builds.settings.targets.iter().position(|t| t.skill == skill));
                self.builds.focus = BuildFocus::Skills;
                self.tab = Tab::Builds;
                self.status = format!("{} added to the Builds tab", self.game.skill_name(skill).unwrap_or("?"));
            }
            _ => return false,
        }
        true
    }
}

//! The Builds tab: wanted skills, the sets found, build templates, and the popups that edit them.

use super::*;

/// Everything the Builds tab keeps: the wanted skills and options (saved per hunter), the pieces the search may use, the sets it
/// found, the build templates, and the popups that edit them.
pub struct BuildManager {
    pub settings: Settings,
    pub pool: Vec<Candidate>,
    pub results: Vec<Found>,
    pub target_state: ListState,
    pub result_state: ListState,
    /// Which of the three lists the keys move.
    pub focus: BuildFocus,
    /// Saved sets, per hunter.
    pub templates: Vec<Template>,
    pub template_state: ListState,
    /// The highlighted slot of the highlighted template (an index into `templates::SLOTS`).
    pub template_slot: usize,
    pub name_prompt: Option<NamePrompt>,
    pub piece_picker: Option<PiecePicker>,
    pub skill_picker: Option<SkillPicker>,
    /// The save changed since the sets were searched; they are searched again when the Builds tab is next shown.
    pub(super) stale: bool,
}

impl BuildManager {
    /// Start from the saved settings and templates (the text of their files, if there are any).
    pub(super) fn load(settings: Option<String>, templates: Option<String>) -> BuildManager {
        BuildManager {
            settings: settings.map(|t| Settings::parse(&t)).unwrap_or_default(),
            pool: Vec::new(),
            results: Vec::new(),
            target_state: ListState::default().with_selected(Some(0)),
            result_state: ListState::default().with_selected(Some(0)),
            focus: BuildFocus::Skills,
            templates: templates.map(|t| templates::parse(&t)).unwrap_or_default(),
            template_state: ListState::default().with_selected(Some(0)),
            template_slot: 0,
            name_prompt: None,
            piece_picker: None,
            skill_picker: None,
            stale: true,
        }
    }
}

/// Which list the keys move on the Builds tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFocus {
    Skills,
    Sets,
    Templates,
}

/// What the name popup does with the name once it is entered.
#[derive(Clone, Copy)]
pub enum NameAction {
    /// Save the found set with this index as a template.
    SaveSet(usize),
    /// Save what is worn now as a template.
    FromWorn,
    /// Rename the template with this index.
    Rename(usize),
}

pub struct NamePrompt {
    pub text: String,
    pub action: NameAction,
}

/// How near a piece is to being worn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Owned,
    OnOffer,
    Unavailable,
}

/// One entry of the piece popup: a piece to put in a template slot, or `None` to empty the slot.
pub struct Choice {
    pub piece: Option<templates::Piece>,
    pub name: String,
    /// Whether the hunter has it, can get it from the blacksmith, or has to wait.
    pub availability: Availability,
    /// Skills, for the talisman (and a hint for armor).
    pub detail: String,
}

/// The popup for choosing the piece of one template slot.
pub struct PiecePicker {
    pub kind: u8,
    pub text: String,
    pub state: ListState,
    pub choices: Vec<Choice>,
}

/// The popup for choosing a skill to add to a build: what has been typed and the highlighted match.
#[derive(Default)]
pub struct SkillPicker {
    pub text: String,
    pub state: ListState,
}

/// The row to select after moving `step` rows from `current` in a list of `len` rows, staying inside the list.
/// `isize::MIN` and `isize::MAX` therefore go to the top and the bottom.
/// Points to ask for when a skill is first added: the point where its first effect starts.
pub(super) fn worn_active_points() -> i32 {
    crate::worn::ACTIVE_AT
}

impl App {
    /// Every armor piece and talisman the build search may use: what the equipment box holds and, as the pool setting says, what the
    /// blacksmith is offering or every piece in the game.
    pub(super) fn build_pool(&self) -> Vec<Candidate> {
        let usable = |stats: &mh3u_core::armor::ArmorStats| builds::usable(stats, self.builds.settings.gender, self.builds.settings.class);
        let mut pool: Vec<Candidate> = Vec::new();
        let mut seen: HashSet<(u8, u16)> = HashSet::new();
        for e in &self.save.equipment_box {
            if (1..=5).contains(&e.kind)
                && self.game.piece_name(e.kind, e.id).is_some()
                && let Some(stats) = self.game.armor_stats(e.kind, e.id).filter(|s| usable(s))
                && seen.insert((e.kind, e.id))
            {
                pool.push(Candidate {
                    kind: e.kind,
                    id: e.id,
                    owned: true,
                    stats: stats.clone(),
                });
            }
            if e.kind == 6 && self.builds.settings.use_talisman && !e.talisman_skills().is_empty() {
                pool.push(Candidate {
                    kind: 6,
                    id: e.id,
                    owned: true,
                    stats: mh3u_core::armor::ArmorStats::talisman(e.talisman_skills()),
                });
            }
        }
        if self.builds.settings.pool != builds::Pool::Owned {
            for kind in 1..=5u8 {
                for id in self.game.piece_ids(kind) {
                    if let Some(stats) = self.game.armor_stats(kind, id).filter(|s| usable(s))
                        && !seen.contains(&(kind, id))
                        && (self.builds.settings.pool == builds::Pool::All || self.at_blacksmith(kind, id))
                    {
                        pool.push(Candidate {
                            kind,
                            id,
                            owned: false,
                            stats: stats.clone(),
                        });
                    }
                }
            }
        }
        pool
    }

    /// Search again for sets that reach the wanted skills.
    pub fn refresh_builds(&mut self) {
        const SHOWN: usize = 300;
        self.builds.stale = false;
        self.builds.pool = self.build_pool();
        self.builds.results = builds::search(&self.builds.pool, &self.builds.settings.targets, SHOWN);
        let len = self.builds.results.len();
        let at = self.builds.result_state.selected().unwrap_or(0).min(len.saturating_sub(1));
        self.builds.result_state.select((len > 0).then_some(at));
        let wanted = self.builds.settings.targets.len();
        let at = self.builds.target_state.selected().unwrap_or(0).min(wanted.saturating_sub(1));
        self.builds.target_state.select((wanted > 0).then_some(at));
    }

    pub(super) fn save_builds(&mut self) {
        let Some(path) = self.files.as_ref().map(|f| &f.builds) else {
            return;
        };
        if let Err(message) = crate::files::save(path, &self.builds.settings.format(), "builds") {
            self.status = message;
        }
    }

    /// Skills whose name matches what was typed in the picker, best match first (all skills when nothing was typed).
    pub fn skill_matches(&self, typed: &str) -> Vec<u8> {
        let words: Vec<String> = typed.split_whitespace().map(str::to_lowercase).collect();
        let mut scored: Vec<(u32, u8)> = self
            .game
            .skill_ids()
            .filter_map(|id| {
                let name = self.game.skill_name(id)?.to_lowercase();
                let mut total = 0;
                for w in &words {
                    total += search::score(w, &name)?;
                }
                Some((total, id))
            })
            .collect();
        scored.sort_by_key(|&(score, id)| (std::cmp::Reverse(score), id));
        scored.into_iter().map(|(_, id)| id).collect()
    }

    pub(super) fn builds_changed(&mut self) {
        self.save_builds();
        self.refresh_builds();
    }

    /// Keys while the skill picker is open.
    pub(super) fn picker_key(&mut self, code: KeyCode) {
        let Some(picker) = self.builds.skill_picker.as_mut() else { return };
        match code {
            KeyCode::Esc => self.builds.skill_picker = None,
            KeyCode::Backspace => {
                picker.text.pop();
                picker.state.select(Some(0));
            }
            KeyCode::Char(c) => {
                picker.text.push(c);
                picker.state.select(Some(0));
            }
            KeyCode::Down => picker.state.select(Some(picker.state.selected().map_or(0, |i| i + 1))),
            KeyCode::Up => picker
                .state
                .select(Some(picker.state.selected().map_or(0, |i| i.saturating_sub(1)))),
            KeyCode::Enter => {
                let text = picker.text.clone();
                let at = picker.state.selected().unwrap_or(0);
                let matches = self.skill_matches(&text);
                if let Some(&skill) = matches.get(at.min(matches.len().saturating_sub(1))) {
                    self.builds.skill_picker = None;
                    if let Some(t) = self.builds.settings.targets.iter().position(|t| t.skill == skill) {
                        self.builds.target_state.select(Some(t));
                    } else {
                        self.builds.settings.targets.push(Target {
                            skill,
                            points: worn_active_points(),
                        });
                        self.builds.target_state.select(Some(self.builds.settings.targets.len() - 1));
                    }
                    self.builds.focus = BuildFocus::Skills;
                    self.builds_changed();
                }
            }
            _ => {}
        }
    }

    /// Keys on the Builds tab; returns whether the key was used.
    pub(super) fn builds_key(&mut self, code: KeyCode) -> bool {
        use mh3u_core::armor::{ArmorClass, Gender};
        match code {
            KeyCode::Char('a') => {
                self.builds.skill_picker = Some(SkillPicker {
                    text: String::new(),
                    state: ListState::default().with_selected(Some(0)),
                });
            }
            KeyCode::Char('f') => {
                self.builds.focus = match self.builds.focus {
                    BuildFocus::Skills => BuildFocus::Sets,
                    BuildFocus::Sets => BuildFocus::Templates,
                    BuildFocus::Templates => BuildFocus::Skills,
                };
            }
            KeyCode::Char('o') => {
                self.builds.settings.pool = self.builds.settings.pool.next();
                self.builds_changed();
            }
            KeyCode::Char('m') => {
                self.builds.settings.use_talisman = !self.builds.settings.use_talisman;
                self.builds_changed();
            }
            KeyCode::Char('e') => {
                self.builds.settings.gender = match self.builds.settings.gender {
                    None => Some(Gender::Male),
                    Some(Gender::Male) => Some(Gender::Female),
                    _ => None,
                };
                self.builds_changed();
            }
            KeyCode::Char('c') => {
                self.builds.settings.class = match self.builds.settings.class {
                    None => Some(ArmorClass::Blademaster),
                    Some(ArmorClass::Blademaster) => Some(ArmorClass::Gunner),
                    _ => None,
                };
                self.builds_changed();
            }
            _ => {
                return match self.builds.focus {
                    BuildFocus::Skills => self.skills_key(code),
                    BuildFocus::Sets => self.sets_key(code),
                    BuildFocus::Templates => self.templates_key(code),
                };
            }
        }
        true
    }

    pub(super) fn skills_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('+' | '=') | KeyCode::Char('-') => {
                let step = if matches!(code, KeyCode::Char('-')) { -1 } else { 1 };
                if let Some(t) = self
                    .builds
                    .target_state
                    .selected()
                    .and_then(|i| self.builds.settings.targets.get_mut(i))
                {
                    t.points = (t.points + step).clamp(1, 30);
                    self.builds_changed();
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some(i) = self
                    .builds
                    .target_state
                    .selected()
                    .filter(|&i| i < self.builds.settings.targets.len())
                {
                    self.builds.settings.targets.remove(i);
                    self.builds_changed();
                }
            }
            _ => return false,
        }
        true
    }

    pub(super) fn sets_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('w') => self.wish_build(),
            KeyCode::Char('s') | KeyCode::Enter => {
                if let Some(i) = self.builds.result_state.selected().filter(|&i| i < self.builds.results.len()) {
                    self.builds.name_prompt = Some(NamePrompt {
                        text: templates::next_name(&self.builds.templates),
                        action: NameAction::SaveSet(i),
                    });
                }
            }
            _ => return false,
        }
        true
    }

    pub(super) fn templates_key(&mut self, code: KeyCode) -> bool {
        let selected = self.builds.template_state.selected().filter(|&i| i < self.builds.templates.len());
        match code {
            KeyCode::Char('n') => {
                self.builds.name_prompt = Some(NamePrompt {
                    text: templates::next_name(&self.builds.templates),
                    action: NameAction::FromWorn,
                });
            }
            KeyCode::Char('r') => {
                if let Some(i) = selected {
                    self.builds.name_prompt = Some(NamePrompt {
                        text: self.builds.templates[i].name.clone(),
                        action: NameAction::Rename(i),
                    });
                }
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                if let Some(i) = selected {
                    let gone = self.builds.templates.remove(i);
                    self.builds
                        .template_state
                        .select((!self.builds.templates.is_empty()).then(|| i.min(self.builds.templates.len() - 1)));
                    self.status = format!("deleted template {}", gone.name);
                    self.save_templates();
                }
            }
            KeyCode::Char(']') | KeyCode::Char('.') => self.builds.template_slot = (self.builds.template_slot + 1) % templates::SLOTS.len(),
            KeyCode::Char('[') | KeyCode::Char(',') => {
                self.builds.template_slot = (self.builds.template_slot + templates::SLOTS.len() - 1) % templates::SLOTS.len();
            }
            KeyCode::Enter => {
                if selected.is_some() {
                    self.open_piece_picker();
                }
            }
            KeyCode::Char('w') => self.wish_template(false),
            KeyCode::Char('W') => self.wish_template(true),
            _ => return false,
        }
        true
    }

    pub(super) fn save_templates(&mut self) {
        let Some(path) = self.files.as_ref().map(|f| &f.templates) else {
            return;
        };
        if let Err(message) = crate::files::save(path, &templates::format(&self.builds.templates), "templates") {
            self.status = message;
        }
    }

    /// Whether the hunter has this piece (a talisman by its id in the equipment box).
    pub fn owns_slot(&self, kind: u8, id: u16) -> bool {
        self.save.owns_equipment(kind, id)
    }

    /// The stats a template piece adds up with: the game's for armor, the record's skills for a talisman.
    pub fn piece_stats(&self, piece: &templates::Piece) -> Option<mh3u_core::armor::ArmorStats> {
        if piece.kind == 6 {
            return Some(mh3u_core::armor::ArmorStats::talisman(piece.skills.clone()));
        }
        self.game.armor_stats(piece.kind, piece.id).cloned()
    }

    pub(super) fn template_piece_of(&self, c: &Candidate) -> templates::Piece {
        templates::Piece {
            kind: c.kind,
            id: c.id,
            skills: if c.kind == 6 { c.stats.skills.clone() } else { Vec::new() },
        }
    }

    /// Finish the name popup: save, or rename.
    pub(super) fn finish_name(&mut self, prompt: &NamePrompt) {
        let name = templates::clean_name(&prompt.text);
        if name.is_empty() {
            self.status = "a template needs a name".to_string();
            return;
        }
        match prompt.action {
            NameAction::Rename(i) => {
                if let Some(t) = self.builds.templates.get_mut(i) {
                    t.name = name;
                }
            }
            NameAction::SaveSet(i) => {
                let Some(found) = self.builds.results.get(i) else { return };
                let pieces = found.pieces.iter().map(|&p| self.template_piece_of(&self.builds.pool[p])).collect();
                self.builds.templates.push(Template {
                    name: name.clone(),
                    pieces,
                });
                self.builds.template_state.select(Some(self.builds.templates.len() - 1));
                self.status = format!("saved template {name}; f switches to the templates");
            }
            NameAction::FromWorn => {
                let pieces = self
                    .worn_armor()
                    .into_iter()
                    .map(|(kind, e)| templates::Piece {
                        kind,
                        id: e.id,
                        skills: Vec::new(),
                    })
                    .collect();
                let mut t = Template {
                    name: name.clone(),
                    pieces,
                };
                t.pieces.sort_by_key(|p| templates::SLOTS.iter().position(|&k| k == p.kind));
                self.builds.templates.push(t);
                self.builds.template_state.select(Some(self.builds.templates.len() - 1));
                self.status = format!("saved what you are wearing as {name}");
            }
        }
        self.save_templates();
    }

    pub(super) fn name_key(&mut self, code: KeyCode) {
        let Some(prompt) = self.builds.name_prompt.as_mut() else { return };
        match code {
            KeyCode::Esc => self.builds.name_prompt = None,
            KeyCode::Backspace => {
                prompt.text.pop();
            }
            KeyCode::Char(c) => prompt.text.push(c),
            KeyCode::Enter => {
                if let Some(prompt) = self.builds.name_prompt.take() {
                    self.finish_name(&prompt);
                }
            }
            _ => {}
        }
    }

    /// The pieces that can go in the slot of `kind`, matching what was typed: empty the slot, then the ones you own, the ones the
    /// blacksmith offers and the rest.
    pub(super) fn piece_choices(&self, kind: u8, typed: &str) -> Vec<Choice> {
        let words: Vec<String> = typed.split_whitespace().map(str::to_lowercase).collect();
        let mut scored: Vec<(u32, Choice)> = Vec::new();
        let mut consider = |name: String, detail: String, availability: Availability, piece: templates::Piece| {
            let lower = format!("{} {}", name.to_lowercase(), detail.to_lowercase());
            let mut total = 0;
            for w in &words {
                let Some(s) = search::score(w, &lower) else { return };
                total += s;
            }
            let order = match availability {
                Availability::Owned => 2000,
                Availability::OnOffer => 1000,
                Availability::Unavailable => 0,
            };
            scored.push((
                total + order,
                Choice {
                    piece: Some(piece),
                    name,
                    availability,
                    detail,
                },
            ));
        };
        if kind == 6 {
            for e in self.save.equipment_box.iter().filter(|e| e.kind == 6) {
                let skills = e.talisman_skills();
                let detail = skills
                    .iter()
                    .map(|&(id, p)| format!("{} {p:+}", self.game.skill_name(id).unwrap_or("?")))
                    .collect::<Vec<_>>()
                    .join(", ");
                let name = self.game.equipment_name(6, e.id).unwrap_or("Talisman").to_string();
                consider(name, detail, Availability::Owned, templates::Piece { kind: 6, id: e.id, skills });
            }
        } else {
            for id in self.game.piece_ids(kind) {
                let (Some(name), Some(stats)) = (self.game.piece_name(kind, id), self.game.armor_stats(kind, id)) else {
                    continue;
                };
                let availability = if self.owns_slot(kind, id) {
                    Availability::Owned
                } else if self.at_blacksmith(kind, id) {
                    Availability::OnOffer
                } else {
                    Availability::Unavailable
                };
                let detail = stats
                    .skills
                    .iter()
                    .map(|&(s, p)| format!("{} {p:+}", self.game.skill_name(s).unwrap_or("?")))
                    .collect::<Vec<_>>()
                    .join(", ");
                consider(
                    name.to_string(),
                    detail,
                    availability,
                    templates::Piece {
                        kind,
                        id,
                        skills: Vec::new(),
                    },
                );
            }
        }
        scored.sort_by_key(|(score, c)| (std::cmp::Reverse(*score), c.name.clone()));
        let mut out: Vec<Choice> = Vec::new();
        if words.is_empty() {
            out.push(Choice {
                piece: None,
                name: "(empty this slot)".to_string(),
                availability: Availability::Unavailable,
                detail: String::new(),
            });
        }
        out.extend(scored.into_iter().map(|(_, c)| c));
        out
    }

    pub(super) fn open_piece_picker(&mut self) {
        let kind = templates::SLOTS[self.builds.template_slot];
        let choices = self.piece_choices(kind, "");
        self.builds.piece_picker = Some(PiecePicker {
            kind,
            text: String::new(),
            state: ListState::default().with_selected(Some(0)),
            choices,
        });
    }

    pub(super) fn piece_key(&mut self, code: KeyCode) {
        let Some(picker) = self.builds.piece_picker.as_mut() else { return };
        let (kind, mut text) = (picker.kind, picker.text.clone());
        match code {
            KeyCode::Esc => self.builds.piece_picker = None,
            KeyCode::Down | KeyCode::Up => {
                let last = picker.choices.len().saturating_sub(1);
                let at = picker.state.selected().unwrap_or(0);
                picker.state.select(Some(if code == KeyCode::Down {
                    (at + 1).min(last)
                } else {
                    at.saturating_sub(1)
                }));
            }
            KeyCode::Backspace | KeyCode::Char(_) => {
                match code {
                    KeyCode::Char(c) => text.push(c),
                    _ => {
                        text.pop();
                    }
                }
                let choices = self.piece_choices(kind, &text);
                if let Some(picker) = self.builds.piece_picker.as_mut() {
                    picker.text = text;
                    picker.choices = choices;
                    picker.state.select(Some(0));
                }
            }
            KeyCode::Enter => {
                let Some(picker) = self.builds.piece_picker.take() else { return };
                let at = picker.state.selected().unwrap_or(0);
                let Some(choice) = picker.choices.into_iter().nth(at) else { return };
                if let Some(i) = self.builds.template_state.selected().filter(|&i| i < self.builds.templates.len()) {
                    self.builds.templates[i].set(kind, choice.piece);
                    self.save_templates();
                }
            }
            _ => {}
        }
    }

    /// Put the pieces of the highlighted found set that you do not own yet on the wishlist.
    pub(super) fn wish_build(&mut self) {
        let Some(found) = self.builds.result_state.selected().and_then(|i| self.builds.results.get(i)) else {
            return;
        };
        let missing: Vec<(u8, u16)> = found
            .pieces
            .iter()
            .map(|&i| &self.builds.pool[i])
            .filter(|c| c.kind != 6 && !c.owned && !self.save.owns_equipment(c.kind, c.id))
            .map(|c| (c.kind, c.id))
            .collect();
        self.wish_pieces(missing);
    }

    /// Put the highlighted template's pieces you do not own on the wishlist: all of them, or just the highlighted slot.
    pub(super) fn wish_template(&mut self, only_slot: bool) {
        let Some(t) = self.builds.template_state.selected().and_then(|i| self.builds.templates.get(i)) else {
            return;
        };
        let slot_kind = templates::SLOTS[self.builds.template_slot];
        let missing: Vec<(u8, u16)> = t
            .pieces
            .iter()
            .filter(|p| p.kind != 6 && (!only_slot || p.kind == slot_kind) && !self.save.owns_equipment(p.kind, p.id))
            .map(|p| (p.kind, p.id))
            .collect();
        self.wish_pieces(missing);
    }

    pub(super) fn wish_pieces(&mut self, pieces: Vec<(u8, u16)>) {
        let mut added = 0;
        for (kind, id) in pieces {
            if !self.is_wished(kind, id) {
                self.add_wish_with_parents(kind, id);
                added += 1;
            }
        }
        self.status = match added {
            0 => "nothing to add: you own every piece, or they are already on the wishlist".to_string(),
            n => format!("added {n} piece(s) to the wishlist"),
        };
    }
}

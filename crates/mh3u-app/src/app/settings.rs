//! The Settings screen (`S`): change the settings of `config.txt` (see `crate::config`) with the keyboard.

use super::*;
use crate::config::{self, Config, Kind, When};

/// The Settings screen: the highlighted row (the first is the profile, then the settings), and the text being typed.
pub struct SettingsScreen {
    pub state: ListState,
    /// The text being typed: the new text of a text setting, or the name of a new profile.
    pub editing: Option<String>,
    /// What `editing` is for: a new profile's name (else the text setting that is highlighted).
    pub naming_profile: bool,
}

impl App {
    /// The setting the screen highlights; `None` on the profile row.
    pub fn selected_setting(&self) -> Option<&'static config::Def> {
        let row = self.settings.as_ref()?.state.selected()?;
        config::DEFS.get(row.checked_sub(1)?)
    }

    /// Whether the highlighted row is the profile.
    pub fn on_profile_row(&self) -> bool {
        self.settings.as_ref().is_some_and(|s| s.state.selected() == Some(0))
    }

    /// How many rows the screen has: the profile, then one per setting.
    pub fn setting_rows(&self) -> usize {
        config::DEFS.len() + 1
    }

    pub(super) fn open_settings(&mut self) {
        self.settings = Some(SettingsScreen {
            state: ListState::default().with_selected(Some(0)),
            editing: None,
            naming_profile: false,
        });
        self.status = format!("settings are kept in {}", self.config_path_text());
    }

    /// Where the settings file is (or would be), for the screen.
    pub fn config_file(&self) -> Option<&std::path::Path> {
        self.files.as_ref().map(|f| f.settings.as_path())
    }

    /// The name of the settings profile in use.
    pub fn profile_name(&self) -> &str {
        self.files.as_ref().map_or(config::DEFAULT_PROFILE, |f| f.profile.as_str())
    }

    /// The settings profiles there are.
    pub fn profile_names(&self) -> Vec<String> {
        self.files.as_ref().map(Files::profiles).unwrap_or_default()
    }

    fn config_path_text(&self) -> String {
        self.files
            .as_ref()
            .map_or("(no config folder)".to_string(), |f| f.settings.display().to_string())
    }

    pub(super) fn settings_key(&mut self, code: Key) {
        let Some(screen) = &mut self.settings else { return };
        if let Some(text) = &mut screen.editing {
            match code {
                Key::Esc => screen.editing = None,
                Key::Backspace => {
                    text.pop();
                }
                Key::Char(c) => text.push(c),
                Key::Enter => {
                    let text = std::mem::take(text);
                    let naming = screen.naming_profile;
                    screen.editing = None;
                    screen.naming_profile = false;
                    if naming {
                        self.new_profile(text.trim());
                    } else if let Some(def) = self.selected_setting() {
                        // an empty text is the default again
                        let value = (!text.trim().is_empty()).then(|| text.trim().to_string());
                        self.change_setting(def.key, value.as_deref());
                    }
                }
                _ => {}
            }
            return;
        }
        match code {
            Key::Esc | Key::Char('q' | 'S') => self.settings = None,
            Key::Down | Key::Char('j') => self.move_setting(1),
            Key::Up | Key::Char('k') => self.move_setting(-1),
            Key::Home | Key::Char('g') => self.move_setting(isize::MIN),
            Key::End | Key::Char('G') => self.move_setting(isize::MAX),
            Key::Char('w') => self.save_config_file(),
            Key::Char('r') => self.reload_config(),
            _ if self.on_profile_row() => match code {
                Key::Right | Key::Char('l' | ' ') => self.step_profile(1),
                Key::Left | Key::Char('h') => self.step_profile(-1),
                Key::Enter | Key::Char('n') => {
                    if let Some(screen) = &mut self.settings {
                        screen.editing = Some(String::new());
                        screen.naming_profile = true;
                    }
                }
                _ => {}
            },
            _ => {
                let Some(def) = self.selected_setting() else { return };
                match code {
                    Key::Right | Key::Char('l' | ' ') => self.step_setting(def, 1),
                    Key::Left | Key::Char('h') => self.step_setting(def, -1),
                    Key::Enter => match def.kind {
                        Kind::Text => {
                            let current = self.config.text(def.key).unwrap_or_default().to_string();
                            if let Some(screen) = &mut self.settings {
                                screen.editing = Some(current);
                            }
                        }
                        _ => self.step_setting(def, 1),
                    },
                    Key::Char('d') | Key::Delete | Key::Backspace => self.change_setting(def.key, None),
                    _ => {}
                }
            }
        }
    }

    fn move_setting(&mut self, step: isize) {
        let rows = self.setting_rows();
        if let Some(screen) = &mut self.settings {
            let at = stepped(screen.state.selected(), step, rows);
            screen.state.select(Some(at));
        }
    }

    fn step_setting(&mut self, def: &'static config::Def, step: isize) {
        if let Some(next) = config::step(def, self.config.get(def.key), step) {
            self.change_setting(def.key, Some(&next));
        }
    }

    /// Go to the next or the previous settings profile.
    fn step_profile(&mut self, step: isize) {
        let names = self.profile_names();
        let at = names.iter().position(|n| n == self.profile_name()).unwrap_or(0) as isize;
        if let Some(next) = names.get((at + step).rem_euclid(names.len() as isize) as usize).cloned() {
            self.use_profile(&next);
        }
    }

    /// Start using a settings profile: its file is read (a missing file is all defaults), the choice is remembered for the next start,
    /// and what can change now does.
    pub fn use_profile(&mut self, name: &str) {
        let Some(files) = &mut self.files else { return };
        files.use_profile(name);
        let remembered = files.remember_profile();
        self.reload_config();
        if let Err(e) = remembered {
            self.status = e;
        } else if !self.config.problems.is_empty() {
            // reload_config has said what is wrong with the file
        } else {
            self.status = format!("profile {name} ({})", self.config_path_text());
        }
    }

    /// Make a new profile: a copy of the settings in use, under a new name, and switch to it. An existing name just switches to it.
    fn new_profile(&mut self, name: &str) {
        if let Err(why) = config::valid_profile_name(name) {
            return self.status = why;
        }
        let Some(files) = &self.files else { return };
        let mut target = files.clone();
        target.use_profile(name);
        if !target.settings.exists() {
            let source = std::fs::read_to_string(&files.settings).unwrap_or_else(|_| config::template());
            if let Err(e) = crate::files::save(&target.settings, &source, "the new profile") {
                return self.status = e;
            }
        }
        self.use_profile(name);
    }

    /// Set a setting, or with `None` put it back to its default: in the program and in the file (only that line of it changes).
    pub fn change_setting(&mut self, key: &str, value: Option<&str>) {
        if let Some(value) = value
            && let Err(why) = config::check(key, value)
        {
            return self.status = why;
        }
        let Some(def) = config::def(key) else { return };
        let path = self.files.as_ref().map(|f| f.settings.clone());
        let old = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_else(config::template);
        let text = config::edit_text(&old, key, value);
        if let Some(path) = &path
            && let Err(e) = crate::files::save(path, &text, "settings")
        {
            return self.status = e;
        }
        match value {
            Some(v) => self.config.set(key, v),
            None => self.config.unset(key),
        }
        self.apply_setting(key);
        self.status = match (value, def.when) {
            (Some(v), When::Now) => format!("{} set to {v}", def.label),
            (Some(v), When::NextStart) => format!("{} set to {v} (takes effect at the next start)", def.label),
            (None, _) => format!(
                "{} back to its default, {}",
                def.label,
                if def.default.is_empty() { "unset" } else { def.default }
            ),
        };
    }

    /// What a changed setting does right away; the screen reads the colors and icons from `App::config` itself.
    fn apply_setting(&mut self, key: &str) {
        if key == "hunt_goal" {
            self.hunts.goal = self.config.hunt_goal();
            self.hunts.stale = true;
            if self.tab == Tab::Hunts {
                self.refresh_hunts();
            }
        }
    }

    /// Save the settings to the profile's file now: made from the commented template if there is none, with every setting that is set
    /// written into it (changes made on the screen are saved as they are made, so this finds nothing new to write unless the file was lost).
    fn save_config_file(&mut self) {
        let Some(path) = self.files.as_ref().map(|f| f.settings.clone()) else {
            return self.status = "no config folder to write the file in".into();
        };
        let mut text = std::fs::read_to_string(&path).unwrap_or_else(|_| config::template());
        let mut written = 0;
        for def in config::DEFS {
            if self.config.is_set(def.key) {
                text = config::edit_text(&text, def.key, Some(self.config.get(def.key)));
                written += 1;
            }
        }
        self.status = match crate::files::save(&path, &text, "settings") {
            Ok(()) if written == 0 => format!("saved {} (every setting is in it, commented out)", path.display()),
            Ok(()) => format!("saved {written} setting(s) to {}", path.display()),
            Err(e) => e,
        };
    }

    /// Read the profile's file again, for a file edited by hand.
    fn reload_config(&mut self) {
        let before = self.status.clone();
        self.load_config();
        self.apply_setting("hunt_goal");
        if self.config.problems.is_empty() {
            self.status = format!("read {} again", self.config_path_text());
        } else if self.status == before {
            self.status = format!("{}: {}", self.config_path_text(), self.config.problems[0]);
        }
    }

    /// Read `config.txt` again (it is read once at the start); problems in it go to the status line.
    pub(super) fn load_config(&mut self) {
        let text = self.files.as_ref().and_then(|f| std::fs::read_to_string(&f.settings).ok());
        self.config = text.as_deref().map(Config::parse).unwrap_or_default();
        self.hunts.goal = self.config.hunt_goal();
        if let Some(first) = self.config.problems.first() {
            self.status = format!(
                "config.txt: {first}{}",
                if self.config.problems.len() > 1 {
                    format!(" (and {} more)", self.config.problems.len() - 1)
                } else {
                    String::new()
                }
            );
        }
    }
}

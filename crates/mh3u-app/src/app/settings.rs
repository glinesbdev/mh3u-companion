//! The Settings screen (`S`): change the settings of `config.txt` (see `crate::config`) with the keyboard.

use super::*;
use crate::config::{self, Config, Kind, When};

/// The Settings screen: the highlighted setting, and the text being typed for a text setting.
pub struct SettingsScreen {
    pub state: ListState,
    /// The new text of a text setting, while it is being typed.
    pub editing: Option<String>,
}

impl App {
    /// The settings in the order of the screen.
    pub fn setting_defs(&self) -> &'static [config::Def] {
        config::DEFS
    }

    /// The setting the screen highlights.
    pub fn selected_setting(&self) -> Option<&'static config::Def> {
        self.settings.as_ref()?.state.selected().and_then(|i| config::DEFS.get(i))
    }

    pub(super) fn open_settings(&mut self) {
        self.settings = Some(SettingsScreen {
            state: ListState::default().with_selected(Some(0)),
            editing: None,
        });
        self.status = format!("settings are kept in {}", self.config_path_text());
    }

    /// Where the settings file is (or would be), for the screen.
    pub fn config_file(&self) -> Option<&std::path::Path> {
        self.files.as_ref().map(|f| f.settings.as_path())
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
                    screen.editing = None;
                    if let Some(def) = self.selected_setting() {
                        // an empty text is the default again
                        let value = (!text.trim().is_empty()).then(|| text.trim().to_string());
                        self.change_setting(def.key, value.as_deref());
                    }
                }
                _ => {}
            }
            return;
        }
        let Some(def) = self.selected_setting() else { return };
        match code {
            Key::Esc | Key::Char('q' | 'S') => self.settings = None,
            Key::Down | Key::Char('j') => self.move_setting(1),
            Key::Up | Key::Char('k') => self.move_setting(-1),
            Key::Home | Key::Char('g') => self.move_setting(isize::MIN),
            Key::End | Key::Char('G') => self.move_setting(isize::MAX),
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
            Key::Char('w') => self.write_config_file(),
            _ => {}
        }
    }

    fn move_setting(&mut self, step: isize) {
        if let Some(screen) = &mut self.settings {
            let at = stepped(screen.state.selected(), step, config::DEFS.len());
            screen.state.select(Some(at));
        }
    }

    fn step_setting(&mut self, def: &'static config::Def, step: isize) {
        if let Some(next) = config::step(def, self.config.get(def.key), step) {
            self.change_setting(def.key, Some(&next));
        }
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

    /// Write the file with every setting commented in, unless there is one already.
    fn write_config_file(&mut self) {
        let Some(path) = self.files.as_ref().map(|f| f.settings.clone()) else {
            return self.status = "no config folder to write the file in".into();
        };
        if path.exists() {
            return self.status = format!("{} is there already", path.display());
        }
        self.status = match crate::files::save(&path, &config::template(), "settings") {
            Ok(()) => format!("wrote {}: every setting is in it, commented out", path.display()),
            Err(e) => e,
        };
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

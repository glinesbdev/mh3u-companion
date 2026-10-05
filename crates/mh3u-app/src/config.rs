//! The settings file (`~/.config/mh3u-companion/config.txt`): colors, icons and a few defaults, editable by hand or from the Settings
//! screen of the TUI (`S`).
//!
//! The file is plain text, one `key = value` per line, with `#` comments on lines of their own. Every setting is listed in it with a
//! comment and its default as a commented-out example line; setting one means removing the `#` of its line. Changing a setting in the
//! TUI edits that one line in place and leaves the rest of the file (comments, order, anything of your own) as it was.
//!
//! The settings are described once, by [`DEFS`]; the file's text, the parser, the checks and the Settings screen all come from that table.

use crate::hunts::Goal;
use std::collections::BTreeMap;

/// What kind of value a setting holds, which decides how the Settings screen changes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One of a few words.
    Choice(&'static [&'static str]),
    /// A color: a name (`lightcyan`), a number from the 256-color palette (`245`) or `#rrggbb`.
    Color,
    /// Free text (a path or a program).
    Text,
    /// A whole number from the first to the second (inclusive).
    Number(u8, u8),
}

/// When a changed setting takes effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum When {
    Now,
    NextStart,
}

/// One setting.
#[derive(Debug, Clone, Copy)]
pub struct Def {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: Kind,
    /// The value used when the file has none. For [`Kind::Text`] it can be empty: "work it out".
    pub default: &'static str,
    pub help: &'static str,
    pub when: When,
}

/// The names of the 16 terminal colors, in the order the Settings screen steps through them.
pub const COLOR_NAMES: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "gray",
    "darkgray",
    "lightred",
    "lightgreen",
    "lightyellow",
    "lightblue",
    "lightmagenta",
    "lightcyan",
    "white",
];

/// The tabs a start tab can be, by the word used in the file.
pub const TAB_WORDS: [&str; 13] = [
    "items",
    "equipment",
    "worn",
    "crafting",
    "wishlist",
    "monsters",
    "hunts",
    "quests",
    "families",
    "skills",
    "compare",
    "builds",
    "pickups",
];

const COLOR_HELP: &str = "A color name (black, red, green, yellow, blue, magenta, cyan, gray, darkgray, lightred, lightgreen, lightyellow, lightblue, lightmagenta, lightcyan, white), a number from 0 to 255 (the 256-color palette) or #rrggbb.";

pub const DEFS: &[Def] = &[
    Def {
        key: "icons",
        label: "Icons",
        kind: Kind::Choice(&["nerd", "plain"]),
        default: "nerd",
        help: "The anvil that marks a piece the blacksmith offers. `nerd` needs a Nerd Font in the terminal; `plain` draws a hammer and pick that any font has. The MH3U_ICONS environment variable (plain) wins over this.",
        when: When::Now,
    },
    Def {
        key: "accent",
        label: "Accent color",
        kind: Kind::Color,
        default: "lightcyan",
        help: "Focus, selection and key names.",
        when: When::Now,
    },
    Def {
        key: "good",
        label: "Good color",
        kind: Kind::Color,
        default: "green",
        help: "Something you have, can afford or can do.",
        when: When::Now,
    },
    Def {
        key: "warn",
        label: "Warning color",
        kind: Kind::Color,
        default: "yellow",
        help: "Something partly there.",
        when: When::Now,
    },
    Def {
        key: "bad",
        label: "Bad color",
        kind: Kind::Color,
        default: "red",
        help: "Something missing or out of reach.",
        when: When::Now,
    },
    Def {
        key: "muted",
        label: "Muted color",
        kind: Kind::Color,
        default: "245",
        help: "Secondary text: counts, separators, key descriptions. A mid gray by default, because the terminal's own dark gray is nearly invisible in many themes.",
        when: When::Now,
    },
    Def {
        key: "faint",
        label: "Faint color",
        kind: Kind::Color,
        default: "darkgray",
        help: "Borders and scroll bars of the panes that do not have focus. Never used for text.",
        when: When::Now,
    },
    Def {
        key: "hunt_goal",
        label: "Hunt plan goal",
        kind: Kind::Choice(&["steps", "runs"]),
        default: "steps",
        help: "What the Hunt plan keeps small: the number of hunts and quests (steps) or the number of runs in all (runs). `g` on the Hunt plan tab switches it for the session.",
        when: When::Now,
    },
    Def {
        key: "start_tab",
        label: "Start tab",
        kind: Kind::Choice(&TAB_WORDS),
        default: "items",
        help: "The tab shown when the program starts.",
        when: When::NextStart,
    },
    Def {
        key: "slot",
        label: "Start slot",
        kind: Kind::Number(1, 3),
        default: "1",
        help: "Which of Cemu's three save slots to read at the start (--slot overrides). With --live the program follows the hunter the game loads.",
        when: When::NextStart,
    },
    Def {
        key: "game_dir",
        label: "Game folder",
        kind: Kind::Text,
        default: "",
        help: "The folder with the game: the dump itself (the one with code/ and content/) or a folder that holds dumps, in which case the one named like `... [Game] [0005000010118300]` is used. A leading ~ is your home folder. --game-dir and MH3U_GAME_DIR win over this.",
        when: When::NextStart,
    },
    Def {
        key: "cemu",
        label: "Cemu program",
        kind: Kind::Text,
        default: "Cemu",
        help: "The program --live starts (--cemu overrides). A name found on the path, or a full path such as /opt/cemu/Cemu.AppImage.",
        when: When::NextStart,
    },
];

pub fn def(key: &str) -> Option<&'static Def> {
    DEFS.iter().find(|d| d.key == key)
}

/// A color as the file writes it. The screen turns it into its own kind of color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpec {
    /// One of [`COLOR_NAMES`] by its position.
    Named(u8),
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl ColorSpec {
    pub fn parse(text: &str) -> Option<ColorSpec> {
        let raw = text.trim().to_ascii_lowercase();
        if let Ok(n) = raw.parse::<u8>() {
            return Some(ColorSpec::Indexed(n));
        }
        let t: String = raw.chars().filter(|c| !matches!(c, ' ' | '-' | '_')).collect();
        if let Some(hex) = t.strip_prefix('#') {
            if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            let n = u32::from_str_radix(hex, 16).ok()?;
            return Some(ColorSpec::Rgb((n >> 16) as u8, (n >> 8) as u8, n as u8));
        }
        // "grey" and "darkgrey" for the British
        let t = t.replace("grey", "gray");
        COLOR_NAMES.iter().position(|n| *n == t).map(|i| ColorSpec::Named(i as u8))
    }

    pub fn format(self) -> String {
        match self {
            ColorSpec::Named(i) => COLOR_NAMES[usize::from(i)].to_string(),
            ColorSpec::Indexed(n) => n.to_string(),
            ColorSpec::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        }
    }
}

/// The name of the profile kept in `config.txt`; any other profile `x` is `config-x.txt`.
pub const DEFAULT_PROFILE: &str = "default";

/// The file name of a profile.
pub fn profile_file(profile: &str) -> String {
    if profile == DEFAULT_PROFILE {
        "config.txt".to_string()
    } else {
        format!("config-{profile}.txt")
    }
}

/// The profile a file name stands for (`config.txt`, `config-work.txt`), if it is one.
pub fn profile_of_file(name: &str) -> Option<String> {
    if name == "config.txt" {
        return Some(DEFAULT_PROFILE.to_string());
    }
    let profile = name.strip_prefix("config-")?.strip_suffix(".txt")?;
    valid_profile_name(profile).is_ok().then(|| profile.to_string())
}

/// Whether a name can be a profile name: 1 to 24 letters, digits, `-` or `_`.
pub fn valid_profile_name(name: &str) -> Result<(), String> {
    let ok = (1..=24).contains(&name.chars().count()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err("a profile name is 1 to 24 letters, digits, - or _".to_string())
    }
}

/// Whether `value` is allowed for the setting `key`; otherwise why not, in words for the status line.
pub fn check(key: &str, value: &str) -> Result<(), String> {
    let Some(def) = def(key) else {
        return Err(format!("no setting called {key}"));
    };
    let value = value.trim();
    match def.kind {
        Kind::Choice(choices) if choices.contains(&value) => Ok(()),
        Kind::Choice(choices) => Err(format!("{key} is one of {}", choices.join(", "))),
        Kind::Color if ColorSpec::parse(value).is_some() => Ok(()),
        Kind::Color => Err(format!("{key} is a color name, a number from 0 to 255 or #rrggbb")),
        Kind::Number(low, high) if value.parse::<u8>().is_ok_and(|n| (low..=high).contains(&n)) => Ok(()),
        Kind::Number(low, high) => Err(format!("{key} is a number from {low} to {high}")),
        Kind::Text => Ok(()),
    }
}

/// The settings that are set in a file, with the problems found in it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    values: BTreeMap<String, String>,
    /// Lines that were not understood, as `line N: what`.
    pub problems: Vec<String>,
}

fn strip_quotes(value: &str) -> &str {
    let v = value.trim();
    v.strip_prefix('"').and_then(|r| r.strip_suffix('"')).unwrap_or(v)
}

impl Config {
    pub fn parse(text: &str) -> Config {
        let mut out = Config::default();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                out.problems.push(format!("line {}: not `key = value`", n + 1));
                continue;
            };
            let (key, value) = (key.trim(), strip_quotes(value));
            match check(key, value) {
                Ok(()) => {
                    out.values.insert(key.to_string(), value.to_string());
                }
                Err(why) => out.problems.push(format!("line {}: {why}", n + 1)),
            }
        }
        out
    }

    /// The value of a setting: the file's, or the default.
    pub fn get(&self, key: &str) -> &str {
        self.values
            .get(key)
            .map(String::as_str)
            .or_else(|| def(key).map(|d| d.default))
            .unwrap_or("")
    }

    /// Whether the file sets this one.
    pub fn is_set(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.trim().to_string());
    }

    pub fn unset(&mut self, key: &str) {
        self.values.remove(key);
    }

    /// A color setting.
    pub fn color(&self, key: &str) -> ColorSpec {
        ColorSpec::parse(self.get(key))
            .or_else(|| def(key).and_then(|d| ColorSpec::parse(d.default)))
            .unwrap_or(ColorSpec::Indexed(7))
    }

    /// Whether the plain icons are chosen.
    pub fn plain_icons(&self) -> bool {
        self.get("icons") == "plain"
    }

    pub fn hunt_goal(&self) -> Goal {
        if self.get("hunt_goal") == "runs" {
            Goal::FewestRuns
        } else {
            Goal::FewestSteps
        }
    }

    /// The position of the start tab in the list of tabs.
    pub fn start_tab(&self) -> usize {
        TAB_WORDS.iter().position(|w| *w == self.get("start_tab")).unwrap_or(0)
    }

    pub fn slot(&self) -> u8 {
        self.get("slot").parse().ok().filter(|n| (1..=3).contains(n)).unwrap_or(1)
    }

    /// A text setting that is set, if it is not empty.
    pub fn text(&self, key: &str) -> Option<&str> {
        Some(self.get(key)).filter(|v| !v.is_empty())
    }
}

/// A new file: a header, then every setting with its comment and its default as a commented-out line.
pub fn template() -> String {
    let mut out = String::from(
        "# MH3U Companion settings.\n\
         #\n\
         # One `key = value` per line. Lines that start with # are comments and are ignored. To set something,\n\
         # remove the # in front of its line and change the value. Put a comment on a line of its own, not after a value.\n\
         # The Settings screen of the program (press S) edits this file for you and leaves the comments alone.\n\
         # Settings marked \"next start\" take effect when the program is started again.\n",
    );
    for def in DEFS {
        out.push('\n');
        out += &format!("# {}", def.label);
        if def.when == When::NextStart {
            out += " (next start)";
        }
        let help = if matches!(def.kind, Kind::Color) {
            format!("{} {COLOR_HELP}", def.help)
        } else {
            def.help.to_string()
        };
        out += &format!(": {}\n", wrap_comment(&help));
        match def.kind {
            Kind::Choice(choices) => out += &format!("# Choices: {}\n", choices.join(", ")),
            Kind::Number(low, high) => out += &format!("# A number from {low} to {high}.\n"),
            Kind::Color | Kind::Text => {}
        }
        if let Some(example) = example(def) {
            out += &format!("# Example: {} = {example}\n", def.key);
        }
        out += format!("# {} = {}", def.key, def.default).trim_end();
        out.push('\n');
    }
    out
}

/// The profile shipped in the repo (`config/default.txt`) for a start: every setting written out at the value the program was made with,
/// with its comment, instead of commented out. Two are left blank on purpose: where the game dump and Cemu are is not known, so
/// the program asks for the folder (`game_dir`) or uses the path (`cemu` as `Cemu`) until they are filled in.
pub fn default_profile_text() -> String {
    let mut out = template();
    for def in DEFS {
        let value = match def.key {
            "game_dir" | "cemu" => "",
            _ => def.default,
        };
        let shown = format!("{} = {value}", def.key).trim_end().to_string();
        let commented = format!("# {} = {}", def.key, def.default).trim_end().to_string();
        out = out.replacen(&format!("{commented}\n"), &format!("{shown}\n"), 1);
    }
    out = out.replacen(
        "# MH3U Companion settings.\n",
        "# MH3U Companion settings: the default profile, with every setting at the value the program was made with.\n\
         # Copy this file to ~/.config/mh3u-companion/config.txt (or to config-NAME.txt for a profile called NAME), or point at it with\n\
         # --config PATH. game_dir and cemu are blank on purpose: fill in where your game dump and Cemu are (the program needs a game_dir, from\n\
         # here, --game-dir or MH3U_GAME_DIR; a blank cemu starts `Cemu` from the path).\n",
        1,
    );
    out
}

/// A value to show as an example, different from the default.
fn example(def: &Def) -> Option<&'static str> {
    Some(match def.key {
        "icons" => "plain",
        "accent" => "#00afff",
        "good" => "lightgreen",
        "warn" => "214",
        "bad" => "lightred",
        "muted" => "#8a8a8a",
        "faint" => "238",
        "hunt_goal" => "runs",
        "start_tab" => "builds",
        "slot" => "2",
        "game_dir" => "/home/me/games/mh3u",
        "cemu" => "/opt/cemu/Cemu.AppImage",
        _ => return None,
    })
}

/// Break a long comment over lines of about 100 characters, each starting with `# ` after the first.
fn wrap_comment(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.len() + 1 + word.len() > 96 {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line += word;
    }
    lines.push(line);
    lines.join("\n# ")
}

/// Set `key` to `value` in the file's text, changing only that line: the line that sets it, else its commented-out example line, else a
/// new line at the end. With `None` the setting is put back to its default by turning its line into a comment. Everything else in the
/// text, comments and lines of your own included, is left alone. Returns the new text.
pub fn edit_text(text: &str, key: &str, value: Option<&str>) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let key_of = |line: &str| line.split_once('=').map(|(k, _)| k.trim().to_string());
    let active = lines
        .iter()
        .position(|l| !l.trim_start().starts_with('#') && key_of(l).as_deref() == Some(key));
    let commented = || {
        lines.iter().rposition(|l| {
            l.trim_start()
                .strip_prefix('#')
                .is_some_and(|rest| key_of(rest).as_deref() == Some(key) && !rest.trim_start().starts_with("Example"))
        })
    };
    match (value, active) {
        (Some(v), Some(i)) => lines[i] = format!("{key} = {v}"),
        (Some(v), None) => match commented() {
            Some(i) => lines[i] = format!("{key} = {v}"),
            None => lines.push(format!("{key} = {v}")),
        },
        (None, Some(i)) => {
            let default = def(key).map_or("", |d| d.default);
            lines[i] = format!("# {key} = {default}").trim_end().to_string();
        }
        (None, None) => {}
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

/// The next value after `current` when stepping through a setting's choices (`step` is 1 or -1); `None` for settings that are typed.
/// A color written another way (a number, `#rrggbb`) is not in the list of names: the first step goes to an end of the list.
pub fn step(def: &Def, current: &str, step: isize) -> Option<String> {
    let cycle = |list: Vec<String>, current: &str| -> String {
        let next = match list.iter().position(|v| v == current) {
            Some(i) => (i as isize + step).rem_euclid(list.len() as isize) as usize,
            None if step > 0 => 0,
            None => list.len() - 1,
        };
        list[next].clone()
    };
    match def.kind {
        Kind::Choice(choices) => Some(cycle(choices.iter().map(|c| c.to_string()).collect(), current)),
        Kind::Number(low, high) => Some(cycle((low..=high).map(|n| n.to_string()).collect(), current)),
        Kind::Color => {
            let current = ColorSpec::parse(current).map(ColorSpec::format).unwrap_or_default();
            Some(cycle(COLOR_NAMES.iter().map(|c| c.to_string()).collect(), &current))
        }
        Kind::Text => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_read_in_every_spelling() {
        assert_eq!(ColorSpec::parse("lightcyan"), Some(ColorSpec::Named(14)));
        assert_eq!(ColorSpec::parse(" Light-Cyan "), Some(ColorSpec::Named(14)));
        assert_eq!(ColorSpec::parse("light_cyan"), Some(ColorSpec::Named(14)));
        assert_eq!(ColorSpec::parse("darkgrey"), Some(ColorSpec::Named(8)));
        assert_eq!(ColorSpec::parse("245"), Some(ColorSpec::Indexed(245)));
        assert_eq!(ColorSpec::parse("#00AFFF"), Some(ColorSpec::Rgb(0, 175, 255)));
        for bad in ["", "256", "#12345", "#gggggg", "purple", "-1"] {
            assert_eq!(ColorSpec::parse(bad), None, "{bad}");
        }
        for text in ["lightcyan", "245", "#00afff", "red"] {
            assert_eq!(ColorSpec::parse(text).unwrap().format(), text);
        }
    }

    #[test]
    fn the_file_is_read_and_bad_lines_are_reported_not_fatal() {
        let c = Config::parse(
            "# a comment\n\nicons = plain\naccent = #00afff\nslot = \"2\"\nhunt_goal = fast\nnonsense\nunknown = 1\nslot = 9\n",
        );
        assert_eq!(c.get("icons"), "plain");
        assert!(c.plain_icons());
        assert_eq!(c.color("accent"), ColorSpec::Rgb(0, 175, 255));
        assert_eq!(c.slot(), 2, "quotes are allowed, and a later bad value does not replace a good one");
        assert_eq!(c.hunt_goal(), Goal::FewestSteps, "a bad value is the default");
        assert_eq!(c.problems.len(), 4, "{:?}", c.problems);
        assert!(c.problems.iter().any(|p| p.starts_with("line 6") && p.contains("steps, runs")));
        assert!(
            c.problems
                .iter()
                .any(|p| p.starts_with("line 8") && p.contains("no setting called unknown"))
        );
        // what is not set is the default
        assert_eq!(c.get("cemu"), "Cemu");
        assert_eq!(c.color("muted"), ColorSpec::Indexed(245));
        assert_eq!(c.text("game_dir"), None);
        assert_eq!(c.text("cemu"), Some("Cemu"));
        assert_eq!(c.start_tab(), 0);
        assert!(!Config::default().plain_icons());
    }

    #[test]
    fn the_template_has_every_setting_commented_out_and_reads_as_all_defaults() {
        let t = template();
        for d in DEFS {
            let line = format!("# {} = {}", d.key, d.default);
            assert!(t.contains(&format!("{}\n", line.trim_end())), "{}", d.key);
            assert!(t.contains(&format!("# {}", d.label)));
        }
        assert!(t.contains("# Example: icons = plain"));
        assert!(t.lines().all(|l| l.len() <= 130), "comments are wrapped");
        let c = Config::parse(&t);
        assert!(c.problems.is_empty(), "{:?}", c.problems);
        assert!(DEFS.iter().all(|d| !c.is_set(d.key)));
        // every example is a legal value
        for d in DEFS {
            if let Some(e) = example(d) {
                assert_eq!(check(d.key, e), Ok(()), "{}", d.key);
            }
            assert!(check(d.key, d.default).is_ok() || d.default.is_empty(), "{}", d.key);
        }
    }

    #[test]
    fn editing_one_setting_changes_only_its_line() {
        let t = template();
        let edited = edit_text(&t, "icons", Some("plain"));
        assert!(edited.contains("\nicons = plain\n"));
        assert!(!edited.contains("# icons = nerd\n"), "the example line became the setting");
        assert!(edited.contains("# Example: icons = plain"), "the example comment stays");
        assert_eq!(edited.lines().count(), t.lines().count(), "one line replaced, none added");
        // changing it again replaces the setting line
        let again = edit_text(&edited, "icons", Some("nerd"));
        assert!(again.lines().any(|l| l == "icons = nerd") && !again.lines().any(|l| l == "icons = plain"));
        // putting it back turns the line into a comment with the default
        let reset = edit_text(&again, "icons", None);
        assert_eq!(reset, t, "back to the template exactly");
        // a file of one's own: comments and other lines are kept, a missing setting is added at the end
        let own = "# mine\naccent = red\n\n# note\n";
        let with = edit_text(own, "good", Some("blue"));
        assert_eq!(with, "# mine\naccent = red\n\n# note\ngood = blue\n");
        assert_eq!(edit_text(own, "accent", Some("white")), "# mine\naccent = white\n\n# note\n");
        assert_eq!(edit_text(own, "bad", None), own, "nothing to reset");
        assert_eq!(edit_text(own, "accent", None), "# mine\n# accent = lightcyan\n\n# note\n");
        // the result is read back
        assert_eq!(Config::parse(&edit_text(&t, "slot", Some("3"))).slot(), 3);
    }

    #[test]
    fn values_step_through_their_choices_and_wrap() {
        let d = |k| def(k).unwrap();
        assert_eq!(step(d("icons"), "nerd", 1).as_deref(), Some("plain"));
        assert_eq!(step(d("icons"), "plain", 1).as_deref(), Some("nerd"));
        assert_eq!(step(d("icons"), "nerd", -1).as_deref(), Some("plain"));
        assert_eq!(step(d("slot"), "3", 1).as_deref(), Some("1"));
        assert_eq!(step(d("slot"), "1", -1).as_deref(), Some("3"));
        assert_eq!(step(d("accent"), "lightcyan", 1).as_deref(), Some("white"));
        assert_eq!(step(d("accent"), "black", -1).as_deref(), Some("white"));
        assert_eq!(
            step(d("accent"), "245", 1).as_deref(),
            Some("black"),
            "a custom color steps in from the start of the list"
        );
        assert_eq!(step(d("game_dir"), "", 1), None, "text is typed");
    }

    #[test]
    fn profile_names_and_their_files() {
        assert_eq!(profile_file("default"), "config.txt");
        assert_eq!(profile_file("work"), "config-work.txt");
        assert_eq!(profile_of_file("config.txt").as_deref(), Some("default"));
        assert_eq!(profile_of_file("config-work.txt").as_deref(), Some("work"));
        assert_eq!(profile_of_file("config-.txt"), None);
        assert_eq!(profile_of_file("config-a b.txt"), None);
        assert_eq!(profile_of_file("wishlist.txt"), None);
        assert!(valid_profile_name("my_profile-2").is_ok());
        for bad in ["", "a b", "a/b", "..", &"x".repeat(25)] {
            assert!(valid_profile_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_shipped_default_profile_is_every_setting_at_its_default_with_two_blanks() {
        let text = default_profile_text();
        let c = Config::parse(&text);
        assert!(c.problems.is_empty(), "{:?}", c.problems);
        for d in DEFS {
            assert!(c.is_set(d.key), "{} is written out", d.key);
            let expect = if matches!(d.key, "game_dir" | "cemu") { "" } else { d.default };
            assert_eq!(c.get(d.key), expect, "{}", d.key);
        }
        assert_eq!((c.slot(), c.start_tab(), c.hunt_goal()), (1, 0, Goal::FewestSteps));
        assert_eq!(
            (c.text("cemu"), c.text("game_dir")),
            (None, None),
            "blank means look for it / use Cemu"
        );
        assert!(text.lines().any(|l| l == "cemu =") && text.lines().any(|l| l == "game_dir ="));
        assert!(
            text.lines().any(|l| l == "icons = nerd")
                && text.lines().any(|l| l == "slot = 1")
                && text.lines().any(|l| l == "start_tab = items")
        );
        assert!(
            text.contains("# Example: cemu = /opt/cemu/Cemu.AppImage"),
            "the comments are still there"
        );
        // the copy kept in the repo is this text; to renew it: cargo run -p mh3u-tools -- default-config > config/default.txt
        let shipped = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/default.txt")).unwrap_or_default();
        assert_eq!(
            shipped, text,
            "config/default.txt is out of date: cargo run -p mh3u-tools -- default-config > config/default.txt"
        );
    }

    #[test]
    fn check_says_what_is_wrong() {
        assert_eq!(check("slot", "2"), Ok(()));
        assert!(check("slot", "4").unwrap_err().contains("1 to 3"));
        assert!(check("accent", "blurple").unwrap_err().contains("#rrggbb"));
        assert!(check("icons", "x").unwrap_err().contains("nerd, plain"));
        assert_eq!(check("cemu", "anything at all"), Ok(()));
        assert!(check("nope", "1").is_err());
    }
}

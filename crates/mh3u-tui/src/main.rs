mod keymap;
mod theme;
mod ui;

#[cfg(feature = "edit")]
use anyhow::bail;
use anyhow::{Context, Result};
use clap::Parser;
use mh3u_app::TITLE_ID;
use mh3u_app::app::{App, Live};
use mh3u_app::config::Config;
use mh3u_app::files::{Files, slot_of};
use mh3u_core::{gamedata, gamedata::GameData, live, procmem::ProcMem, save::Save};
use std::path::PathBuf;

/// A companion for Monster Hunter 3 Ultimate on Cemu: read a save and see what you hold, can make and need.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The game dump folder (the one with code/ and content/). Default: `game_dir` in the settings file. A folder of dumps is fine
    #[arg(long, env = "MH3U_GAME_DIR", value_name = "DIR")]
    game_dir: Option<PathBuf>,
    /// A save file (userN); overrides --slot
    #[arg(long, env = "MH3U_SAVE", value_name = "FILE")]
    save: Option<PathBuf>,
    /// Which of Cemu's three save slots to read (with --live the app follows the hunter the game loads, so this is only the start).
    /// Default: `slot` in config.txt, else 1
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=3))]
    slot: Option<u8>,
    /// Start Cemu on the game and follow it live
    #[arg(long)]
    live: bool,
    /// The settings profile to use: a name (`work` is config-work.txt in the config folder) or a path to a settings file. Default: the
    /// profile last chosen on the Settings screen, else config.txt
    #[arg(long, env = "MH3U_CONFIG", value_name = "NAME|FILE")]
    config: Option<String>,
    /// The Cemu program to start with --live. Default: `cemu` in config.txt, else Cemu
    #[arg(long, requires = "live", value_name = "PATH")]
    cemu: Option<String>,
    /// Allow commands that change the running game (backs up the saves first)
    #[cfg(feature = "edit")]
    #[arg(long, requires = "live")]
    debug_edit: bool,
}

/// What to say when the game folder is missing, or `asked` (and where it came from) holds no game dump: what a game folder is and every
/// way of giving it, the flag first.
fn game_dir_help(asked: Option<(&str, &str)>) -> String {
    let what = format!(
        "A game folder is the game dump itself (the folder with content/nativeCafe in it) or a folder that holds dumps, in which case the one \
         named like `... [Game] [{TITLE_ID}]` is used."
    );
    let how = format!(
        "Give it with the --game-dir flag, for example:\n    mh3u-tui --game-dir \"/path/to/MONSTER HUNTER 3 ULTIMATE [Game] [{TITLE_ID}]\"\n\
         or set it once: MH3U_GAME_DIR in the environment, or game_dir in the settings file (press S in the app, or see config/default.txt)."
    );
    match asked {
        None => format!("no game folder was given.\n{what}\n{how}"),
        Some((folder, source)) => {
            let exists = gamedata::expand_home(folder).exists();
            let problem = if exists {
                "it holds no game dump"
            } else {
                "that folder does not exist"
            };
            format!("no game dump in {folder} (from {source}): {problem}.\n{what}\n{how}")
        }
    }
}

fn home() -> Result<PathBuf> {
    dirs::home_dir().context("no home folder")
}

/// Start Cemu on the game and read its memory. Only a process we started may be read (see `docs/live.md`).
fn start_live(game_dir: &std::path::Path, save: &std::path::Path, cemu: &str, #[cfg(feature = "edit")] editable: bool) -> Result<Live> {
    let rpx = gamedata::rpx_path(game_dir)?;
    // Own process group: Ctrl-C in the TUI or closing its terminal must not take the game down with it.
    use std::os::unix::process::CommandExt;
    let child = std::process::Command::new(cemu)
        .process_group(0)
        .arg("-g")
        .arg(&rpx)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("starting {cemu} (use --cemu to give its path)"))?;
    #[cfg(feature = "edit")]
    let mem = if editable {
        ProcMem::open_writable(child.id())
    } else {
        ProcMem::open(child.id())
    };
    #[cfg(not(feature = "edit"))]
    let mem = ProcMem::open(child.id());
    let mem = mem.context("opening Cemu's memory")?;
    // The hunters in the three save slots; the one loaded in the game is one of them.
    let names: Vec<String> = (1..=3)
        .filter_map(|n| std::fs::read(save.with_file_name(format!("user{n}"))).ok())
        .filter_map(|bytes| Save::parse(&bytes).ok())
        .map(|s| s.hunter_name)
        .collect();
    Ok(Live {
        reader: live::spawn(mem, names),
        child,
        connected: false,
    })
}

#[cfg(feature = "edit")]
/// Copy the save slots (and the system file) next to the one in use into `<backups>/<time>/`, so debug edits that get saved by
/// mistake can be undone.
fn back_up_saves(save: &std::path::Path, backups: &std::path::Path) -> Result<String> {
    let dir = save.parent().context("the save file has no folder")?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let target = backups.join(stamp.to_string());
    std::fs::create_dir_all(&target)?;
    let mut copied = 0;
    for name in ["user1", "user2", "user3", "system"] {
        if std::fs::copy(dir.join(name), target.join(name)).is_ok() {
            copied += 1;
        }
    }
    Ok(format!("EDIT MODE: backed up {copied} save files to {}", target.display()))
}

fn main() -> Result<()> {
    let Cli {
        game_dir,
        save,
        slot,
        live,
        cemu,
        #[cfg(feature = "edit")]
        debug_edit,
        config: config_choice,
    } = Cli::parse();
    // the settings file supplies what was not given on the command line
    let profile = Files::resolve_settings(config_choice.as_deref());
    let config = profile
        .as_ref()
        .and_then(|(path, _)| std::fs::read_to_string(path).ok())
        .map(|t| Config::parse(&t))
        .unwrap_or_default();
    let slot = slot.unwrap_or_else(|| config.slot());
    let cemu = cemu
        .or_else(|| config.text("cemu").map(str::to_string))
        .unwrap_or_else(|| "Cemu".to_string());
    // the game folder: --game-dir (or MH3U_GAME_DIR), else the `game_dir` setting; a folder of dumps is fine, the game's is used
    let from_command_line = game_dir.is_some();
    let asked = game_dir
        .map(|p| p.to_string_lossy().into_owned())
        .or_else(|| config.text("game_dir").map(str::to_string))
        .context(game_dir_help(None))?;
    let source = if !from_command_line {
        "the game_dir setting"
    } else if std::env::args().any(|a| a == "--game-dir" || a.starts_with("--game-dir=")) {
        "--game-dir"
    } else {
        "MH3U_GAME_DIR"
    };
    let game_dir = gamedata::find_dump(&gamedata::expand_home(&asked)).with_context(|| game_dir_help(Some((&asked, source))))?;
    let save = match save {
        Some(s) => s,
        None => home()?.join(format!(
            ".local/share/Cemu/mlc01/usr/save/00050000/{}/user/80000001/user{slot}",
            &TITLE_ID[8..]
        )),
    };

    let game = GameData::load(&game_dir).with_context(|| format!("loading game data from {}", game_dir.display()))?;
    let mut files = Files::for_slot(slot_of(&save).unwrap_or(slot));
    if let (Some(files), Some((path, name))) = (&mut files, &profile) {
        files.use_settings_file(path.clone());
        files.profile = name.clone();
    }
    #[cfg(feature = "edit")]
    let (backups, writable) = {
        let backups = files.as_ref().map(|f| f.backups.clone());
        // Debug edits are for trying things out alone: not while Cemu is set up for online play.
        if debug_edit
            && let Some(settings) = mh3u_core::online::cemu_settings_path()
            && mh3u_core::online::cemu_online_enabled(&settings)
        {
            bail!(
                "--debug-edit is not available while online play is turned on in Cemu ({}). Turn online play off for the account in Cemu's \
                 account settings first.",
                settings.display()
            );
        }
        // edits recorded earlier must be possible to take out again, so the game's memory is opened for writing in that case too
        (backups, debug_edit || files.as_ref().is_some_and(|f| f.any_debug_edits()))
    };
    let mut app = App::new(game, save.clone(), files)?;
    if live {
        #[cfg(feature = "edit")]
        {
            let note = match (debug_edit, backups) {
                (true, Some(dir)) => Some(back_up_saves(&save, &dir)?),
                (true, None) => bail!("--debug-edit needs a home folder to keep backups of the saves in"),
                (false, _) => None,
            };
            app.set_live(start_live(&game_dir, &save, &cemu, writable)?);
            if let Some(note) = note {
                app.enable_edit(note);
            }
        }
        #[cfg(not(feature = "edit"))]
        app.set_live(start_live(&game_dir, &save, &cemu)?);
    }
    let mouse = app.config.mouse();
    let mut terminal = ratatui::init();
    if mouse {
        use ratatui::crossterm::{event::EnableMouseCapture, execute};
        execute!(std::io::stdout(), EnableMouseCapture)?;
        // a panic must give the mouse back too (ratatui's own hook only restores the screen)
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = ratatui::crossterm::execute!(std::io::stdout(), ratatui::crossterm::event::DisableMouseCapture);
            previous(info);
        }));
    }
    let result = run(&mut app, &mut terminal);
    if mouse {
        let _ = ratatui::crossterm::execute!(std::io::stdout(), ratatui::crossterm::event::DisableMouseCapture);
    }
    ratatui::restore();
    app.finish();
    result
}

/// Draw, wait a moment for a key, let the app catch up with the world, until the app wants to quit.
fn run(app: &mut App, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyEventKind};
    while !app.wants_quit() {
        terminal.draw(|f| ui::draw(f, app))?;
        if event::poll(std::time::Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if let Some(code) = keymap::from_terminal(key.code) {
                        app.on_key(
                            code,
                            mh3u_app::input::Mods {
                                ctrl: key.modifiers.contains(event::KeyModifiers::CONTROL),
                            },
                        );
                    }
                }
                Event::Mouse(mouse) => {
                    if let Some(pointer) = keymap::pointer_from_terminal(mouse) {
                        app.on_pointer(pointer);
                    }
                }
                _ => {}
            }
        }
        app.tick();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_folder_error_says_what_is_wrong_and_suggests_the_flag() {
        let missing = game_dir_help(Some(("/no/such/folder", "--game-dir")));
        assert!(
            missing.starts_with("no game dump in /no/such/folder (from --game-dir): that folder does not exist."),
            "{missing}"
        );
        let empty = game_dir_help(Some((&std::env::temp_dir().to_string_lossy(), "the game_dir setting")));
        assert!(empty.contains("(from the game_dir setting): it holds no game dump."), "{empty}");
        for text in [&missing, &empty, &game_dir_help(None)] {
            assert!(text.contains("--game-dir flag, for example:"), "{text}");
            assert!(
                text.contains("mh3u-tui --game-dir \"/path/to/MONSTER HUNTER 3 ULTIMATE [Game] [0005000010118300]\""),
                "{text}"
            );
            assert!(
                text.contains("MH3U_GAME_DIR") && text.contains("game_dir in the settings file"),
                "{text}"
            );
            assert!(text.contains("content/nativeCafe"));
        }
        assert!(game_dir_help(None).starts_with("no game folder was given."));
    }
}

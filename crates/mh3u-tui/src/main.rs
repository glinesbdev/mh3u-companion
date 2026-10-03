mod app;
mod builds;
mod commands;
mod families;
mod files;
mod hunts;
mod search;
mod templates;
mod theme;
mod tree;
mod ui;
mod unlocked;
mod upgrade_path;
mod worn;

use anyhow::{Context, Result, bail};
use app::App;
use app::Live;
use clap::Parser;
use files::Files;
use mh3u_core::{gamedata, gamedata::GameData, live, procmem::ProcMem, save::Save};
use std::path::PathBuf;

const TITLE_ID: &str = "0005000010118300";
/// A companion for Monster Hunter 3 Ultimate on Cemu: read a save and see what you hold, can make and need.
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// The game dump folder (the one with code/ and content/); found under ~/games/wiiu when not given
    #[arg(long, env = "MH3U_GAME_DIR", value_name = "DIR")]
    game_dir: Option<PathBuf>,
    /// A save file (userN); overrides --slot
    #[arg(long, env = "MH3U_SAVE", value_name = "FILE")]
    save: Option<PathBuf>,
    /// Which of Cemu's three save slots to read
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..=3))]
    slot: u8,
    /// Start Cemu on the game and follow it live
    #[arg(long)]
    live: bool,
    /// The Cemu program to start with --live
    #[arg(long, default_value = "Cemu", requires = "live", value_name = "PATH")]
    cemu: String,
    /// Allow commands that change the running game (backs up the saves first)
    #[arg(long, requires = "live")]
    debug_edit: bool,
}

fn home() -> Result<PathBuf> {
    dirs::home_dir().context("no home folder")
}

/// Look for a `... [Game] [0005000010118300]` folder under ~/games/wiiu.
fn guess_game_dir() -> Option<PathBuf> {
    let dir = home().ok()?.join("games/wiiu");
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok().map(|e| e.path())).find(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains(TITLE_ID) && n.contains("[Game]"))
    })
}

/// Which hunter's list a save file belongs to: the slot number from a `userN` file name.
fn slot_of(save: &std::path::Path) -> Option<u8> {
    let n = save.file_name()?.to_str()?.strip_prefix("user")?.parse().ok()?;
    (1..=3).contains(&n).then_some(n)
}

/// Start Cemu on the game and read its memory. Only a process we started may be read (see `docs/live.md`).
fn start_live(game_dir: &std::path::Path, save: &std::path::Path, cemu: &str, editable: bool) -> Result<Live> {
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
    let mem = if editable {
        ProcMem::open_writable(child.id())
    } else {
        ProcMem::open(child.id())
    }
    .context("opening Cemu's memory")?;
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
        debug_edit,
    } = Cli::parse();
    let game_dir = game_dir
        .or_else(guess_game_dir)
        .context("game dump not found: give --game-dir or set MH3U_GAME_DIR")?;
    let save = match save {
        Some(s) => s,
        None => home()?.join(format!(
            ".local/share/Cemu/mlc01/usr/save/00050000/{}/user/80000001/user{slot}",
            &TITLE_ID[8..]
        )),
    };

    let game = GameData::load(&game_dir).with_context(|| format!("loading game data from {}", game_dir.display()))?;
    let files = Files::for_slot(slot_of(&save).unwrap_or(slot));
    let backups = files.as_ref().map(|f| f.backups.clone());
    let mut app = App::new(game, save.clone(), files)?;
    if live {
        let note = match (debug_edit, backups) {
            (true, Some(dir)) => Some(back_up_saves(&save, &dir)?),
            (true, None) => bail!("--debug-edit needs a home folder to keep backups of the saves in"),
            (false, _) => None,
        };
        app.set_live(start_live(&game_dir, &save, &cemu, debug_edit)?);
        if let Some(note) = note {
            app.enable_edit(note);
        }
    }
    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn the_slot_comes_from_the_save_file_name() {
        assert_eq!(slot_of(Path::new("/x/80000001/user2")), Some(2));
        assert_eq!(slot_of(Path::new("user3")), Some(3));
        assert_eq!(slot_of(Path::new("user4")), None);
        assert_eq!(slot_of(Path::new("system")), None);
    }
}

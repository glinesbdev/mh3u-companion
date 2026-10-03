mod app;
mod commands;
mod search;
mod theme;
mod tree;
mod ui;
mod unlocked;

use anyhow::{Context, Result, bail};
use app::App;
use app::Live;
use mh3u_core::{gamedata, gamedata::GameData, live, procmem::ProcMem, save::Save};
use std::path::PathBuf;

const TITLE_ID: &str = "0005000010118300";
const USAGE: &str = "usage: mh3u-tui [--game-dir <dump folder with code/ and content/>] [--save <path to a userN file> | --slot <1-3>]\n\
                     [--live [--cemu <path to the Cemu program>] [--debug-edit]]\n\
                     (or set MH3U_GAME_DIR / MH3U_SAVE)";

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from).context("HOME is not set")
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

/// `$XDG_CONFIG_HOME/mh3u-companion/wishlist.txt`, or `~/.config/...` when that isn't set.
fn wishlist_path() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| home().ok().map(|h| h.join(".config")))?;
    Some(config.join("mh3u-companion/wishlist.txt"))
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

/// Copy the save slots (and the system file) next to the one in use into
/// `~/.local/share/mh3u-companion/backups/<time>/`, so debug edits that get saved by mistake can be undone.
fn back_up_saves(save: &std::path::Path) -> Result<String> {
    let dir = save.parent().context("the save file has no folder")?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let target = home()?.join(format!(".local/share/mh3u-companion/backups/{stamp}"));
    std::fs::create_dir_all(&target)?;
    let mut copied = 0;
    for name in ["user1", "user2", "user3", "system"] {
        if std::fs::copy(dir.join(name), target.join(name)).is_ok() {
            copied += 1;
        }
    }
    Ok(format!("EDIT MODE: backed up {copied} save files to {}", target.display()))
}

/// `$XDG_DATA_HOME/mh3u-companion/prices.tsv`, or `~/.local/share/...` when that isn't set.
fn prices_path() -> Option<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| home().ok().map(|h| h.join(".local/share")))?;
    Some(data.join("mh3u-companion/prices.tsv"))
}

fn main() -> Result<()> {
    let (mut live_mode, mut debug_edit, mut cemu) = (false, false, "Cemu".to_string());
    let mut slot = 1u8;
    let (mut game_dir, mut save) = (
        std::env::var_os("MH3U_GAME_DIR").map(PathBuf::from),
        std::env::var_os("MH3U_SAVE").map(PathBuf::from),
    );
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--game-dir" => game_dir = args.next().map(PathBuf::from),
            "--save" => save = args.next().map(PathBuf::from),
            "--live" => live_mode = true,
            "--debug-edit" => debug_edit = true,
            "--cemu" => cemu = args.next().with_context(|| format!("--cemu needs a path\n{USAGE}"))?,
            "--slot" => {
                slot = args
                    .next()
                    .and_then(|n| n.parse().ok())
                    .filter(|n| (1..=3).contains(n))
                    .with_context(|| format!("--slot needs 1, 2 or 3\n{USAGE}"))?
            }
            _ => bail!("{USAGE}"),
        }
    }
    let game_dir = game_dir
        .or_else(guess_game_dir)
        .with_context(|| format!("game dump not found\n{USAGE}"))?;
    let save = match save {
        Some(s) => s,
        None => home()?.join(format!(
            ".local/share/Cemu/mlc01/usr/save/00050000/{}/user/80000001/user{slot}",
            &TITLE_ID[8..]
        )),
    };

    let game = GameData::load(&game_dir).with_context(|| format!("loading game data from {}", game_dir.display()))?;
    if debug_edit && !live_mode {
        bail!("--debug-edit works on the live game, so it needs --live\n{USAGE}");
    }
    let mut app = App::new(game, save.clone(), wishlist_path(), prices_path())?;
    if live_mode {
        let note = if debug_edit { Some(back_up_saves(&save)?) } else { None };
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

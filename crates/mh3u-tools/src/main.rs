mod ansi2svg;
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use mh3u_core::{
    arc::Arc,
    diff,
    gamedata::{self, GameData},
    gmd, livesave,
    prices::{Ledger, PriceEntry, Recorded, Route, Source},
    procmem::ProcMem,
    recipes, rpx,
    save::Save,
};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

/// Developer tools for exploring Monster Hunter 3 Ultimate's data and saves. Not needed to use the companion app.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Tool,
}

#[derive(Clone, Copy, ValueEnum)]
enum RouteArg {
    Create,
    Upgrade,
}

#[derive(Subcommand)]
enum Tool {
    /// Show which bytes differ between two files, with a little context.
    Savediff { a: PathBuf, b: PathBuf },
    /// Draw a colored terminal capture (`tmux capture-pane -p -e`) as an SVG, replacing text first.
    Ansi2svg {
        capture: PathBuf,
        out: PathBuf,
        /// Text to replace, as FROM=TO.
        replacements: Vec<String>,
    },
    /// Armor pieces whose first listed material is in the pouch or box.
    UnlockGuess { save: PathBuf, game_dir: PathBuf },
    /// Pieces whose first material drops from a monster the save counts as hunted.
    UnlockMonsters { save: PathBuf, game_dir: PathBuf },
    /// Print the pouch, item box and equipment box with names.
    Items { save: PathBuf, game_dir: PathBuf },
    /// List the entries of an archive.
    Arcls { arc: PathBuf },
    /// Extract every entry of an archive as OUTDIR/NAME.TYPEHASH.
    Arcx { arc: PathBuf, outdir: PathBuf },
    /// Print every string of a text table, or only the given indexes.
    Gmd { file: PathBuf, ids: Vec<usize> },
    /// Find byte patterns (hex) in every decompressed archive entry under a folder.
    Arcsearch {
        dir: PathBuf,
        #[arg(required = true)]
        patterns: Vec<String>,
    },
    /// Show the recipe of every equipment piece matching the name.
    Recipe { game_dir: PathBuf, name: String },
    /// Find entries where at least MIN of the values appear (as big-endian 2- or 4-byte numbers) within WINDOW bytes.
    Arcprox {
        dir: String,
        window: usize,
        min: usize,
        /// Comma-separated numbers.
        values: String,
    },
    /// Note a price read off the game's screens in the ledger.
    PricesAdd {
        game_dir: PathBuf,
        ledger: PathBuf,
        route: RouteArg,
        cost: String,
        /// The exact name of the piece.
        piece: Vec<String>,
    },
    /// Look for where the ledger's costs are stored in the game's data.
    PricesHint { game_dir: PathBuf, ledger: PathBuf },
    /// Armor pieces whose price is not in the ledger yet.
    ArmorTodo { game_dir: PathBuf, ledger: PathBuf },
    /// Every monster drop list as `monster<TAB>row<TAB>rank<TAB>kind<TAB>item:quantity:percent,...`.
    Drops { game_dir: PathBuf },
    /// Take the debug edits recorded in a ledger file (`~/.local/share/mh3u-companion/debug-edits[-N].txt`) out of a save file the game is
    /// not running on. The save is copied to `<save>.before-purge` first, and the ledger file is removed.
    PurgeSave { save: PathBuf, ledger: PathBuf },
    /// Print the default settings profile (what `config/default.txt` in the repo holds): `cargo run -p mh3u-tools -- default-config > config/default.txt`.
    DefaultConfig,
    /// Every monster's first table of hit zones, one row per zone: monster id, name, row, then the eight values.
    Zones { game_dir: PathBuf },
    /// Every weapon as `kind id name`, for joining with other tables.
    WeaponNames { game_dir: PathBuf },
    /// Start Cemu on the game and record every save block found in its memory.
    CemuHost { game_dir: PathBuf, outdir: PathBuf },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Tool::Savediff { a, b } => savediff(&a, &b),
        Tool::Ansi2svg {
            capture,
            out,
            replacements,
        } => {
            let text = std::fs::read_to_string(&capture).with_context(|| capture.display().to_string())?;
            let replacements: Vec<(String, String)> = replacements
                .iter()
                .filter_map(|a| a.split_once('=').map(|(f, t)| (f.to_string(), t.to_string())))
                .collect();
            std::fs::write(out, ansi2svg::render(&text, &replacements))?;
            Ok(())
        }
        Tool::UnlockGuess { save, game_dir } => unlock_guess(&save, &game_dir),
        Tool::UnlockMonsters { save, game_dir } => unlock_monsters(&save, &game_dir),
        Tool::Items { save, game_dir } => items(&save, &game_dir),
        Tool::Arcls { arc } => arcls(&arc),
        Tool::Arcx { arc, outdir } => arcx(&arc, &outdir),
        Tool::Gmd { file, ids } => gmd_strings(&file, &ids),
        Tool::Arcsearch { dir, patterns } => arcsearch(&dir, &patterns),
        Tool::Recipe { game_dir, name } => recipe(&game_dir, &name),
        Tool::Arcprox { dir, window, min, values } => arcprox(&dir, window, min, &values),
        Tool::PricesAdd {
            game_dir,
            ledger,
            route,
            cost,
            piece,
        } => prices_add(&game_dir, &ledger, route, &cost, &piece.join(" ")),
        Tool::PricesHint { game_dir, ledger } => prices_hint(&game_dir, &ledger),
        Tool::ArmorTodo { game_dir, ledger } => armor_todo(&game_dir, &ledger),
        Tool::Drops { game_dir } => drops(&game_dir),
        Tool::WeaponNames { game_dir } => weapon_names(&game_dir),
        Tool::Zones { game_dir } => zones(&game_dir),
        Tool::DefaultConfig => {
            print!("{}", mh3u_app::config::default_profile_text());
            Ok(())
        }
        Tool::PurgeSave { save, ledger } => purge_save(&save, &ledger),
        Tool::CemuHost { game_dir, outdir } => cemu_host(&game_dir, &outdir),
    }
}

fn purge_save(save: &Path, ledger: &Path) -> Result<()> {
    use mh3u_core::debug_edits::Ledger;
    let mut bytes = read(save)?;
    let recorded = Ledger::parse(&std::fs::read_to_string(ledger).with_context(|| ledger.display().to_string())?);
    if recorded.is_empty() {
        println!("no debug edits are recorded in {}", ledger.display());
        return Ok(());
    }
    let (patches, report) = recorded.purge(&bytes);
    let mut backup = save.as_os_str().to_owned();
    backup.push(".before-purge");
    std::fs::copy(save, &backup).with_context(|| "copying the save first")?;
    for patch in &patches {
        mh3u_core::edit::apply(&mut bytes, patch);
    }
    std::fs::write(save, &bytes)?;
    if report.kept.is_empty() {
        std::fs::remove_file(ledger)?;
    }
    println!("{} (the save before: {})", report.summary(), Path::new(&backup).display());
    Ok(())
}

fn zones(game_dir: &Path) -> Result<()> {
    let game = GameData::load(game_dir)?;
    for id in 0..200u16 {
        let Some(name) = game.monster_name(id) else { continue };
        for (row, z) in game.hit_zones(id).iter().enumerate() {
            println!(
                "{id}\t{name}\t{row}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                z.cut, z.impact, z.shot, z.fire, z.water, z.ice, z.thunder, z.dragon
            );
        }
    }
    Ok(())
}

fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).with_context(|| path.display().to_string())
}

fn savediff(a: &Path, b: &Path) -> Result<()> {
    let (a, b) = (read(a)?, read(b)?);
    println!("sizes: {} -> {}", a.len(), b.len());
    for c in diff::changes(&a, &b, 8) {
        println!(
            "{:06x} ({} bytes)\n  - {}\n  + {}",
            c.offset,
            c.before.len(),
            hex(&c.before),
            hex(&c.after)
        );
    }
    Ok(())
}

fn unlock_guess(save: &Path, game_dir: &Path) -> Result<()> {
    let save = Save::parse(&read(save)?)?;
    let data = GameData::load(game_dir)?;
    println!("hunter: {}", save.hunter_name);
    for kind in [5u8, 1, 2, 3, 4] {
        for id in data.piece_ids(kind) {
            let (Some(recipe), Some(name)) = (data.recipe(kind, id), data.piece_name(kind, id)) else {
                continue;
            };
            if recipe.flag == 1 {
                continue;
            }
            let first = recipe.materials[0].id;
            if save.item_count(first) > 0 {
                println!(
                    "kind {kind} id {id:>3} {name:<24} first material {} x{}",
                    data.item_name(first).unwrap_or("?"),
                    save.item_count(first)
                );
            }
        }
    }
    Ok(())
}

fn unlock_monsters(save: &Path, game_dir: &Path) -> Result<()> {
    let bytes = read(save)?;
    let save = Save::parse(&bytes)?;
    let data = GameData::load(game_dir)?;
    for (m, name) in (6u16..80).map(|m| (m, data.monster_name(m))) {
        if let (n @ 1.., Some(name)) = (save.times_hunted(m), name) {
            println!("hunted: {name} x{n}");
        }
    }
    for kind in (1..=5u8).chain(7..=19) {
        for id in data.piece_ids(kind) {
            let (Some(recipe), Some(name)) = (data.recipe(kind, id), data.piece_name(kind, id)) else {
                continue;
            };
            if recipe.flag == 1 {
                continue;
            }
            let first = recipe.materials[0].id;
            let mut sources: Vec<u16> = data.drops().sources(first).iter().map(|s| s.monster).collect();
            sources.sort_unstable();
            sources.dedup();
            let any = sources.iter().any(|&m| save.times_hunted(m) > 0);
            let monsters: Vec<&str> = sources.iter().filter_map(|&m| data.monster_name(m)).collect();
            println!(
                "{} kind {kind:>2} {name:<24} first {:<18} from {monsters:?}",
                if any { "OFFER" } else { "  -  " },
                data.item_name(first).unwrap_or("?"),
            );
        }
    }
    Ok(())
}

fn items(save: &Path, game_dir: &Path) -> Result<()> {
    let save = Save::parse(&read(save)?)?;
    let data = GameData::load(game_dir)?;
    println!("hunter: {}", save.hunter_name);
    for (label, stacks) in [("pouch", &save.pouch), ("box", &save.item_box)] {
        println!("{label}:");
        for s in stacks {
            println!("  {:<24} x{:<3} (id {})", data.item_name(s.id).unwrap_or("?"), s.count, s.id);
        }
    }
    println!("equipment box:");
    for e in &save.equipment_box {
        println!(
            "  {:<16} {:<24} (kind {}, id {})",
            data.equipment_kind_label(e.kind).unwrap_or("?"),
            data.equipment_name(e.kind, e.id).unwrap_or("?"),
            e.kind,
            e.id
        );
    }
    Ok(())
}

fn arcls(arc: &Path) -> Result<()> {
    for e in Arc::parse(&read(arc)?)?.entries {
        println!("{:08x} {:>9} {:>9}  {}", e.type_hash, e.compressed_size, e.size, e.name);
    }
    Ok(())
}

fn arcx(arc: &Path, outdir: &Path) -> Result<()> {
    let data = read(arc)?;
    let arc = Arc::parse(&data)?;
    for e in &arc.entries {
        let path = outdir.join(format!("{}.{:08x}", e.name.replace('\\', "/"), e.type_hash));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, arc.read(e)?)?;
    }
    println!("extracted {} entries", arc.entries.len());
    Ok(())
}

fn gmd_strings(file: &Path, ids: &[usize]) -> Result<()> {
    let strings = gmd::parse(&read(file)?)?;
    if ids.is_empty() {
        for (i, s) in strings.iter().enumerate() {
            println!("{i:5} {s}");
        }
    } else {
        for &i in ids {
            println!("{i:5} {}", strings.get(i).map_or("<out of range>", String::as_str));
        }
    }
    Ok(())
}

fn arcsearch(dir: &Path, patterns: &[String]) -> Result<()> {
    let pats: Vec<Vec<u8>> = patterns.iter().map(|h| hex_bytes(h)).collect::<Result<_>>()?;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for ent in std::fs::read_dir(&dir)? {
            let path = ent?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|x| x == "arc") {
                let data = std::fs::read(&path)?;
                let Ok(arc) = Arc::parse(&data) else { continue };
                for e in &arc.entries {
                    let Ok(body) = arc.read(e) else { continue };
                    for (pi, pat) in pats.iter().enumerate() {
                        for (off, _) in body.windows(pat.len()).enumerate().filter(|(_, w)| *w == &pat[..]) {
                            println!("pat{pi} {} :: {} ({:08x}) @ {off:#x}", path.display(), e.name, e.type_hash);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn recipe(game_dir: &Path, name: &str) -> Result<()> {
    let data = GameData::load(game_dir)?;
    let mats = |stacks: &[mh3u_core::save::ItemStack]| -> String {
        stacks
            .iter()
            .map(|m| format!("{} x{}", data.item_name(m.id).unwrap_or("?"), m.count))
            .collect::<Vec<_>>()
            .join(", ")
    };
    for (kind, id, piece) in data.find_equipment(name) {
        let label = data.equipment_kind_label(kind).unwrap_or("?");
        let create = data.recipe(kind, id);
        let upgrade = data.upgrade(kind, id);
        if create.is_none() && upgrade.is_none() {
            println!("{label:<15} {piece:<26} (no recipe)");
        }
        if let Some(r) = create {
            println!(
                "{label:<15} {piece:<26} create:  {}  [flag {} tier {}]",
                mats(&r.materials),
                r.flag,
                r.tier
            );
        }
        if let Some(u) = upgrade {
            let parents: Vec<&str> = u.parents.iter().map(|&p| data.equipment_name(kind, p).unwrap_or("?")).collect();
            println!(
                "{label:<15} {piece:<26} upgrade from [{}]: {}",
                parents.join(" / "),
                mats(&u.materials)
            );
        }
    }
    Ok(())
}

fn drops(game_dir: &Path) -> Result<()> {
    use mh3u_core::drops::{Method, Rank};
    let game = GameData::load(game_dir)?;
    let mut methods: Vec<Method> = (1..=40u8)
        .map(Method::Break)
        .chain(Method::CARVES)
        .chain([Method::Capture])
        .collect();
    methods.sort();
    for &monster in game.drops().monsters() {
        for rank in Rank::ALL {
            for &method in &methods {
                let Some(list) = game.drops().list(monster, rank, method) else {
                    continue;
                };
                let items: Vec<String> = list
                    .iter()
                    .map(|d| format!("{}:{}:{}", game.item_name(d.item).unwrap_or("?"), d.quantity, d.percent))
                    .collect();
                println!(
                    "{}\t{monster}\t{}\t{}\t{}",
                    game.monster_name(monster).unwrap_or("?"),
                    rank.label(),
                    method.label(),
                    items.join(",")
                );
            }
        }
    }
    Ok(())
}

/// Start the emulator as our child (so we may read its memory) and serve commands from `out/cmd.txt` until it exits or
/// `out/STOP` appears (the program is left running either way). Only this process can read the emulator's memory, so
/// exploring it means asking this process. The program defaults to `Cemu -g <rpx>`; `MH3U_HOST_PROGRAM` overrides it
/// (with its arguments after, split on spaces) so the command channel can be tested against a dummy process.
///
/// Commands, one per line, results written to the named file under `out/`:
/// - `scan <hex> <file> [context=32] [max=200]`: every address holding the bytes, with hex context
/// - `read <hexaddr> <len> <file>`: raw bytes
/// - `regions <file>`: the memory map
/// - `findsave <file>`: addresses of save blocks (see `livesave`)
fn cemu_host(game_dir: &Path, out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut child = match std::env::var("MH3U_HOST_PROGRAM") {
        Ok(program) => {
            let mut parts = program.split(' ');
            Command::new(parts.next().unwrap_or("true"))
                .args(parts)
                .stdin(Stdio::null())
                .spawn()
        }
        Err(_) => {
            let rpx = std::fs::read_dir(game_dir.join("code"))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .find(|p| p.extension().is_some_and(|x| x == "rpx"))
                .context("no .rpx in the game folder")?;
            Command::new("Cemu")
                .arg("-g")
                .arg(&rpx)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(std::fs::File::create(out.join("cemu-stderr.txt"))?)
                .spawn()
        }
    }
    .context("starting the program")?;
    let mem = ProcMem::open(child.id())?;
    let mut log = std::fs::File::create(out.join("log.txt"))?;
    let mut say = |text: String| {
        use std::io::Write;
        println!("{text}");
        let _ = writeln!(log, "{text}");
    };
    say(format!(
        "started pid {}; waiting for commands in {}",
        child.id(),
        out.join("cmd.txt").display()
    ));
    loop {
        std::thread::sleep(Duration::from_millis(300));
        if child.try_wait()?.is_some() {
            say("program exited".into());
            return Ok(());
        }
        if out.join("STOP").exists() {
            let _ = std::fs::remove_file(out.join("STOP"));
            say("STOP requested; leaving the program running".into());
            return Ok(());
        }
        let Ok(commands) = std::fs::read_to_string(out.join("cmd.txt")) else {
            continue;
        };
        let _ = std::fs::remove_file(out.join("cmd.txt"));
        for line in commands.lines().map(str::trim).filter(|l| !l.is_empty()) {
            let started = Instant::now();
            let result = host_command(&mem, out, line);
            say(format!(
                "{line}  ->  {} ({} ms)",
                result.unwrap_or_else(|e| format!("error: {e:#}")),
                started.elapsed().as_millis()
            ));
        }
    }
}

fn hex_bytes(h: &str) -> Result<Vec<u8>> {
    if !h.len().is_multiple_of(2) {
        bail!("odd-length hex");
    }
    Ok((0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16))
        .collect::<Result<_, _>>()?)
}

fn host_command(mem: &ProcMem, out: &Path, line: &str) -> Result<String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts.as_slice() {
        ["scan", hex, file, rest @ ..] => {
            let pattern = hex_bytes(hex)?;
            let context: usize = rest.first().map_or(Ok(32), |c| c.parse())?;
            let max: usize = rest.get(1).map_or(Ok(200), |m| m.parse())?;
            let hits = mem.scan(&pattern)?;
            let mut text = format!("{} hit(s)\n", hits.len());
            for &addr in hits.iter().take(max) {
                let before = mem.read(addr.saturating_sub(16), 16).map(|b| hex_string(&b)).unwrap_or_default();
                let after = mem
                    .read(addr, context.max(pattern.len()))
                    .map(|b| hex_string(&b))
                    .unwrap_or_default();
                text += &format!("{addr:#x}  [{before}] {after}\n");
            }
            std::fs::write(out.join(file), text)?;
            Ok(format!("{} hit(s)", hits.len()))
        }
        ["read", addr, len, file] => {
            let data = mem.read(u64::from_str_radix(addr.trim_start_matches("0x"), 16)?, len.parse()?)?;
            std::fs::write(out.join(file), &data)?;
            Ok(format!("{} bytes", data.len()))
        }
        ["regions", file] => {
            let text: String = mem
                .regions()?
                .iter()
                .map(|r| {
                    format!(
                        "{:#x}-{:#x} {:>6} MB {}{}{}\n",
                        r.start,
                        r.end,
                        (r.end - r.start) >> 20,
                        if r.readable { 'r' } else { '-' },
                        if r.writable { 'w' } else { '-' },
                        if r.private { 'p' } else { 's' }
                    )
                })
                .collect();
            std::fs::write(out.join(file), text)?;
            Ok("ok".into())
        }
        ["findsave", file] => {
            let blocks = livesave::find_save_blocks(mem)?;
            std::fs::write(out.join(file), blocks.iter().map(|b| format!("{b:#x}\n")).collect::<String>())?;
            Ok(format!("{} block(s)", blocks.len()))
        }
        _ => bail!("unknown command"),
    }
}

fn hex_string(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

fn prices_add(game_dir: &Path, ledger_path: &Path, route: RouteArg, cost: &str, name: &str) -> Result<()> {
    let game = GameData::load(game_dir)?;
    let route = match route {
        RouteArg::Create => Route::Create,
        RouteArg::Upgrade => Route::Upgrade,
    };
    let cost: u32 = cost.replace(',', "").parse().context("cost must be a number")?;
    let matches: Vec<_> = game
        .find_equipment(name)
        .into_iter()
        .filter(|(_, _, n)| n.eq_ignore_ascii_case(name))
        .collect();
    let [(kind, id, exact)] = matches[..] else {
        bail!("'{name}' matches {} pieces; need exactly one (exact name)", matches.len())
    };
    let mut ledger = Ledger::parse(&std::fs::read_to_string(ledger_path).unwrap_or_default());
    let when = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let result = ledger.record(PriceEntry {
        kind,
        id,
        route,
        cost,
        source: Source::Notes,
        when,
        name: exact.to_string(),
        materials: None,
        parent: None,
    });
    if let Some(dir) = ledger_path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(ledger_path, ledger.format())?;
    println!(
        "{exact} ({} #{id}) {} {cost}: {}",
        game.equipment_kind_label(kind).unwrap_or("?"),
        route.label(),
        match result {
            Recorded::New => "added".to_string(),
            Recorded::Unchanged => "already there".to_string(),
            Recorded::Changed(old) => format!("changed from {old}"),
        }
    );
    Ok(())
}

/// For each way a table of costs might be laid out (value width, a divisor, a spacing between pieces), count how many ledger
/// entries put their cost at the same base address, assuming the cost sits at `base + spacing * piece id`. A base that
/// several different pieces agree on is a strong sign of where the costs are stored.
/// (kind, route, value width, divisor, spacing, base offset)
/// Armor pieces with a create recipe and no ledger price, cheapest-looking first (rarity, then how many materials).
fn weapon_names(game_dir: &Path) -> Result<()> {
    let game = GameData::load(game_dir)?;
    for kind in 7..=19u8 {
        for id in 1..400u16 {
            if let Some(name) = game.equipment_name(kind, id).filter(|n| !n.is_empty()) {
                println!("{kind}\t{id}\t{name}");
            }
        }
    }
    Ok(())
}

fn armor_todo(game_dir: &Path, ledger_path: &Path) -> Result<()> {
    let game = GameData::load(game_dir)?;
    let ledger = Ledger::parse(&std::fs::read_to_string(ledger_path).unwrap_or_default());
    let mut rows = Vec::new();
    for kind in 1..=5u8 {
        for id in 1..2000u16 {
            let (Some(recipe), Some(stats), Some(name)) =
                (game.recipe(kind, id), game.armor_stats(kind, id), game.equipment_name(kind, id))
            else {
                continue;
            };
            if ledger.get(kind, id, Route::Create).is_some() || name.is_empty() {
                continue;
            }
            rows.push((
                stats.rarity,
                recipe.materials.len(),
                kind,
                id,
                name.to_string(),
                stats.class,
                stats.gender,
            ));
        }
    }
    rows.sort_by_key(|r| (r.0, r.1, r.2, r.3));
    println!(
        "{} unpriced craftable armor pieces (rarity, materials, kind, id, name, class, gender)",
        rows.len()
    );
    for (rarity, mats, kind, id, name, class, gender) in rows {
        println!("R{rarity}\t{mats}\t{kind}\t{id}\t{name}\t{class:?}\t{gender:?}");
    }
    Ok(())
}

type Layout = (u8, &'static str, usize, u32, usize, usize);

fn prices_hint(game_dir: &Path, ledger_path: &Path) -> Result<()> {
    use std::collections::{BTreeMap, HashSet};
    let ledger = Ledger::parse(&std::fs::read_to_string(ledger_path).with_context(|| ledger_path.display().to_string())?);
    let rpx_bytes = std::fs::read(gamedata::rpx_path(game_dir)?)?;
    let sections = [("data", recipes::DATA_SECTION_ADDR), ("rodata", 0x1000_0000)];
    const STRIDES: [usize; 19] = [2, 4, 6, 8, 10, 12, 14, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 64];
    println!("{} ledger entries", ledger.entries().len());
    for (section_name, addr) in sections {
        let data = rpx::section_at(&rpx_bytes, addr)?;
        // (kind, route, width, divisor, stride, base) -> the entries that agree
        let mut votes: BTreeMap<Layout, HashSet<usize>> = BTreeMap::new();
        for (n, e) in ledger.entries().iter().enumerate() {
            for width in [1usize, 2, 4] {
                for div in [1u32, 10, 50, 100, 1000] {
                    if e.cost % div != 0 {
                        continue;
                    }
                    let value = e.cost / div;
                    if width < 4 && value >= 1 << (8 * width) {
                        continue;
                    }
                    let needle = &value.to_be_bytes()[4 - width..];
                    for pos in memchr::memmem::find_iter(&data, needle) {
                        for stride in STRIDES {
                            if let Some(base) = pos.checked_sub(stride * e.id as usize) {
                                votes
                                    .entry((e.kind, e.route.label(), width, div, stride, base))
                                    .or_default()
                                    .insert(n);
                            }
                        }
                    }
                }
            }
        }
        // How many ledger entries each kind and route has, to say how much of it a layout explains.
        let mut totals: BTreeMap<(u8, &str), usize> = BTreeMap::new();
        for e in ledger.entries() {
            *totals.entry((e.kind, e.route.label())).or_default() += 1;
        }
        // A real table explains (nearly) every price of its kind, so only layouts covering 3/4 of the entries count. One-byte
        // values match by chance all over a data section, so they also need five pieces.
        let enough = |(kind, route, width, ..): &Layout, agreeing: usize| {
            let total = totals[&(*kind, *route)];
            total >= 4 && agreeing * 4 >= total * 3 && agreeing >= if *width == 1 { 5 } else { 3 }
        };
        let mut best: Vec<_> = votes.iter().filter(|(k, v)| enough(k, v.len())).collect();
        best.sort_by_key(|(k, v)| (std::cmp::Reverse(v.len()), **k));
        let strongest = votes
            .iter()
            .filter(|(k, _)| totals[&(k.0, k.1)] >= 4)
            .map(|(k, v)| (v.len() * 100 / totals[&(k.0, k.1)], v.len(), k.0, k.1))
            .max();
        println!(
            "\n== {section_name} section: {} layout(s) explaining 3/4 or more of a kind's prices",
            best.len()
        );
        if let Some((percent, n, kind, route)) = strongest {
            println!(
                "   (best any layout did: {n} of {} {route} prices of kind {kind}, {percent}%)",
                totals[&(kind, route)]
            );
        }
        for ((kind, route, width, div, stride, base), agreeing) in best.iter().take(12) {
            let names: Vec<&str> = agreeing.iter().map(|&n| ledger.entries()[n].name.as_str()).collect();
            println!(
                "{} of {} agree: kind {kind} {route}: {width}-byte value (cost/{div}) every {stride} bytes from offset {base:#x}  [{}]",
                agreeing.len(),
                totals[&(*kind, *route)],
                names.join(", ")
            );
        }
    }
    Ok(())
}

/// See the `arcprox` command: report where several of the given numbers sit close together in an archive entry.
fn arcprox(dir: &str, window: usize, min: usize, values: &str) -> Result<()> {
    let values: Vec<u32> = values.split(',').map(|v| v.trim().parse()).collect::<Result<_, _>>()?;
    let mut stack = vec![std::path::PathBuf::from(dir)];
    while let Some(dir) = stack.pop() {
        for ent in std::fs::read_dir(&dir)? {
            let path = ent?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|x| x != "arc") {
                continue;
            }
            let data = std::fs::read(&path)?;
            let Ok(arc) = Arc::parse(&data) else { continue };
            for e in &arc.entries {
                let Ok(body) = arc.read(e) else { continue };
                for width in [2usize, 4] {
                    let positions: Vec<Vec<usize>> = values
                        .iter()
                        .map(|v| {
                            let bytes = v.to_be_bytes();
                            memchr::memmem::find_iter(&body, &bytes[4 - width..]).collect()
                        })
                        .collect();
                    let mut best: Option<(usize, usize)> = None; // (distinct values, position)
                    for &p in positions.iter().flatten() {
                        let near = positions.iter().filter(|ps| ps.iter().any(|&q| q.abs_diff(p) <= window)).count();
                        if near >= min && best.is_none_or(|(n, _)| near > n) {
                            best = Some((near, p));
                        }
                    }
                    if let Some((near, p)) = best {
                        println!(
                            "{near} of {} values ({width}-byte) near {p:#x}  {} :: {}",
                            values.len(),
                            path.display(),
                            e.name
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

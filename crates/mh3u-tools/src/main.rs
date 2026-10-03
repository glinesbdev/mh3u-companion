mod ansi2svg;
use anyhow::{Context, Result, bail};
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
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("savediff") if args.len() == 3 => {
            let a = std::fs::read(&args[1]).with_context(|| args[1].clone())?;
            let b = std::fs::read(&args[2]).with_context(|| args[2].clone())?;
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
        // ansi2svg <capture> <out.svg> [FROM=TO ...]: draw a colored terminal capture as an SVG, replacing text first
        Some("ansi2svg") if args.len() >= 3 => {
            let capture = std::fs::read_to_string(&args[1]).with_context(|| args[1].clone())?;
            let replacements: Vec<(String, String)> = args[3..]
                .iter()
                .filter_map(|a| a.split_once('=').map(|(f, t)| (f.to_string(), t.to_string())))
                .collect();
            std::fs::write(&args[2], ansi2svg::render(&capture, &replacements))?;
            Ok(())
        }
        // unlock-guess <user1> <game_dir>: armor pieces whose first listed material is in the pouch or box
        Some("unlock-guess") if args.len() == 3 => {
            let save = Save::parse(&std::fs::read(&args[1]).with_context(|| args[1].clone())?)?;
            let data = GameData::load(Path::new(&args[2]))?;
            println!("hunter: {}", save.hunter_name);
            for kind in [5u8, 1, 2, 3, 4] {
                for id in 1..2000u16 {
                    let (Some(recipe), Some(name)) = (data.recipe(kind, id), data.equipment_name(kind, id)) else {
                        continue;
                    };
                    if name.is_empty() || recipe.flag == 1 {
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
        // unlock-monsters <user> <game_dir>: pieces whose first material drops from a monster the save counts as hunted
        Some("unlock-monsters") if args.len() == 3 => {
            let bytes = std::fs::read(&args[1]).with_context(|| args[1].clone())?;
            let data = GameData::load(Path::new(&args[2]))?;
            // u16 per monster from 0x57a0; entry n is monster n + 6 (worked out from saves, see docs/formats.md)
            let hunted = |m: u16| -> u16 {
                let o = 0x57a0 + 2 * (m as usize).saturating_sub(6);
                u16::from_be_bytes([bytes[o], bytes[o + 1]])
            };
            for (m, name) in (6u16..80).map(|m| (m, data.monster_name(m))) {
                if let (n @ 1.., Some(name)) = (hunted(m), name) {
                    println!("hunted: {name} x{n}");
                }
            }
            for kind in (1..=5u8).chain(7..=19) {
                for id in 1..2000u16 {
                    let (Some(recipe), Some(name)) = (data.recipe(kind, id), data.equipment_name(kind, id)) else {
                        continue;
                    };
                    if name.is_empty() || name == "DUMMY" || recipe.flag == 1 {
                        continue;
                    }
                    let first = recipe.materials[0].id;
                    let mut sources: Vec<u16> = data.drops().sources(first).iter().map(|s| s.0).collect();
                    sources.sort_unstable();
                    sources.dedup();
                    let any = sources.iter().any(|&m| hunted(m) > 0);
                    let monsters: Vec<&str> = sources.iter().filter_map(|&m| data.monster_name(m)).collect();
                    println!(
                        "{} kind {kind:>2} {name:<24} first {:<18} from {:?}",
                        if any { "OFFER" } else { "  -  " },
                        data.item_name(first).unwrap_or("?"),
                        monsters
                    );
                }
            }
            Ok(())
        }
        // items <user1> <game_dir>: print pouch, item box and equipment box with names
        Some("items") if args.len() == 3 => {
            let save = Save::parse(&std::fs::read(&args[1]).with_context(|| args[1].clone())?)?;
            let data = GameData::load(Path::new(&args[2]))?;
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
        Some("arcls") if args.len() == 2 => {
            let data = std::fs::read(&args[1]).with_context(|| args[1].clone())?;
            for e in Arc::parse(&data)?.entries {
                println!("{:08x} {:>9} {:>9}  {}", e.type_hash, e.compressed_size, e.size, e.name);
            }
            Ok(())
        }
        // arcx <archive> <outdir>: extract every entry as <outdir>/<name>.<typehash>
        Some("arcx") if args.len() == 3 => {
            let data = std::fs::read(&args[1]).with_context(|| args[1].clone())?;
            let arc = Arc::parse(&data)?;
            for e in &arc.entries {
                let path = std::path::Path::new(&args[2]).join(format!("{}.{:08x}", e.name.replace('\\', "/"), e.type_hash));
                std::fs::create_dir_all(path.parent().unwrap())?;
                std::fs::write(&path, arc.read(e)?)?;
            }
            println!("extracted {} entries", arc.entries.len());
            Ok(())
        }
        // gmd <file> [id...]: print every string, or only the given indexes
        Some("gmd") if args.len() >= 2 => {
            let strings = gmd::parse(&std::fs::read(&args[1]).with_context(|| args[1].clone())?)?;
            if args.len() == 2 {
                for (i, s) in strings.iter().enumerate() {
                    println!("{i:5} {s}");
                }
            } else {
                for a in &args[2..] {
                    let i: usize = a.parse()?;
                    println!("{i:5} {}", strings.get(i).map_or("<out of range>", String::as_str));
                }
            }
            Ok(())
        }
        // arcsearch <dir> <hex> [hex...]: find byte patterns in every decompressed archive entry
        Some("arcsearch") if args.len() >= 3 => {
            let pats: Vec<Vec<u8>> = args[2..]
                .iter()
                .map(|h| {
                    (0..h.len())
                        .step_by(2)
                        .map(|i| u8::from_str_radix(&h[i..i + 2], 16))
                        .collect::<Result<_, _>>()
                })
                .collect::<Result<_, _>>()?;
            let mut stack = vec![std::path::PathBuf::from(&args[1])];
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
        // recipe <game_dir> <name>: show the recipe of every equipment piece matching the name
        Some("recipe") if args.len() == 3 => {
            let data = GameData::load(Path::new(&args[1]))?;
            let mats = |stacks: &[mh3u_core::save::ItemStack]| -> String {
                stacks
                    .iter()
                    .map(|m| format!("{} x{}", data.item_name(m.id).unwrap_or("?"), m.count))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            for (kind, id, name) in data.find_equipment(&args[2]) {
                let label = data.equipment_kind_label(kind).unwrap_or("?");
                let create = data.recipe(kind, id);
                let upgrade = data.upgrade(kind, id);
                if create.is_none() && upgrade.is_none() {
                    println!("{label:<15} {name:<26} (no recipe)");
                }
                if let Some(r) = create {
                    println!(
                        "{label:<15} {name:<26} create:  {}  [flag {} tier {}]",
                        mats(&r.materials),
                        r.flag,
                        r.tier
                    );
                }
                if let Some(u) = upgrade {
                    let parents: Vec<&str> = u.parents.iter().map(|&p| data.equipment_name(kind, p).unwrap_or("?")).collect();
                    println!(
                        "{label:<15} {name:<26} upgrade from [{}]: {}",
                        parents.join(" / "),
                        mats(&u.materials)
                    );
                }
            }
            Ok(())
        }
        // arcprox <dir> <window> <min> <v1,v2,...>: find entries where at least <min> of the values appear (as big-endian 2- or
        // 4-byte numbers) within <window> bytes of each other
        Some("arcprox") if args.len() == 5 => arcprox(&args[1], args[2].parse()?, args[3].parse()?, &args[4]),
        // prices-add <game_dir> <ledger> <create|upgrade> <cost> <exact piece name>: note a price read off the game's screens
        Some("prices-add") if args.len() >= 6 => {
            prices_add(Path::new(&args[1]), Path::new(&args[2]), &args[3], &args[4], &args[5..].join(" "))
        }
        // prices-hint <game_dir> <ledger>: look for where the ledger's costs are stored in the game's data
        Some("prices-hint") if args.len() == 3 => prices_hint(Path::new(&args[1]), Path::new(&args[2])),
        // armor-todo <game_dir> <ledger>: craftable armor pieces whose price is not in the ledger yet
        // drops <game_dir>: every monster drop list as `monster<TAB>rank<TAB>kind<TAB>item:quantity:percent,...`
        Some("drops") if args.len() == 2 => {
            let game = GameData::load(Path::new(&args[1]))?;
            for monster in game.drops().monsters() {
                for rank in mh3u_core::drops::Rank::ALL {
                    let mut methods: Vec<_> = (1..=40u8)
                        .map(mh3u_core::drops::Method::Break)
                        .chain(mh3u_core::drops::Method::CARVES)
                        .chain([mh3u_core::drops::Method::Capture])
                        .collect();
                    methods.sort();
                    for method in methods {
                        if let Some(list) = game.drops().list(monster, rank, method) {
                            let items: Vec<String> = list
                                .iter()
                                .map(|d| format!("{}:{}:{}", game.item_name(d.item).unwrap_or("?"), d.quantity, d.percent))
                                .collect();
                            println!(
                                "{}\t{}\t{}\t{}",
                                game.monster_name(monster).unwrap_or("?"),
                                rank.label(),
                                method.label(),
                                items.join(",")
                            );
                        }
                    }
                }
            }
            Ok(())
        }
        // weapon-names <game_dir>: every weapon as `kind id name`, for joining with other tables
        Some("weapon-names") if args.len() == 2 => weapon_names(Path::new(&args[1])),
        Some("armor-todo") if args.len() == 3 => armor_todo(Path::new(&args[1]), Path::new(&args[2])),
        // cemu-host <game_dir> <outdir>: start Cemu on the game and record every save block found in its memory
        Some("cemu-host") if args.len() == 3 => cemu_host(Path::new(&args[1]), Path::new(&args[2])),
        _ => bail!(
            "usage: mh3u-tools savediff <a> <b> | items <user1> <game_dir> | arcls <arc> | arcx <arc> <outdir> | gmd <file> [id...] | arcsearch <dir> <hex>... | recipe <game_dir> <name> | arcprox <dir> <window> <min> <v1,v2,...> | prices-add <game_dir> <ledger> <create|upgrade> <cost> <piece name> | prices-hint <game_dir> <ledger> | armor-todo <game_dir> <ledger> | weapon-names <game_dir> | drops <game_dir> | ansi2svg <capture> <out.svg> [FROM=TO...] | cemu-host <game_dir> <outdir>"
        ),
    }
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

fn prices_add(game_dir: &Path, ledger_path: &Path, route: &str, cost: &str, name: &str) -> Result<()> {
    let game = GameData::load(game_dir)?;
    let route = match route {
        "create" => Route::Create,
        "upgrade" => Route::Upgrade,
        other => bail!("route must be create or upgrade, not '{other}'"),
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

//! The `scan` debug command: look for the blacksmith's list in the game's memory.
//!
//! With the blacksmith's armor menu open, the list is in memory somewhere. This looks for runs of the piece ids the app expects on
//! offer (see `mh3u_core::shopscan`) and writes a report: where each run is (in the emulator's memory and in the game's own
//! addresses), how it differs from the expected list, and the bytes around it. Reading only; it takes a few seconds, during which the
//! screen does not move.

use super::*;
use mh3u_core::{live as livemod, procmem::ProcMem, shopscan};
use std::fmt::Write;

const KEEP: usize = 12;
/// Flags allowed to differ from the app's idea of what is on offer.
const FLAG_MISSES: usize = 2;
/// How many rows or ids a flag pattern covers.
const FLAG_SPAN: usize = 64;
/// How many flags to read past a find, to list the ones set (the pattern is shorter than the game's array).
const FLAG_READ: usize = 400;

impl App {
    pub(super) fn run_scan(&mut self, kind: u8, shown: &[String]) {
        if !self.live_connected() {
            return self.status = "scan needs a hunter loaded in the game".into();
        }
        let Some(live) = &self.live else { return };
        let wanted: HashSet<u16> = if shown.is_empty() {
            self.game.piece_ids(kind).filter(|&id| self.at_blacksmith(kind, id)).collect()
        } else {
            let mut ids = HashSet::new();
            for name in shown {
                let found = self
                    .game
                    .piece_ids(kind)
                    .find(|&id| self.game.equipment_name(kind, id).is_some_and(|n| n.eq_ignore_ascii_case(name)));
                match found {
                    Some(id) => ids.insert(id),
                    None => {
                        return self.status = format!(
                            "no {} piece is called '{name}'",
                            self.game.equipment_kind_label(kind).unwrap_or("armor")
                        );
                    }
                };
            }
            ids
        };
        let given = !shown.is_empty();
        if wanted.len() < 4 {
            return self.status = "too few pieces expected on offer to look for a list".into();
        }
        let min_len = (wanted.len() * 6 / 10).max(4);
        let result = ProcMem::open(live.child.id()).and_then(|mem| {
            let base = livemod::find_live_block(&mem, std::slice::from_ref(&self.save.hunter_name))?
                .and_then(|block| mem.read(block, 0x30).ok().and_then(|head| livemod::validate_block(&head, block)));
            // The game's own memory is the 4 GB after `base`; the rest is the emulator's.
            let within = base.map_or(0..u64::MAX, |b| b..b + (1 << 32));
            let found = shopscan::scan(&mem, within.clone(), &wanted, min_len, KEEP)?;
            let patterns = self.flag_patterns(kind, given.then_some(&wanted));
            let flags = shopscan::scan_flags(&mem, within.clone(), &patterns, if given { 0 } else { FLAG_MISSES }, KEEP)?;
            let mut report = self.flag_report(&mem, base, &flags);
            report += &self.scan_report(&mem, kind, &wanted, base, &found, false);
            // the game may list the rows of the recipe table instead of the piece ids
            let rows: HashSet<u16> = self
                .game
                .recipe_rows(kind)
                .iter()
                .enumerate()
                .filter(|(_, id)| wanted.contains(id))
                .map(|(row, _)| row as u16)
                .collect();
            let row_found = shopscan::scan(&mem, within.clone(), &rows, min_len, KEEP)?;
            report += "\nROW NUMBERS (positions in the recipe table) instead of piece ids\n\n";
            report += &self.scan_report(&mem, kind, &rows, base, &row_found, true);
            Ok((report, found.len()))
        });
        self.status = match result {
            Err(e) => format!("scan failed: {e}"),
            Ok((report, n)) => match self.write_scan_report(&report) {
                Ok(path) => format!(
                    "scan: {n} candidate list(s) (>= {min_len} of {} ids), report in {}",
                    wanted.len(),
                    path.display()
                ),
                Err(e) => e,
            },
        };
    }

    /// What the app thinks is on offer as one flag per piece: by row of the game's recipe table (the order of the blacksmith's menu) and
    /// by piece id.
    fn flag_patterns(&self, kind: u8, shown: Option<&HashSet<u16>>) -> Vec<Vec<bool>> {
        let on = |id: u16| shown.map_or_else(|| self.at_blacksmith(kind, id), |s| s.contains(&id));
        let by_row = self.game.recipe_rows(kind).iter().take(FLAG_SPAN).map(|&id| on(id)).collect();
        let by_id = (0..FLAG_SPAN as u16).map(|id| id != 0 && on(id)).collect();
        vec![by_row, by_id]
    }

    fn flag_report(&self, mem: &ProcMem, base: Option<u64>, hits: &[shopscan::FlagHit]) -> String {
        let mut out = String::from(
            "FLAGS: one flag per piece (by row of the recipe table, or by piece id), as bytes, words, bits...\n\
             (the blacksmith menu lists pieces in recipe-table order)\n\n",
        );
        for (i, h) in hits.iter().enumerate() {
            let guest = base
                .and_then(|b| h.host.checked_sub(b))
                .map_or("?".to_string(), |g| format!("{g:#010x}"));
            let _ = writeln!(
                out,
                "F{} host {:#x} guest {guest} {:?} indexed by {} with {} flag(s) different",
                i + 1,
                h.host,
                h.layout,
                if h.pattern == 0 { "recipe row" } else { "piece id" },
                h.misses
            );
            if let Some(bytes) = (0..FLAG_READ).rev().step_by(64).find_map(|n| mem.read(h.host, n * 4).ok()) {
                let set = h.layout.set_flags(&bytes, FLAG_READ);
                let _ = writeln!(
                    out,
                    "  flags set (index = {}): {set:?}",
                    if h.pattern == 0 { "row" } else { "piece id" }
                );
            }
            if let Ok(bytes) = mem.read(h.host.saturating_sub(16), 16 + 96) {
                for (n, line) in bytes.chunks(16).enumerate() {
                    let hex: Vec<String> = line.iter().map(|b| format!("{b:02x}")).collect();
                    let _ = writeln!(out, "  {:#x}  {}", h.host.saturating_sub(16) + (n * 16) as u64, hex.join(" "));
                }
            }
            out.push('\n');
        }
        if hits.is_empty() {
            out.push_str("no flags found\n\n");
        }
        out
    }

    fn write_scan_report(&self, report: &str) -> Result<PathBuf, String> {
        let dir = self
            .files
            .as_ref()
            .and_then(|f| f.backups.parent().map(std::path::Path::to_path_buf))
            .ok_or("no data folder for the report")?;
        let secs = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let path = dir.join(format!("scan-{secs}.txt"));
        crate::files::save(&path, report, "scan report")?;
        Ok(path)
    }

    fn scan_report(
        &self,
        mem: &ProcMem,
        kind: u8,
        wanted: &HashSet<u16>,
        base: Option<u64>,
        found: &[shopscan::Found],
        rows: bool,
    ) -> String {
        let name = |n: u16| {
            let id = if rows {
                self.game.recipe_rows(kind).get(usize::from(n)).copied().unwrap_or(0)
            } else {
                n
            };
            self.game.equipment_name(kind, id).unwrap_or("?")
        };
        let mut out = String::new();
        let mut expected: Vec<u16> = wanted.iter().copied().collect();
        expected.sort_unstable();
        let _ = writeln!(
            out,
            "scan for {} armor, hunter {}\nexpected on offer ({}): {}\nguest memory starts at host {}\n",
            self.game.equipment_kind_label(kind).unwrap_or("?"),
            self.save.hunter_name,
            expected.len(),
            expected
                .iter()
                .map(|id| format!("{id} {}", name(*id)))
                .collect::<Vec<_>>()
                .join(", "),
            base.map_or("unknown".to_string(), |b| format!("{b:#x}")),
        );
        for (i, f) in found.iter().enumerate() {
            let guest = base
                .and_then(|b| f.host.checked_sub(b))
                .map_or("?".to_string(), |g| format!("{g:#010x}"));
            let missing: Vec<String> = expected
                .iter()
                .filter(|id| !f.run.ids.contains(id))
                .map(|id| id.to_string())
                .collect();
            let _ = writeln!(
                out,
                "#{} host {:#x} guest {guest} stride {} length {} of {}{}\n  ids {:?}\n  expected but not in the run: {}",
                i + 1,
                f.host,
                f.run.stride,
                f.run.ids.len(),
                expected.len(),
                if f.run.ids.windows(2).any(|w| w[1] < w[0]) {
                    ", not in id order"
                } else {
                    ""
                },
                f.run.ids,
                if missing.is_empty() { "none".into() } else { missing.join(" ") },
            );
            let span = f.run.ids.len() * f.run.stride;
            let start = f.host.saturating_sub(32);
            if let Ok(bytes) = mem.read(start, 32 + span.min(160) + 32) {
                for (n, line) in bytes.chunks(16).enumerate() {
                    let hex: Vec<String> = line.iter().map(|b| format!("{b:02x}")).collect();
                    let _ = writeln!(out, "  {:#x}  {}", start + (n * 16) as u64, hex.join(" "));
                }
            }
            out.push('\n');
        }
        if found.is_empty() {
            out.push_str("nothing found: no run of expected ids at these strides\n");
        }
        out
    }
}

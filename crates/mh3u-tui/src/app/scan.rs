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

impl App {
    pub(super) fn run_scan(&mut self, kind: u8) {
        if !self.live_connected() {
            return self.status = "scan needs a hunter loaded in the game".into();
        }
        let Some(live) = &self.live else { return };
        let wanted: HashSet<u16> = self.game.piece_ids(kind).filter(|&id| self.at_blacksmith(kind, id)).collect();
        if wanted.len() < 4 {
            return self.status = "too few pieces expected on offer to look for a list".into();
        }
        let min_len = (wanted.len() * 6 / 10).max(4);
        let result = ProcMem::open(live.child.id()).and_then(|mem| {
            let base = livemod::find_live_block(&mem, std::slice::from_ref(&self.save.hunter_name))?
                .and_then(|block| mem.read(block, 0x30).ok().and_then(|head| livemod::validate_block(&head, block)));
            let found = shopscan::scan(&mem, &wanted, min_len, KEEP)?;
            let report = self.scan_report(&mem, kind, &wanted, base, &found);
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

    fn scan_report(&self, mem: &ProcMem, kind: u8, wanted: &HashSet<u16>, base: Option<u64>, found: &[shopscan::Found]) -> String {
        let name = |id: u16| self.game.equipment_name(kind, id).unwrap_or("?");
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

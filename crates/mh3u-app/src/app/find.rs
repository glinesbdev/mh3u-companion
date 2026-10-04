//! The `find` debug command: look for a weapon's sharpness bar in the game's memory.
//!
//! Give the bar of the weapon you have equipped (red first, as a database lists it) and this looks for numbers in proportion to it, in
//! any unit and layout (see `mh3u_core::barsearch`), and writes a report with the game address and the bytes around each find.
//! Reading only; it takes a while, during which the screen does not move.

use super::*;
use mh3u_core::{
    barsearch, live as livemod,
    procmem::{CHUNK_LEN, ProcMem},
};
use std::fmt::Write;

/// Hits kept in the report.
const KEEP: usize = 300;
/// Bytes shown on each side of a hit.
const CONTEXT: u64 = 32;

impl App {
    pub(super) fn run_find(&mut self, bar: &[u32]) {
        if !self.live_connected() {
            return self.status = "find needs a hunter loaded in the game".into();
        }
        let Some(live) = &self.live else { return };
        let result = ProcMem::open(live.child.id()).and_then(|mem| {
            let base = livemod::find_live_block(&mem, std::slice::from_ref(&self.save.hunter_name))?
                .and_then(|block| mem.read(block, 0x30).ok().and_then(|head| livemod::validate_block(&head, block)));
            // The game's own memory is the 4 GB after `base`; the rest is the emulator's.
            let within = base.map_or(0..u64::MAX, |b| b..b + (1 << 32));
            let mut hits: Vec<(u64, barsearch::Hit)> = Vec::new();
            mem.for_each_chunk_in(within, 64, |addr, chunk| {
                for h in barsearch::find(chunk, bar, CHUNK_LEN) {
                    let at = addr + h.offset as u64;
                    if hits.len() < KEEP * 4 && !hits.iter().any(|(a, o)| *a == at && o.layout == h.layout && o.running == h.running) {
                        hits.push((at, h));
                    }
                }
            })?;
            let mut report = format!("BAR {bar:?}: numbers in proportion to it (any unit), as bytes, words, floats or running totals\n\n");
            let total = hits.len();
            for (n, (at, h)) in hits.iter().take(KEEP).enumerate() {
                let guest = base
                    .and_then(|b| at.checked_sub(b))
                    .map_or("?".to_string(), |g| format!("{g:#010x}"));
                let _ = writeln!(
                    report,
                    "H{} host {at:#x} guest {guest} {:?}{} {:?}",
                    n + 1,
                    h.layout,
                    if h.running { " running totals" } else { "" },
                    h.values
                );
                let from = at.saturating_sub(CONTEXT);
                if let Ok(bytes) = mem.read(from, (CONTEXT as usize) * 2 + h.layout.width() * bar.len()) {
                    for (i, line) in bytes.chunks(16).enumerate() {
                        let hex: Vec<String> = line.iter().map(|b| format!("{b:02x}")).collect();
                        let _ = writeln!(report, "  {:#x}  {}", from + (i * 16) as u64, hex.join(" "));
                    }
                }
                report.push('\n');
            }
            Ok((report, total))
        });
        self.status = match result {
            Err(e) => format!("find failed: {e}"),
            Ok((report, n)) => match self.write_report("find", &report) {
                Ok(path) => format!("find: {n} hit(s), report in {}", path.display()),
                Err(e) => e,
            },
        };
    }
}

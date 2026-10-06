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
/// The window `locate` shows round a hit.
const LOCATE_BEFORE: u64 = 0x100;
const LOCATE_AFTER: u64 = 0x40;

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

    /// `locate TEXT`: every place the game's memory holds this text, with the bytes around it and where it is in the game's save
    /// block, if inside. For finding out where the game keeps something that is also in the save (a guild card's greeting).
    pub(super) fn run_locate(&mut self, text: &str) {
        if !self.live_connected() {
            return self.status = "locate needs a hunter loaded in the game".into();
        }
        let Some(live) = &self.live else { return };
        let result = ProcMem::open(live.child.id()).and_then(|mem| {
            let base = livemod::find_live_block(&mem, std::slice::from_ref(&self.save.hunter_name))?;
            // the whole process: the game keeps things below the save block as well as above it; and the text in the forms the game
            // may keep it in: plain bytes, and 16-bit letters (big-endian, as the Wii U's own memory is, and little-endian)
            let wide = |big: bool| -> Vec<u8> {
                text.encode_utf16()
                    .flat_map(|u| if big { u.to_be_bytes() } else { u.to_le_bytes() })
                    .collect()
            };
            let forms: [(&str, Vec<u8>); 3] = [
                ("plain bytes", text.as_bytes().to_vec()),
                ("16-bit big-endian", wide(true)),
                ("16-bit little-endian", wide(false)),
            ];
            let mut hits: Vec<(&str, usize, u64)> = Vec::new();
            for (label, pattern) in &forms {
                for at in mem.scan(pattern)?.into_iter().take(KEEP) {
                    hits.push((label, pattern.len(), at));
                }
            }
            let mut report = format!("TEXT {text:?}: the save block starts at {base:x?}\n\n");
            // what the save block itself holds where the save file keeps the guild card (title at 0x7a28, greeting at 0x7ad0)
            if let Some(b) = base {
                for (what, off, len) in [("title and name", 0x7a28u64, 0x30usize), ("greeting", 0x7ad0, 0x30)] {
                    if let Ok(bytes) = mem.read(b + off, len) {
                        let hex: Vec<String> = bytes.iter().map(|x| format!("{x:02x}")).collect();
                        let _ = writeln!(report, "block {what} at {off:#x}: {}", hex.join(" "));
                    }
                }
                report.push('\n');
            }
            for (n, (label, len, at)) in hits.iter().enumerate() {
                let place = match base.map(|b| *at as i128 - b as i128) {
                    Some(d) if (0..mh3u_core::save::SAVE_LEN as i128).contains(&d) => format!("INSIDE the save block at offset {d:#x}"),
                    Some(d) if d < 0 => format!("{:#x} BEFORE the save block", -d),
                    Some(d) => format!("{d:#x} after the start of the save block"),
                    None => "?".to_string(),
                };
                let _ = writeln!(report, "H{} {label} host {at:#x}: {place}", n + 1);
                // a wide window: what comes before a text shows what kind of record it is in
                let from = at.saturating_sub(LOCATE_BEFORE);
                if let Ok(bytes) = mem.read(from, (LOCATE_BEFORE + LOCATE_AFTER) as usize + len) {
                    for (i, line) in bytes.chunks(16).enumerate() {
                        let hex: Vec<String> = line.iter().map(|b| format!("{b:02x}")).collect();
                        let _ = writeln!(report, "  {:#x}  {}", from + (i * 16) as u64, hex.join(" "));
                    }
                }
                report.push('\n');
            }
            Ok((report, hits.len()))
        });
        self.status = match result {
            Err(e) => format!("locate failed: {e}"),
            Ok((report, n)) => match self.write_report("locate", &report) {
                Ok(path) => format!("locate: {n} place(s), report in {}", path.display()),
                Err(e) => e,
            },
        };
    }
}

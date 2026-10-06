//! Live save data from a running emulator that we started.
//!
//! While a hunter is loaded, the game keeps its working copy of the save data in memory in the same layout as the `user1`
//! file, and updates it as you play, before anything is saved. Cemu keeps guest memory in guest byte order, so
//! `Save::parse` reads it unchanged. The game also holds an as-loaded copy and some UI state with the same layout, so a
//! candidate is only accepted if its header has the live object's self-pointers.
//!
//! Layout of a live block's header (host address `B`, guest address `G = B - 0xc - base` where `base` is where guest
//! memory starts in the host): u32 at 0x08 is a guest pointer `P`, u32 at 0x24 is `P + 0x34`, and the hunter's name
//! starts at 0x2b.

#[cfg(feature = "edit")]
use crate::edit::Patch;
use crate::procmem::ProcMem;
use crate::save::SAVE_LEN;
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const PROBE_LEN: usize = 0x30;
/// Where the hunter's name starts in a block. The three bytes before it differ between hunters (`00 01 00` for one,
/// `00 00 00` for another), so the search is for the name alone.
const NAME_OFFSET: u64 = 0x2b;

fn be32(d: &[u8], o: usize) -> u64 {
    u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]) as u64
}

/// If the first bytes of a block at host address `addr` are the live save object's header, return the host address where
/// guest memory starts.
pub fn validate_block(head: &[u8], addr: u64) -> Option<u64> {
    if head.len() < PROBE_LEN {
        return None;
    }
    let (ptr, ptr2) = (be32(head, 0x08), be32(head, 0x24));
    if ptr == 0 || ptr2 != ptr + 0x34 {
        return None;
    }
    let base = addr.checked_sub(0xc)?.checked_sub(ptr)?;
    (base % 0x1_0000 == 0).then_some(base)
}

/// Search the emulator's memory for the live save block of one of the hunters in `names`. Returns its host address.
pub fn find_live_block(mem: &ProcMem, names: &[String]) -> io::Result<Option<u64>> {
    for name in names.iter().filter(|n| !n.is_empty()) {
        let mut pattern = name.as_bytes().to_vec();
        pattern.push(0);
        for hit in mem.scan(&pattern)? {
            let Some(block) = hit.checked_sub(NAME_OFFSET) else {
                continue;
            };
            if mem.read(block, PROBE_LEN).is_ok_and(|head| validate_block(&head, block).is_some()) {
                return Ok(Some(block));
            }
        }
    }
    Ok(None)
}

/// Why a search found nothing, as text for a log: every place a hunter name appears in memory, with the header there and
/// whether it passed. Used when `MH3U_LIVE_LOG` names a file.
fn diagnose(mem: &ProcMem, names: &[String]) -> String {
    let mut out = String::new();
    for name in names.iter().filter(|n| !n.is_empty()) {
        let mut pattern = name.as_bytes().to_vec();
        pattern.push(0);
        match mem.scan(&pattern) {
            Err(e) => out += &format!("{name}: scan failed: {e}\n"),
            Ok(hits) => {
                out += &format!("{name}: {} hit(s)\n", hits.len());
                for hit in hits.iter().take(20) {
                    let block = hit.saturating_sub(NAME_OFFSET);
                    match mem.read(block, PROBE_LEN) {
                        Ok(head) => {
                            let hex: String = head.iter().map(|b| format!("{b:02x}")).collect();
                            out += &format!("  {block:#x} {} {hex}\n", validate_block(&head, block).is_some());
                        }
                        Err(e) => out += &format!("  {block:#x} unreadable: {e}\n"),
                    }
                }
            }
        }
    }
    out
}

pub enum LiveEvent {
    /// The live block was found at this host address.
    Connected(u64),
    /// The live save data, sent when it first appears and whenever it changes.
    Save(Vec<u8>),
    /// The block went away (the game returned to the title screen or exited).
    Lost,
    /// A requested write did not happen (see `LiveReader::write`).
    #[cfg(feature = "edit")]
    WriteFailed(String),
}

/// A background thread that finds the live block and reports changes. Dropping it stops the thread.
pub struct LiveReader {
    pub events: mpsc::Receiver<LiveEvent>,
    #[cfg(feature = "edit")]
    writes: mpsc::Sender<Patch>,
    stop: Arc<AtomicBool>,
}

#[cfg(feature = "edit")]
impl LiveReader {
    /// Ask for bytes of the live save block to be overwritten (debug editing). The memory must have been opened with
    /// `ProcMem::open_writable`; if the write cannot be done a `WriteFailed` event follows. The new data comes back as a
    /// `Save` event like any other change.
    pub fn write(&self, patch: Patch) {
        let _ = self.writes.send(patch);
    }
}

impl Drop for LiveReader {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Start watching. `names` are the hunter names to look for (from the save files); searching memory takes about a
/// second, so it happens on the thread and is repeated every few seconds until a block is found.
pub fn spawn(mut mem: ProcMem, names: Vec<String>) -> LiveReader {
    let (tx, events) = mpsc::channel();
    #[cfg(feature = "edit")]
    let (writes, pending_writes) = mpsc::channel::<Patch>();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    thread::spawn(move || {
        let mut block: Option<u64> = None;
        let mut last: Option<Vec<u8>> = None;
        let mut last_search = Instant::now() - Duration::from_secs(60);
        while !flag.load(Ordering::Relaxed) {
            #[cfg(feature = "edit")]
            {
                while let Ok(patch) = pending_writes.try_recv() {
                    let result = match block {
                        None => Err("the game's save data has not been found yet".to_string()),
                        Some(_) if patch.offset + patch.bytes.len() > SAVE_LEN => Err("write outside the save block".to_string()),
                        Some(addr) => mem.write(addr + patch.offset as u64, &patch.bytes).map_err(|e| e.to_string()),
                    };
                    if let Err(why) = result
                        && tx.send(LiveEvent::WriteFailed(why)).is_err()
                    {
                        return;
                    }
                }
            }
            match block {
                Some(addr) => {
                    let data = mem.read(addr, SAVE_LEN);
                    match data {
                        Ok(d) if validate_block(&d, addr).is_some() => {
                            if last.as_ref() != Some(&d) && tx.send(LiveEvent::Save(d.clone())).is_err() {
                                return;
                            }
                            last = Some(d);
                        }
                        _ => {
                            block = None;
                            last = None;
                            if tx.send(LiveEvent::Lost).is_err() {
                                return;
                            }
                        }
                    }
                }
                None if last_search.elapsed() >= Duration::from_secs(2) => {
                    last_search = Instant::now();
                    // The program may have replaced itself since we opened its memory (launcher scripts do).
                    let _ = mem.reopen();
                    if let Ok(Some(addr)) = find_live_block(&mem, &names) {
                        block = Some(addr);
                        if tx.send(LiveEvent::Connected(addr)).is_err() {
                            return;
                        }
                    } else if let Some(path) = std::env::var_os("MH3U_LIVE_LOG") {
                        let text = format!("-- search at {:?}\n{}", Instant::now(), diagnose(&mem, &names));
                        let _ = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .and_then(|mut f| std::io::Write::write_all(&mut f, text.as_bytes()));
                    }
                }
                None => {}
            }
            thread::sleep(Duration::from_millis(250));
        }
    });
    LiveReader {
        events,
        #[cfg(feature = "edit")]
        writes,
        stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    /// The hunter name stored in a capture, so tests need not spell out a real one.
    fn hunter_name(block: &[u8]) -> String {
        crate::save::Save::parse(block).unwrap().hunter_name
    }

    #[test]
    fn accepts_the_live_block_and_rejects_the_others() {
        // Host address the live block had in the capture; the other two blocks (as-loaded copy, the file) must not match.
        let live = fixture!("05-live/live_block.bin");
        assert_eq!(validate_block(&live, 0x7542be62cb90), Some(0x754290000000));
        assert_eq!(validate_block(&live, 0x7542be62cb91), None); // misaligned base
        assert_eq!(validate_block(&fixture!("05-live/loaded_block.bin"), 0x7542be693a04), None);
        let file = fixture!("04-worntester/user1");
        assert_eq!(validate_block(&file, 0x1000), None);
        assert_eq!(validate_block(&live[..10], 0x7542be62cb90), None);
    }

    #[test]
    fn live_block_parses_like_a_save_file() {
        let live = crate::save::Save::parse(&fixture!("05-live/live_block.bin")).unwrap();
        assert!(!live.hunter_name.is_empty());
        // The capture was taken right after moving 3 Honey from the box to the pouch, before saving.
        assert!(live.pouch.iter().any(|s| s.id == 175 && s.count == 3));
        assert!(live.item_box.iter().any(|s| s.id == 175 && s.count == 8));
    }

    /// Finds the block in another process's memory, using a Python child that places the capture at a host address
    /// with the alignment the finder expects.
    #[test]
    fn finds_the_block_in_a_child_process() {
        let name = hunter_name(&fixture!("05-live/live_block.bin")); // also skips without the snapshot
        let script = r#"
import ctypes, sys, time
data = open(sys.argv[1], 'rb').read()
buf = (ctypes.c_char * (4 << 20))()
addr = ctypes.addressof(buf)
want = 0xcb90                      # block start must be congruent to this mod 0x10000
off = ((want - addr) % 0x10000) + 0x10000
ctypes.memmove(addr + off, data, len(data))
print(hex(addr + off), flush=True)
time.sleep(30)
"#;
        let Ok(mut child) = Command::new("python3")
            .args([
                "-c",
                script,
                &format!("{}/../../snapshots/05-live/live_block.bin", env!("CARGO_MANIFEST_DIR")),
            ])
            .stdout(Stdio::piped())
            .spawn()
        else {
            return; // no python3 here
        };
        let mut line = String::new();
        std::io::BufRead::read_line(&mut std::io::BufReader::new(child.stdout.take().unwrap()), &mut line).unwrap();
        let expected = u64::from_str_radix(line.trim().trim_start_matches("0x"), 16).unwrap();
        let mem = ProcMem::open(child.id()).unwrap();
        let found = find_live_block(&mem, std::slice::from_ref(&name)).unwrap();
        assert_eq!(found, Some(expected));
        assert_eq!(find_live_block(&mem, &["Nobody".to_string()]).unwrap(), None);
        child.kill().unwrap();
    }

    /// The whole background reader: it finds the block, reports it, and reports a later change.
    #[test]
    fn reader_reports_the_block_and_a_later_change() {
        let name = hunter_name(&fixture!("05-live/live_block.bin")); // also skips without the snapshot
        let script = r#"
import ctypes, sys, time
data = bytearray(open(sys.argv[1], 'rb').read())
buf = (ctypes.c_char * (4 << 20))()
addr = ctypes.addressof(buf)
off = ((0xcb90 - addr) % 0x10000) + 0x10000
ctypes.memmove(addr + off, bytes(data), len(data))
print('ready', flush=True)
time.sleep(4)
data[0xd4:0xd8] = (176).to_bytes(2, 'big') + (7).to_bytes(2, 'big')   # pouch slot 1 becomes 7 x Herb
ctypes.memmove(addr + off, bytes(data), len(data))
time.sleep(30)
"#;
        let Ok(mut child) = Command::new("python3")
            .args([
                "-c",
                script,
                &format!("{}/../../snapshots/05-live/live_block.bin", env!("CARGO_MANIFEST_DIR")),
            ])
            .stdout(Stdio::piped())
            .spawn()
        else {
            return;
        };
        std::io::BufRead::read_line(&mut std::io::BufReader::new(child.stdout.take().unwrap()), &mut String::new()).unwrap();
        let reader = spawn(ProcMem::open(child.id()).unwrap(), vec![name.clone()]);
        let wait = |reader: &LiveReader| reader.events.recv_timeout(Duration::from_secs(20)).expect("no event in 20 s");
        assert!(matches!(wait(&reader), LiveEvent::Connected(_)));
        let LiveEvent::Save(first) = wait(&reader) else {
            panic!("expected the save data")
        };
        let LiveEvent::Save(second) = wait(&reader) else {
            panic!("expected the changed data")
        };
        let pouch = |d: &[u8]| crate::save::Save::parse(d).unwrap().pouch;
        assert!(pouch(&first).iter().any(|s| s.id == 175 && s.count == 3));
        assert!(pouch(&second).iter().any(|s| s.id == 176 && s.count == 7));
        drop(reader);
        child.kill().unwrap();
    }

    #[cfg(feature = "edit")]
    /// Debug editing: a write request reaches the other process's memory and comes back as changed save data, and a
    /// read-only handle is refused.
    #[test]
    fn writes_go_through_the_reader_when_the_memory_is_writable() {
        let name = hunter_name(&fixture!("05-live/live_block.bin")); // also skips without the snapshot
        let script = r#"
import ctypes, sys, time
data = bytearray(open(sys.argv[1], 'rb').read())
buf = (ctypes.c_char * (4 << 20))()
addr = ctypes.addressof(buf)
off = ((0xcb90 - addr) % 0x10000) + 0x10000
ctypes.memmove(addr + off, bytes(data), len(data))
print('ready', flush=True)
time.sleep(60)
"#;
        let start = || {
            let mut child = Command::new("python3")
                .args([
                    "-c",
                    script,
                    &format!("{}/../../snapshots/05-live/live_block.bin", env!("CARGO_MANIFEST_DIR")),
                ])
                .stdout(Stdio::piped())
                .spawn()
                .ok()?;
            std::io::BufRead::read_line(&mut std::io::BufReader::new(child.stdout.take().unwrap()), &mut String::new()).unwrap();
            Some(child)
        };
        let Some(mut child) = start() else { return };
        let wait = |reader: &LiveReader| reader.events.recv_timeout(Duration::from_secs(20)).expect("no event in 20 s");
        let zenny = |d: &[u8]| crate::save::Save::parse(d).unwrap().zenny;

        // writable: the change is applied and read back
        let reader = spawn(ProcMem::open_writable(child.id()).unwrap(), vec![name.clone()]);
        assert!(matches!(wait(&reader), LiveEvent::Connected(_)));
        let LiveEvent::Save(before) = wait(&reader) else {
            panic!("expected the save data")
        };
        assert_eq!(zenny(&before), 1500);
        reader.write(crate::edit::set_zenny(54_321));
        let LiveEvent::Save(after) = wait(&reader) else {
            panic!("expected the edited data")
        };
        assert_eq!(zenny(&after), 54_321);
        reader.write(Patch {
            offset: SAVE_LEN - 1,
            bytes: vec![0, 0],
        });
        assert!(matches!(wait(&reader), LiveEvent::WriteFailed(why) if why.contains("outside")));
        drop(reader);

        // read-only: refused, and the game's data is untouched
        let reader = spawn(ProcMem::open(child.id()).unwrap(), vec![name.clone()]);
        assert!(matches!(wait(&reader), LiveEvent::Connected(_)));
        let LiveEvent::Save(_) = wait(&reader) else {
            panic!("expected the save data")
        };
        reader.write(crate::edit::set_zenny(1));
        assert!(matches!(wait(&reader), LiveEvent::WriteFailed(why) if why.contains("read-only")));
        child.kill().unwrap();
    }
}

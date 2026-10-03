//! Find the game's in-memory copy of the save data inside a running emulator.
//!
//! Cemu keeps guest memory in the guest's own (big-endian) byte order, so a save block in memory looks like the `user1`
//! file. We find candidates by the header's fixed fields and then check a few more bytes.

use crate::{procmem::ProcMem, save::SAVE_LEN};
use std::io;

/// Offset in the block of the length fields (`0x0c`, `0x8a00`) that mark a hunter save.
const SIGNATURE_OFFSET: usize = 0x18;
const SIGNATURE: [u8; 8] = [0, 0, 0, 0x0c, 0, 0, 0x8a, 0];

/// Host addresses where a save block (`SAVE_LEN` bytes) appears to start.
pub fn find_save_blocks(mem: &ProcMem) -> io::Result<Vec<u64>> {
    let mut blocks = Vec::new();
    for hit in mem.scan(&SIGNATURE)? {
        let Some(base) = hit.checked_sub(SIGNATURE_OFFSET as u64) else {
            continue;
        };
        let Ok(head) = mem.read(base, 0x30) else { continue };
        if looks_like_header(&head) {
            blocks.push(base);
        }
    }
    Ok(blocks)
}

/// Check the fixed bytes at the start of a hunter save block.
pub fn looks_like_header(head: &[u8]) -> bool {
    head.len() >= 0x30
        && head[..0x0b].iter().all(|&b| b == 0)
        && head[0x0b] == 4
        && head[0x13] == 0x14
        && head[SIGNATURE_OFFSET..SIGNATURE_OFFSET + 8] == SIGNATURE
}

/// Read one whole save block.
pub fn read_block(mem: &ProcMem, base: u64) -> io::Result<Vec<u8>> {
    mem.read(base, SAVE_LEN)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_a_real_save_header() {
        let file = fixture!("04-worntester/user2");
        assert!(looks_like_header(&file));
        assert!(!looks_like_header(&file[1..]));
        // The other save files (`system`) have a different header and must not match.
        let system = fixture!("04-worntester/system");
        assert!(!looks_like_header(&system));
    }
}

//! Wii U `.rpx` executables: a big-endian ELF32 whose sections may be zlib-compressed.

use anyhow::{Context, Result, bail};
use flate2::read::ZlibDecoder;
use std::io::Read;

/// Section flag marking zlib compression; the stream is preceded by the u32 uncompressed size.
const SHF_RPL_ZLIB: u32 = 0x0800_0000;
const SHT_NOBITS: u32 = 8;

fn be16(d: &[u8], o: usize) -> usize {
    u16::from_be_bytes([d[o], d[o + 1]]) as usize
}

fn be32(d: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]])
}

/// Return the decompressed contents of the section loaded at virtual address `addr`.
pub fn section_at(rpx: &[u8], addr: u32) -> Result<Vec<u8>> {
    if rpx.len() < 0x34 || &rpx[..4] != b"\x7fELF" {
        bail!("not an ELF/RPX file");
    }
    let table = be32(rpx, 0x20) as usize;
    let entry_len = be16(rpx, 0x2e);
    let count = be16(rpx, 0x30);
    for i in 0..count {
        let h = table + i * entry_len;
        let header = rpx.get(h..h + 40).context("truncated section table")?;
        let (kind, flags, sh_addr, offset, size) = (
            be32(header, 4),
            be32(header, 8),
            be32(header, 12),
            be32(header, 16) as usize,
            be32(header, 20) as usize,
        );
        if sh_addr != addr || kind == SHT_NOBITS {
            continue;
        }
        let raw = rpx.get(offset..offset + size).context("section data out of bounds")?;
        if flags & SHF_RPL_ZLIB == 0 {
            return Ok(raw.to_vec());
        }
        let mut out = Vec::with_capacity(be32(raw, 0) as usize);
        ZlibDecoder::new(&raw[4..]).read_to_end(&mut out).context("inflating section")?;
        return Ok(out);
    }
    bail!("no section at address {addr:#x}")
}

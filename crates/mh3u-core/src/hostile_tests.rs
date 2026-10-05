//! Files the program reads are not always files it can trust: a game dump may be damaged, and a save or a settings file may come from
//! somebody else. These feed the parsers files that are made up, cut short and scrambled, and want an error or a result, never a panic.
//! They build their own files, so they run everywhere; the ones that also use the player's dump skip without it.

use crate::{arc::Arc, gmd, rpx, save::Save};
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// A copy of `base` cut short, or with a few bytes or bits changed, or a whole number set to an extreme.
fn scramble(rng: &mut Rng, base: &[u8], keep_len: bool) -> Vec<u8> {
    let mut d = base.to_vec();
    if d.is_empty() {
        return d;
    }
    match rng.below(4) {
        0 if !keep_len => d.truncate(rng.below(d.len())),
        1 => {
            for _ in 0..1 + rng.below(8) {
                let i = rng.below(d.len());
                d[i] = rng.next() as u8;
            }
        }
        2 if d.len() >= 4 => {
            let i = rng.below(d.len() - 3);
            let v = [0u32, 1, 0x7fff_ffff, 0xffff_ffff, 0x8000_0000, 0x1fff_ffff][rng.below(6)];
            d[i..i + 4].copy_from_slice(&v.to_be_bytes());
        }
        _ => {
            for _ in 0..1 + rng.below(40) {
                let i = rng.below(d.len());
                d[i] ^= 1 << rng.below(8);
            }
        }
    }
    d
}

/// Run `f` on `rounds` scrambled copies of `base`; the inputs that made it panic.
fn panics(base: &[u8], rounds: usize, keep_len: bool, f: impl Fn(&[u8])) -> Vec<Vec<u8>> {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut bad = Vec::new();
    for _ in 0..rounds {
        let d = scramble(&mut rng, base, keep_len);
        if catch_unwind(AssertUnwindSafe(|| f(&d))).is_err() {
            bad.push(d);
        }
    }
    bad
}

fn zlib(data: &[u8]) -> Vec<u8> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

/// An executable with one compressed section at `addr` holding `content`.
fn rpx_with(addr: u32, content: &[u8]) -> Vec<u8> {
    let mut packed = (content.len() as u32).to_be_bytes().to_vec();
    packed.extend(zlib(content));
    let mut d = vec![0u8; 0x40 + 40];
    d[..4].copy_from_slice(b"\x7fELF");
    d[0x20..0x24].copy_from_slice(&0x40u32.to_be_bytes());
    d[0x2e..0x30].copy_from_slice(&40u16.to_be_bytes());
    d[0x30..0x32].copy_from_slice(&1u16.to_be_bytes());
    d[0x44..0x48].copy_from_slice(&1u32.to_be_bytes());
    d[0x48..0x4c].copy_from_slice(&0x0800_0000u32.to_be_bytes());
    d[0x4c..0x50].copy_from_slice(&addr.to_be_bytes());
    d[0x50..0x54].copy_from_slice(&((0x40 + 40) as u32).to_be_bytes());
    d[0x54..0x58].copy_from_slice(&(packed.len() as u32).to_be_bytes());
    d.extend(packed);
    d
}

/// An archive with one entry, `name`, holding `content`.
fn arc_with(name: &str, content: &[u8]) -> Vec<u8> {
    let packed = zlib(content);
    let mut d = vec![0u8; 12 + 80];
    d[..4].copy_from_slice(b"\0CRA");
    d[6..8].copy_from_slice(&1u16.to_be_bytes());
    d[12..12 + name.len()].copy_from_slice(name.as_bytes());
    d[12 + 64..12 + 68].copy_from_slice(&0x1234u32.to_be_bytes());
    d[12 + 68..12 + 72].copy_from_slice(&(packed.len() as u32).to_be_bytes());
    d[12 + 72..12 + 76].copy_from_slice(&(content.len() as u32).to_be_bytes());
    d[12 + 76..12 + 80].copy_from_slice(&92u32.to_be_bytes());
    d.extend(packed);
    d
}

/// A text table with these strings.
fn gmd_with(strings: &[&str]) -> Vec<u8> {
    let body: Vec<u8> = strings.iter().flat_map(|s| s.bytes().chain([0])).collect();
    let mut d = vec![0u8; 0x24];
    d[..4].copy_from_slice(b"\0DMG");
    d[0x18..0x1c].copy_from_slice(&(strings.len() as u32).to_be_bytes());
    d[0x20..0x24].copy_from_slice(&(body.len() as u32).to_be_bytes());
    d.extend(body);
    d
}

#[test]
fn the_made_up_files_are_valid_to_begin_with() {
    let content: Vec<u8> = (0..3000u32).map(|i| (i % 200) as u8).collect();
    assert_eq!(rpx::section_at(&rpx_with(0x1010_a000, &content), 0x1010_a000).unwrap(), content);
    let archive = arc_with("GUI\\\\font\\\\Item00_eng", &content);
    let arc = Arc::parse(&archive).unwrap();
    assert_eq!(arc.read(&arc.entries[0]).unwrap(), content);
    assert_eq!(gmd::parse(&gmd_with(&["Potion", "Herb"])).unwrap(), ["Potion", "Herb"]);
}

#[test]
fn a_scrambled_or_cut_short_executable_is_an_error_not_a_panic() {
    let base = rpx_with(0x1010_a000, &vec![7u8; 4000]);
    let bad = panics(&base, 3000, false, |d| {
        let _ = rpx::section_at(d, 0x1010_a000);
    });
    assert!(
        bad.is_empty(),
        "{} inputs panicked, the first: {:02x?}",
        bad.len(),
        bad.first().map(|d| &d[..d.len().min(100)])
    );
    // cut at every length
    for len in 0..base.len() {
        assert!(catch_unwind(|| rpx::section_at(&base[..len], 0x1010_a000)).is_ok(), "cut at {len}");
    }
}

#[test]
fn a_compressed_section_that_is_too_short_to_hold_its_size_is_refused() {
    let mut d = rpx_with(0x1010_a000, b"x");
    // the section header says the section is 2 bytes long
    d[0x54..0x58].copy_from_slice(&2u32.to_be_bytes());
    let err = rpx::section_at(&d, 0x1010_a000).unwrap_err();
    assert!(
        format!("{err:#}").contains("inflating section") || format!("{err:#}").contains("too short"),
        "{err:#}"
    );
}

#[test]
fn a_section_that_claims_gigabytes_is_refused_not_allocated() {
    let mut d = rpx_with(0x1010_a000, b"hello");
    // the uncompressed size at the start of the section says 4 GiB - 1
    d[0x68..0x6c].copy_from_slice(&u32::MAX.to_be_bytes());
    let err = format!("{:#}", rpx::section_at(&d, 0x1010_a000).unwrap_err());
    assert!(err.contains("allowed"), "{err}");
}

#[test]
fn a_scrambled_archive_is_an_error_not_a_panic() {
    let base = arc_with("GUI\\\\font\\\\Item00_eng", &[3u8; 2000]);
    let bad = panics(&base, 3000, false, |d| {
        if let Ok(arc) = Arc::parse(d) {
            for e in &arc.entries {
                let _ = arc.read(e);
            }
        }
    });
    assert!(bad.is_empty(), "{} inputs panicked", bad.len());
}

#[test]
fn an_archive_entry_that_unpacks_to_more_than_it_says_is_refused() {
    let mut d = arc_with("big", &vec![0u8; 8 << 20]);
    // it says 100 bytes
    d[12 + 72..12 + 76].copy_from_slice(&100u32.to_be_bytes());
    let arc = Arc::parse(&d).unwrap();
    let err = format!("{:#}", arc.read(&arc.entries[0]).unwrap_err());
    assert!(err.contains("more than the 100 bytes"), "{err}");
}

#[test]
fn a_scrambled_text_table_is_an_error_not_a_panic() {
    let base = gmd_with(&["Potion", "Mega Potion", "Herb", "Honey", ""]);
    let bad = panics(&base, 3000, false, |d| {
        let _ = gmd::parse(d);
    });
    assert!(bad.is_empty(), "{} inputs panicked", bad.len());
}

#[test]
fn a_save_with_any_contents_parses_or_is_refused() {
    let mut base = vec![0u8; crate::save::SAVE_LEN];
    base[0x2b..0x2b + 5].copy_from_slice(b"Hunty");
    let bad = panics(&base, 1500, true, |d| {
        let _ = Save::parse(d);
    });
    assert!(bad.is_empty(), "{} inputs panicked", bad.len());
    for len in [0, 1, 100, crate::save::SAVE_LEN - 1, crate::save::SAVE_LEN + 1] {
        assert!(Save::parse(&vec![0u8; len]).is_err(), "{len}");
    }
    // a save of all 0xff, all items and ids at their largest
    assert!(catch_unwind(|| Save::parse(&vec![0xffu8; crate::save::SAVE_LEN])).is_ok());
}

/// The same with the player's own files, when there is a dump (skips without one).
#[test]
fn the_players_own_files_scrambled_are_errors_not_panics() {
    let Some(dump) = crate::gamedata::dump_from_env() else { return };
    let quests = dump.join("content/nativeCafe/quest/us");
    let mut files: Vec<_> = std::fs::read_dir(&quests)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    files.sort();
    for path in files.iter().take(4) {
        let base = std::fs::read(path).unwrap();
        let bad = panics(&base, 400, false, |d| {
            let _ = crate::quest::parse(d);
        });
        assert!(bad.is_empty(), "{}: {} inputs panicked", path.display(), bad.len());
    }
    let archive = std::fs::read(dump.join("content/nativeCafe/arc/ID/ID_arena_eng.arc")).unwrap();
    let bad = panics(&archive[..archive.len().min(100_000)], 200, false, |d| {
        if let Ok(arc) = Arc::parse(d) {
            for e in arc.entries.iter().take(3) {
                let _ = arc.read(e);
            }
        }
    });
    assert!(bad.is_empty(), "{} archives panicked", bad.len());
}

//! Monster hit zones, read from `enemy\emNNN\em_status00` inside each monster's archive (`arc/enemy/emNNN.arc`, NNN = the
//! monster's id in the name table).
//!
//! The file is a "SME" block with a header of 0x80+ bytes, then tables of 10-byte rows, one row per hit zone: eight
//! effectiveness values (cut, impact, shot, fire, water, ice, thunder, dragon, each 0-180 percent), a dizzy byte and a fixed 0x64.
//! Where the first table starts differs per monster (0xD0, 0x110, ...), so rows are found by their shape: a run of rows ending in
//! 0x64, with a non-zero start, followed by filler rows that are all 0x64. The first table is the monster's normal state; later
//! tables (an enraged or broken state) are not read. Checked against Kiranico's hit-zone numbers on 49 monsters: 319 of the 432
//! zones it lists are in the first table (the rest belong to other states). The game's data carries no names for the zones.

/// How much damage of each kind a zone takes, in percent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zone {
    pub cut: u8,
    pub impact: u8,
    pub shot: u8,
    pub fire: u8,
    pub water: u8,
    pub ice: u8,
    pub thunder: u8,
    pub dragon: u8,
}

impl Zone {
    /// The three weapon damage types, in order cut, impact, shot.
    pub fn physical(&self) -> [u8; 3] {
        [self.cut, self.impact, self.shot]
    }

    /// The five elements, in order fire, water, ice, thunder, dragon.
    pub fn elements(&self) -> [u8; 5] {
        [self.fire, self.water, self.ice, self.thunder, self.dragon]
    }
}

const ROW: usize = 10;
/// Search from here: the header before it holds look-alike bytes.
const SEARCH_FROM: usize = 0xc0;

fn zone_at(d: &[u8], o: usize) -> Option<Zone> {
    let r = d.get(o..o + ROW)?;
    let shaped = r[9] == 0x64 && matches!(r[8], 0 | 0x64) && r[..8].iter().max().is_some_and(|&m| m <= 0xb4);
    // filler rows are all 0x64; a real zone never starts with a zero (the header's look-alikes do)
    if !shaped || r[..8].iter().all(|&b| b == 0x64) || r[..3].contains(&0) {
        return None;
    }
    Some(Zone {
        cut: r[0],
        impact: r[1],
        shot: r[2],
        fire: r[3],
        water: r[4],
        ice: r[5],
        thunder: r[6],
        dragon: r[7],
    })
}

/// The hit zones of the monster's normal state, in the game's order; empty when the file has no recognizable table.
pub fn parse(data: &[u8]) -> Vec<Zone> {
    let mut o = SEARCH_FROM;
    while o + ROW <= data.len() {
        if zone_at(data, o).is_some() {
            let mut zones = Vec::new();
            while let Some(z) = zone_at(data, o + zones.len() * ROW) {
                zones.push(z);
            }
            return zones;
        }
        o += 1;
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: &[[u8; 8]], filler: usize) -> Vec<u8> {
        let mut d = vec![0u8; 0xd0];
        // header look-alike: alternating zero and 0x64
        d[0x90..0xa0].copy_from_slice(&[0, 0x64].repeat(8));
        for r in rows {
            d.extend_from_slice(r);
            d.extend_from_slice(&[0, 0x64]);
        }
        d.extend(std::iter::repeat_n(0x64, filler * ROW));
        d.extend_from_slice(&[1, 0, 0, 0]);
        d
    }

    #[test]
    fn reads_the_rows_up_to_the_filler() {
        let d = table(&[[0x5a, 0x50, 0x46, 0, 15, 15, 20, 35], [0x28, 0x28, 0x28, 0, 10, 10, 15, 20]], 3);
        let z = parse(&d);
        assert_eq!(z.len(), 2);
        assert_eq!(z[0].physical(), [0x5a, 0x50, 0x46]);
        assert_eq!(z[0].elements(), [0, 15, 15, 20, 35]);
        assert_eq!(z[1].cut, 0x28);
    }

    #[test]
    fn a_file_without_a_table_gives_none() {
        assert!(parse(&[0u8; 0x200]).is_empty());
        assert!(parse(&[]).is_empty());
    }
}

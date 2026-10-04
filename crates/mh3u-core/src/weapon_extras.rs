//! Sharpness and element (or status) of the melee weapons.
//!
//! The game keeps neither where this program can read it (see `docs/formats.md`: the two record bytes that select a bar are known, the
//! bars are not), so this is data taken from Kiranico's Monster Hunter 3 Ultimate database: `data/weapon_extras.tsv`, one line per weapon
//! with the game's weapon kind and id. The database's ids differ from the game's, so the lines were joined to the game's weapons by name
//! (1111 of its 1119 melee weapons; the rest are DLC weapons the game's text does not name). On load, a line whose name is not the
//! name the game has for that kind and id is dropped, so a different version of the game never shows another weapon's numbers.

use anyhow::{Context, Result, bail};
use std::collections::HashMap;

const TABLE: &str = include_str!("../data/weapon_extras.tsv");

/// The colors of a sharpness bar, red first. The last (purple) is only reached with Sharpness +1.
pub const COLORS: usize = 7;

/// A sharpness bar: how many points each color holds.
pub type Bar = [u8; COLORS];

/// An element or status on a weapon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Special {
    pub name: String,
    pub value: u16,
    /// Hidden until the Awaken skill is on.
    pub hidden: bool,
}

/// What is known of one weapon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extras {
    pub sharpness: Bar,
    /// The same bar with Sharpness +1.
    pub plus: Bar,
    pub specials: Vec<Special>,
}

/// The table as (kind, id) -> (name, extras).
pub fn parse() -> Result<HashMap<(u8, u16), (String, Extras)>> {
    parse_text(TABLE)
}

fn bar(text: &str) -> Result<Bar> {
    let values: Vec<u8> = text
        .split(',')
        .map(|v| v.parse().context("sharpness number"))
        .collect::<Result<_>>()?;
    values
        .try_into()
        .map_err(|_| anyhow::anyhow!("a sharpness bar has {COLORS} numbers"))
}

fn parse_text(text: &str) -> Result<HashMap<(u8, u16), (String, Extras)>> {
    let mut out = HashMap::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 6 {
            bail!("weapon_extras.tsv: a line has {} fields, not 6: {line}", f.len());
        }
        let specials = f[5]
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| {
                let p: Vec<&str> = s.split(':').collect();
                match p[..] {
                    [name, value, hidden] => Ok(Special {
                        name: name.to_string(),
                        value: value.parse()?,
                        hidden: hidden == "1",
                    }),
                    _ => bail!("weapon_extras.tsv: bad element {s}"),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        out.insert(
            (f[0].parse()?, f[1].parse()?),
            (
                f[2].to_string(),
                Extras {
                    sharpness: bar(f[3])?,
                    plus: bar(f[4])?,
                    specials,
                },
            ),
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_reads_and_every_bar_is_in_order() {
        let t = parse().unwrap();
        assert!(t.len() > 1000);
        let iron = &t[&(7, 1)];
        assert_eq!(iron.0, "Iron Sword");
        assert_eq!(iron.1.sharpness, [20, 9, 4, 0, 0, 0, 0]);
        assert_eq!(iron.1.plus, [20, 9, 15, 0, 0, 0, 0]);
        assert_eq!(
            iron.1.specials,
            [Special {
                name: "Ice".into(),
                value: 50,
                hidden: true
            }]
        );
        for (key, (name, e)) in &t {
            // a bar fills from red: no color after an empty one, and Sharpness +1 never takes any away
            let filled = |b: &Bar| b.iter().rposition(|&v| v > 0).map_or(0, |p| p + 1);
            assert!(e.sharpness[..filled(&e.sharpness)].iter().all(|&v| v > 0), "{key:?} {name}");
            assert!(e.sharpness.iter().sum::<u8>() <= e.plus.iter().sum::<u8>(), "{key:?} {name}");
        }
    }

    #[test]
    fn a_bad_line_is_an_error() {
        assert!(parse_text("7\t1\tx\t1,2,3\t1,2,3,0,0,0,0\t").is_err(), "a bar needs 7 numbers");
        assert!(parse_text("7\t1\tx").is_err());
    }
}

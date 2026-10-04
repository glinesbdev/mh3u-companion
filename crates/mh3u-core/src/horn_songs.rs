//! The songs a hunting horn can play.
//!
//! A horn has three notes (colors). A song is a sequence of them, three or four long, and which songs a horn can play depends on its set of
//! notes. The game's files were not searched for this table (it is in no text the program reads); `data/horn_songs.tsv` is taken from the
//! Monster Hunter Wiki's MH3U song tables (white, purple and orange note horns), one line per song. See `docs/formats.md`.

use anyhow::{Context, Result, bail};

const TABLE: &str = include_str!("../data/horn_songs.tsv");

/// An effect of a song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect {
    pub name: String,
    /// Only the player who plays it gets it.
    pub self_only: bool,
}

/// A value in seconds and what it is with the Horn Maestro skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seconds {
    pub plain: i16,
    pub maestro: Option<i16>,
}

/// One song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Song {
    /// The notes to play, as colors (white, purple, red, blue, green, yellow, sky, orange).
    pub notes: Vec<String>,
    pub effect: Effect,
    /// The second effect, got by playing the song again while the first lasts.
    pub effect2: Option<Effect>,
    pub duration: Option<Seconds>,
    /// What each replay adds to the time.
    pub extension: Option<Seconds>,
    /// Songs with notes of any horn that has them (a double white or double purple note), instead of one horn's set.
    pub any_horn: bool,
}

fn effect(text: &str) -> Option<Effect> {
    let (name, self_only) = match text.strip_prefix('@') {
        Some(rest) => (rest, true),
        None => (text, false),
    };
    (!name.is_empty()).then(|| Effect {
        name: name.to_string(),
        self_only,
    })
}

fn seconds(text: &str) -> Result<Option<Seconds>> {
    if text.is_empty() {
        return Ok(None);
    }
    let (plain, rest) = text.split_once('(').map_or((text, None), |(p, r)| (p, Some(r)));
    Ok(Some(Seconds {
        plain: plain.trim_start_matches('+').parse().with_context(|| format!("seconds {text}"))?,
        maestro: rest
            .map(|r| {
                r.trim_end_matches(')')
                    .trim_start_matches('+')
                    .parse()
                    .with_context(|| format!("seconds {text}"))
            })
            .transpose()?,
    }))
}

/// The table as (the horn's notes, sorted; or empty for songs any horn can play) and the song.
pub fn parse() -> Result<Vec<(Vec<String>, Song)>> {
    parse_text(TABLE)
}

fn parse_text(text: &str) -> Result<Vec<(Vec<String>, Song)>> {
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 6 {
            bail!("horn_songs.tsv: a line has {} fields, not 6: {line}", f.len());
        }
        let horn: Vec<String> = if f[0] == "any" {
            Vec::new()
        } else {
            f[0].split(',').map(str::to_string).collect()
        };
        out.push((
            horn.clone(),
            Song {
                notes: f[1].split(',').map(str::to_string).collect(),
                effect: effect(f[2]).with_context(|| format!("horn_songs.tsv: a song has no effect: {line}"))?,
                effect2: effect(f[3]),
                duration: seconds(f[4])?,
                extension: seconds(f[5])?,
                any_horn: horn.is_empty(),
            },
        ));
    }
    Ok(out)
}

/// The songs a horn with these notes (in any order) can play.
pub fn songs_for<'a>(table: &'a [(Vec<String>, Song)], notes: &[String]) -> Vec<&'a Song> {
    let mut want: Vec<&str> = notes.iter().map(String::as_str).collect();
    want.sort_unstable();
    table
        .iter()
        .filter(|(horn, song)| {
            if horn.is_empty() {
                song.notes.iter().all(|n| notes.contains(n))
            } else {
                horn.iter().map(String::as_str).eq(want.iter().copied())
            }
        })
        .map(|(_, s)| s)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn notes(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_table_reads_and_a_horn_gets_its_songs() {
        let t = parse().unwrap();
        // a horn with blue, red and white notes (listed in any order): the four songs of the wiki's example, and the double-white song
        let songs = songs_for(&t, &notes(&["red", "blue", "white"]));
        let attack = songs.iter().find(|s| s.effect.name == "Attack Boost (S)").unwrap();
        assert_eq!(attack.notes, notes(&["white", "red", "red"]));
        assert_eq!(attack.effect2.as_ref().unwrap().name, "Attack Boost Bonus");
        assert_eq!(
            attack.duration,
            Some(Seconds {
                plain: 120,
                maestro: Some(150)
            })
        );
        assert_eq!(
            attack.extension,
            Some(Seconds {
                plain: 90,
                maestro: Some(120)
            })
        );
        let health = songs.iter().find(|s| s.effect.name == "Health Boost (S)").unwrap();
        assert_eq!((health.effect2.clone(), health.extension), (None, None));
        let speed = songs.iter().find(|s| s.any_horn).unwrap();
        assert_eq!(speed.notes, notes(&["white", "white"]));
        assert!(speed.effect.self_only);
        assert_eq!(songs.len(), 5);
        // a horn with no white note gets no double-white song
        assert!(
            songs_for(&t, &notes(&["blue", "green", "purple"]))
                .iter()
                .all(|s| s.notes.iter().all(|n| n != "white"))
        );
    }

    #[test]
    fn every_horn_in_the_weapon_table_has_songs() {
        let weapons = crate::weapon_extras::parse().unwrap();
        let t = parse().unwrap();
        let mut sets = 0;
        for (_, (name, e)) in weapons.iter().filter(|(k, _)| k.0 == 19) {
            if let crate::weapon_extras::Part::Notes(n) = &e.part {
                sets += 1;
                assert!(songs_for(&t, n).len() >= 2, "{name} {n:?}");
            }
        }
        assert!(sets > 90);
    }

    #[test]
    fn bad_lines_are_errors() {
        assert!(parse_text("a,b,c\tx").is_err());
        assert!(parse_text("a,b,c\tr,r\t\t\t\t").is_err(), "no effect");
        assert!(parse_text("a,b,c\tr,r\tHeal\t\tsoon\t").is_err());
    }
}

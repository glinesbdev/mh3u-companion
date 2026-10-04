//! Where items can be gathered: mined, picked, caught as bugs or fished.
//!
//! The game keeps this in tables the program does not read yet (a table of pointers sits just before the capture and break lists in the
//! executable's data, see `docs/ideas.md`). `data/gather_spots.tsv` is taken from Kiranico's Monster Hunter 3 Ultimate database and joined
//! to the game's items by name (all 155 items it lists for gathering): one line per item, map, kind of spot and rank, with the areas of
//! the map where the spot is. The database gives no chances, and a spot can be a map's "Secret" area. On load, a line whose name is not
//! the game's name for that item id is dropped (see `GameData::gather_spots`).

use anyhow::{Result, bail};
use std::collections::HashMap;

const TABLE: &str = include_str!("../data/gather_spots.tsv");

/// One place an item can be got by gathering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spot {
    pub map: String,
    /// Gathering, Mining, Bug-Catching or Fishing.
    pub kind: String,
    /// Low, High or G.
    pub rank: String,
    /// The areas of the map, as the database names them ("1", "6", "Secret").
    pub areas: Vec<String>,
}

/// The table by item id: (the item's name, its spots).
pub fn parse() -> Result<HashMap<u16, (String, Vec<Spot>)>> {
    parse_text(TABLE)
}

fn parse_text(text: &str) -> Result<HashMap<u16, (String, Vec<Spot>)>> {
    let mut out: HashMap<u16, (String, Vec<Spot>)> = HashMap::new();
    for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() != 6 {
            bail!("gather_spots.tsv: a line has {} fields, not 6: {line}", f.len());
        }
        let spot = Spot {
            map: f[2].to_string(),
            kind: f[3].to_string(),
            rank: f[4].to_string(),
            areas: f[5].split(',').map(str::to_string).collect(),
        };
        out.entry(f[0].parse()?)
            .or_insert_with(|| (f[1].to_string(), Vec::new()))
            .1
            .push(spot);
    }
    Ok(out)
}

/// A short account of where an item can be got: `Mining at Volcano, Tundra and 2 more maps` (the most common kind first).
pub fn summary(spots: &[Spot]) -> Option<String> {
    let mut kinds: Vec<(&str, Vec<&str>)> = Vec::new();
    for s in spots {
        match kinds.iter_mut().find(|(k, _)| *k == s.kind) {
            Some((_, maps)) => {
                if !maps.contains(&s.map.as_str()) {
                    maps.push(&s.map);
                }
            }
            None => kinds.push((&s.kind, vec![&s.map])),
        }
    }
    kinds.sort_by_key(|(_, maps)| std::cmp::Reverse(maps.len()));
    let (kind, maps) = kinds.first()?;
    let shown: Vec<&str> = maps.iter().take(2).copied().collect();
    let more = maps.len() - shown.len();
    Some(format!(
        "{kind} at {}{}",
        shown.join(", "),
        if more > 0 { format!(" and {more} more") } else { String::new() }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_reads_and_groups_spots_by_item() {
        let t = parse().unwrap();
        assert_eq!(t.len(), 155);
        let (name, spots) = &t[&10];
        assert_eq!(name, "Mega Potion");
        assert!(
            spots
                .iter()
                .any(|s| s.map == "Land Arena" && s.kind == "Gathering" && s.areas == ["1"])
        );
    }

    #[test]
    fn a_summary_names_the_commonest_kind_and_a_few_maps() {
        let spot = |map: &str, kind: &str| Spot {
            map: map.into(),
            kind: kind.into(),
            rank: "Low".into(),
            areas: vec!["1".into()],
        };
        let spots = [
            spot("Volcano", "Mining"),
            spot("Tundra", "Mining"),
            spot("Tundra", "Gathering"),
            spot("Moga Woods", "Mining"),
        ];
        assert_eq!(summary(&spots).as_deref(), Some("Mining at Volcano, Tundra and 1 more"));
        assert_eq!(summary(&[]), None);
    }

    #[test]
    fn bad_lines_are_errors() {
        assert!(parse_text("1\tx\tVolcano").is_err());
        assert!(parse_text("x\tx\tVolcano\tMining\tLow\t1").is_err());
    }
}

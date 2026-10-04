//! The debug command line, opened with `:` when started with `--debug-edit`.
//!
//! ```text
//! zenny 50000       set the wallet          zenny +500 / zenny -200   change it
//! give iron ore     fill a stack to 99      give honey 5              add 5 (a stack holds 99)
//!     give slagtoth hood   an armor piece or weapon: add one to the equipment box
//!                    (a trailing number that is part of an item's name, like `give tenderizer jwl 3`, is the name)
//! set honey 5       set exactly 5 (0 removes the item)
//! stock             make sure the item pouch and box hold everything the wishlist needs
//! scan head         look for the blacksmith's list in the game's memory (read-only; also body, arms, waist, legs)
//! scan head A, B    the same, for a menu that really shows pieces A and B (names as in the game, comma separated)
//! equip 12          show the 16 bytes of equipment box slot 12 (counted from 0)
//! equip 12 = <hex>  write a whole record (32 hex digits);  equip 12 @4 <hex>  write bytes starting at offset 4
//! talisman auto-guard 10, psychic 5   add a talisman with these skills (copies the kind and id bytes of one you have)
//! stock all         the same, also for wishlisted pieces you already own (to craft another copy)
//! ```

use crate::search;
use mh3u_core::gamedata::GameData;

#[derive(Debug, PartialEq, Eq)]
pub enum ZennyOp {
    Set(u32),
    Add(i64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Zenny(ZennyOp),
    /// Add to the item box; with no count, fill the stack.
    Give {
        item: String,
        count: Option<u16>,
    },
    Set {
        item: String,
        count: u16,
    },
    /// Cover the wishlist; with `include_owned`, also pieces you already own (to craft another copy).
    Stock {
        include_owned: bool,
    },
    /// Show (no bytes) or write bytes of an equipment box record, for finding out what the record's fields do.
    Equip {
        slot: usize,
        offset: usize,
        bytes: Vec<u8>,
    },
    /// Add a talisman with skills given as (skill name, points).
    Talisman {
        skills: Vec<(String, i8)>,
    },
    /// Look in the game's memory for a sharpness bar (its numbers in any unit), read-only.
    Find {
        bar: Vec<u32>,
    },
    /// Look for the blacksmith's list of this kind of armor in the game's memory (read-only).
    Scan {
        kind: u8,
        /// The pieces the menu really shows (comma separated names), when the app's idea of the list should not be trusted.
        names: Vec<String>,
    },
}

fn number(word: &str) -> Option<u32> {
    word.replace(',', "").parse().ok()
}

pub fn parse(text: &str) -> Result<Command, String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let Some((&verb, args)) = words.split_first() else {
        return Err("type a command: zenny, give, set, stock, equip, talisman, scan or find".into());
    };
    match verb.to_lowercase().as_str() {
        "zenny" | "z" => {
            let [arg] = args else {
                return Err("zenny needs an amount: zenny 5000, zenny +500 or zenny -200".into());
            };
            let amount = number(arg.trim_start_matches(['+', '-'])).ok_or("that is not a number")?;
            Ok(Command::Zenny(match arg.chars().next() {
                Some('+') => ZennyOp::Add(i64::from(amount)),
                Some('-') => ZennyOp::Add(-i64::from(amount)),
                _ => ZennyOp::Set(amount),
            }))
        }
        "give" | "g" => {
            let (item, count) = split_item(args);
            let count = count
                .map(|c| u16::try_from(c).map_err(|_| "that count is too large".to_string()))
                .transpose()?;
            if item.is_empty() {
                return Err("give what? e.g. give iron ore 20".into());
            }
            Ok(Command::Give { item, count })
        }
        "set" | "s" => {
            let (item, count) = split_item(args);
            let count = count.ok_or("set needs a count: set honey 5")?;
            if item.is_empty() {
                return Err("set what? e.g. set honey 5".into());
            }
            Ok(Command::Set {
                item,
                count: u16::try_from(count).map_err(|_| "that count is too large".to_string())?,
            })
        }
        "stock" => match args {
            [] => Ok(Command::Stock { include_owned: false }),
            [all] if all.eq_ignore_ascii_case("all") => Ok(Command::Stock { include_owned: true }),
            _ => Err("stock covers the wishlist; 'stock all' also covers pieces you already own".into()),
        },
        "equip" | "eq" => {
            let slot = args
                .first()
                .and_then(|s| s.parse().ok())
                .ok_or("equip needs a slot number: equip 12, equip 12 = <hex> or equip 12 @4 <hex>")?;
            let mut rest = &args[1..];
            let mut offset = 0;
            match rest.first() {
                Some(&"=") => rest = &rest[1..],
                Some(w) if w.starts_with('@') => {
                    offset = w[1..].parse().map_err(|_| "after @ comes the byte offset, like @4".to_string())?;
                    rest = &rest[1..];
                }
                _ => {}
            }
            let hex: String = rest.concat().replace("0x", "");
            if !hex.len().is_multiple_of(2) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("the bytes are hex digits in pairs, like 25 0a".into());
            }
            let bytes = (0..hex.len() / 2)
                .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).unwrap_or(0))
                .collect();
            Ok(Command::Equip { slot, offset, bytes })
        }
        "talisman" | "tal" => {
            let skills = args
                .join(" ")
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|segment| {
                    let (name, points) = segment.rsplit_once(' ').ok_or("each skill is a name and points: auto-guard 10")?;
                    let points: i8 = points
                        .trim_start_matches('+')
                        .parse()
                        .map_err(|_| "the points are a number from -127 to 127")?;
                    Ok((name.trim().to_string(), points))
                })
                .collect::<Result<Vec<_>, String>>()?;
            if skills.is_empty() {
                return Err("talisman needs skills: talisman auto-guard 10, psychic 5".into());
            }
            Ok(Command::Talisman { skills })
        }
        "find" => {
            let bar: Vec<u32> = args.iter().filter_map(|w| number(w)).collect();
            if bar.len() != args.len() || !(3..=7).contains(&bar.len()) || bar[0] == 0 {
                return Err("find 22 11 22 11 20 2: a sharpness bar (3 to 7 numbers, red first)".into());
            }
            Ok(Command::Find { bar })
        }
        "scan" => {
            let kind = match args.first().map(|w| w.to_lowercase()).as_deref() {
                None | Some("head") => 5,
                Some("body") => 1,
                Some("arms") => 2,
                Some("waist") => 3,
                Some("legs") => 4,
                _ => return Err("scan head, body, arms, waist or legs".into()),
            };
            let names = args[args.len().min(1)..]
                .join(" ")
                .split(',')
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .collect();
            Ok(Command::Scan { kind, names })
        }
        other => Err(format!(
            "unknown command '{other}': zenny, give, set, stock, equip, talisman, scan or find"
        )),
    }
}

/// Split `iron ore 20` into the item words and an optional trailing count.
fn split_item(args: &[&str]) -> (String, Option<u32>) {
    match args.split_last() {
        Some((last, rest)) if number(last).is_some() && !rest.is_empty() => (rest.join(" "), number(last)),
        _ => (args.join(" "), None),
    }
}

/// The skill chosen for a query by the same rules as items: an exact name, then a name that starts with it, then one that contains it.
pub fn resolve_skill(game: &GameData, query: &str) -> Option<(u8, String)> {
    let names: Vec<(u16, &str)> = game
        .skill_ids()
        .filter_map(|id| Some((u16::from(id), game.skill_name(id)?)))
        .collect();
    let found = pick(&names, query)?;
    Some((u8::try_from(found.id).ok()?, found.name.to_string()))
}

/// The item chosen for a query, and other names that matched equally well (so the status line can say what else it
/// might have meant).
pub struct Resolved<'a> {
    pub id: u16,
    pub name: &'a str,
    pub others: Vec<&'a str>,
}

impl Resolved<'_> {
    /// `Name` or `Name (also matched: A, B)`.
    pub fn describe(&self) -> String {
        if self.others.is_empty() {
            self.name.to_string()
        } else {
            format!("{} (also matched: {})", self.name, self.others.join(", "))
        }
    }
}

/// Whether an item is called exactly this (ignoring case), so a trailing number can be part of its name.
pub fn has_item_named(game: &GameData, name: &str) -> bool {
    game.item_names().any(|(_, n)| n.eq_ignore_ascii_case(name))
}

/// The item that best matches the query. A whole-name match wins, then a name that starts with the query, then one that
/// contains it; only if none does, every word must match loosely (the same fuzzy rules as the search). Within a step the
/// shortest, then lowest-numbered, name is chosen.
pub fn resolve_item<'a>(game: &'a GameData, query: &str) -> Option<Resolved<'a>> {
    let names: Vec<(u16, &str)> = game
        .item_names()
        .filter(|(_, name)| !name.is_empty() && !name.starts_with("DUMMY") && !name.starts_with('('))
        .collect();
    pick(&names, query)
}

/// The armor piece or weapon a query names as (kind, id, name), by a whole name, the start of one or part of one (no loose matching,
/// so a typo does not give a stray weapon). The second value is how good the match was: 0 whole, 1 start, 2 inside.
pub fn resolve_equipment<'a>(game: &'a GameData, query: &str) -> Option<((u8, u16, &'a str), usize)> {
    let pieces = game.equipment_pieces();
    let names: Vec<(u16, &str)> = pieces.iter().enumerate().map(|(i, p)| (i as u16, p.2)).collect();
    let (step, found) = pick_named(&names, query)?;
    Some((pieces[usize::from(found.id)], step))
}

/// What `give` adds: an item for the item box, or an armor piece or weapon for the equipment box.
pub enum Target<'a> {
    Item(Resolved<'a>),
    Piece { kind: u8, id: u16, name: &'a str },
}

/// The item, armor piece or weapon a query names. A piece wins only when its name matches better than the best item's (a whole
/// name, then the start of one, then part of one); on a tie the item wins, and loose matching is for items only.
pub fn resolve_give<'a>(game: &'a GameData, query: &str) -> Option<Target<'a>> {
    let item = resolve_item(game, query);
    let phrase = query.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    let item_step = item.as_ref().map(|f| {
        let name = f.name.to_lowercase();
        if name == phrase {
            0
        } else if name.starts_with(&phrase) {
            1
        } else if name.contains(&phrase) {
            2
        } else {
            3
        }
    });
    match resolve_equipment(game, query) {
        Some(((kind, id, name), step)) if item_step.is_none_or(|s| step < s) => Some(Target::Piece { kind, id, name }),
        _ => item.map(Target::Item),
    }
}

fn pick<'a>(names: &[(u16, &'a str)], query: &str) -> Option<Resolved<'a>> {
    if let Some((_, found)) = pick_named(names, query) {
        return Some(found);
    }
    pick_loosely(names, query)
}

/// The best match by whole name, then start, then containing the query, with which of those it was.
fn pick_named<'a>(names: &[(u16, &'a str)], query: &str) -> Option<(usize, Resolved<'a>)> {
    let phrase = query.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    if phrase.is_empty() {
        return None;
    }
    let lowered: Vec<(u16, &str, String)> = names.iter().map(|&(id, name)| (id, name, name.to_lowercase())).collect();
    for step in 0..3 {
        // (where it matched, length, id, name): the order they are preferred in
        let mut hits: Vec<(usize, usize, u16, &str)> = lowered
            .iter()
            .filter_map(|(id, name, lower)| {
                let at = match step {
                    0 => (*lower == phrase).then_some(0),
                    1 => lower.starts_with(&phrase).then_some(0),
                    _ => lower.find(&phrase),
                }?;
                Some((at, lower.len(), *id, *name))
            })
            .collect();
        if !hits.is_empty() {
            hits.sort();
            return Some((
                step,
                Resolved {
                    id: hits[0].2,
                    name: hits[0].3,
                    others: hits[1..].iter().take(3).map(|h| h.3).collect(),
                },
            ));
        }
    }
    None
}

fn pick_loosely<'a>(names: &[(u16, &'a str)], query: &str) -> Option<Resolved<'a>> {
    let phrase = query.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
    if phrase.is_empty() {
        return None;
    }
    let lowered: Vec<(u16, &str, String)> = names.iter().map(|&(id, name)| (id, name, name.to_lowercase())).collect();
    let words: Vec<&str> = phrase.split(' ').collect();
    lowered
        .iter()
        .filter_map(|(id, name, lower)| {
            let mut total = 0;
            for w in &words {
                total += search::score(w, lower)?;
            }
            Some((total, *id, *name))
        })
        .max_by_key(|&(score, id, _)| (score, std::cmp::Reverse(id)))
        .map(|(_, id, name)| Resolved {
            id,
            name,
            others: Vec::new(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn give(item: &str, count: Option<u16>) -> Command {
        Command::Give { item: item.into(), count }
    }

    #[test]
    fn equip_shows_writes_or_pokes_a_record() {
        assert_eq!(
            parse("equip 12"),
            Ok(Command::Equip {
                slot: 12,
                offset: 0,
                bytes: vec![]
            })
        );
        assert_eq!(
            parse("equip 3 = 06 00 00 01 25 0a"),
            Ok(Command::Equip {
                slot: 3,
                offset: 0,
                bytes: vec![6, 0, 0, 1, 0x25, 0x0a]
            })
        );
        assert_eq!(
            parse("equip 3 @4 250a"),
            Ok(Command::Equip {
                slot: 3,
                offset: 4,
                bytes: vec![0x25, 0x0a]
            })
        );
        assert!(parse("equip").is_err() && parse("equip x").is_err());
        assert!(parse("equip 3 @4 25 0").is_err(), "an odd number of digits");
        assert!(parse("equip 3 @4 zz").is_err());
    }

    #[test]
    fn talisman_takes_skills_and_points_separated_by_commas() {
        assert_eq!(
            parse("talisman auto-guard +10, psychic -3"),
            Ok(Command::Talisman {
                skills: vec![("auto-guard".into(), 10), ("psychic".into(), -3)]
            })
        );
        assert_eq!(
            parse("tal attack up (s) 4"),
            Ok(Command::Talisman {
                skills: vec![("attack up (s)".into(), 4)]
            })
        );
        assert!(parse("talisman").is_err());
        assert!(parse("talisman psychic").is_err(), "no points");
        assert!(parse("talisman psychic lots").is_err());
    }

    #[test]
    fn parses_zenny() {
        assert_eq!(parse("zenny 5000"), Ok(Command::Zenny(ZennyOp::Set(5000))));
        assert_eq!(parse("zenny 50,000"), Ok(Command::Zenny(ZennyOp::Set(50_000))));
        assert_eq!(parse("zenny +500"), Ok(Command::Zenny(ZennyOp::Add(500))));
        assert_eq!(parse("zenny -200"), Ok(Command::Zenny(ZennyOp::Add(-200))));
        assert!(parse("zenny").is_err());
        assert!(parse("zenny lots").is_err());
    }

    #[test]
    fn parses_find_with_a_bar() {
        assert_eq!(
            parse("find 22 11 22 11 20 2"),
            Ok(Command::Find {
                bar: vec![22, 11, 22, 11, 20, 2]
            })
        );
        assert!(parse("find").is_err());
        assert!(parse("find 22 11").is_err());
        assert!(parse("find 22 x 3").is_err());
    }

    #[test]
    fn parses_give_and_set_with_multi_word_items() {
        assert_eq!(parse("give iron ore 20"), Ok(give("iron ore", Some(20))));
        assert_eq!(parse("give iron ore"), Ok(give("iron ore", None)));
        assert_eq!(parse("give honey 5"), Ok(give("honey", Some(5))));
        assert_eq!(parse("give 5"), Ok(give("5", None))); // nothing but a number is the item words, not a count
        assert_eq!(
            parse("set honey 0"),
            Ok(Command::Set {
                item: "honey".into(),
                count: 0
            })
        );
        assert!(parse("set honey").is_err());
        assert!(parse("give").is_err());
        assert!(parse("give honey 99999999").is_err());
    }

    #[test]
    fn scan_takes_a_kind_of_armor_and_defaults_to_the_head() {
        assert_eq!(parse("scan"), Ok(Command::Scan { kind: 5, names: vec![] }));
        assert_eq!(parse("scan Legs"), Ok(Command::Scan { kind: 4, names: vec![] }));
        assert_eq!(
            parse("scan head Leather Headgear, Hunter's Helm"),
            Ok(Command::Scan {
                kind: 5,
                names: vec!["Leather Headgear".into(), "Hunter's Helm".into()]
            })
        );
        assert!(parse("scan feet").is_err());
    }

    #[test]
    fn parses_stock_and_rejects_the_rest() {
        assert_eq!(parse("stock"), Ok(Command::Stock { include_owned: false }));
        assert_eq!(parse("  STOCK "), Ok(Command::Stock { include_owned: false }));
        assert_eq!(parse("stock all"), Ok(Command::Stock { include_owned: true }));
        assert_eq!(parse("Stock ALL"), Ok(Command::Stock { include_owned: true }));
        assert!(parse("stock everything").is_err());
        assert!(parse("").is_err());
        assert!(parse("dance").unwrap_err().contains("unknown command"));
    }

    fn bones() -> Vec<(u16, &'static str)> {
        vec![
            (1, "Monster Bone S"),
            (2, "Monster Bone M"),
            (3, "Monster Bone L"),
            (4, "Monster Bone+"),
            (5, "Honey"),
            (6, "Honeycomb"),
            (7, "Iron Ore"),
        ]
    }

    fn chosen(query: &str) -> Option<u16> {
        pick(&bones(), query).map(|r| r.id)
    }

    #[test]
    fn a_whole_name_wins_over_a_shorter_loose_match() {
        // these used to give Monster Bone+ for the single-letter ones, because 's' / 'm' match inside "monster" anywhere
        assert_eq!(chosen("monster bone s"), Some(1));
        assert_eq!(chosen("monster bone m"), Some(2));
        assert_eq!(chosen("monster bone l"), Some(3));
        assert_eq!(chosen("MONSTER   bone  S"), Some(1));
        assert_eq!(chosen("honey"), Some(5)); // not Honeycomb
    }

    #[test]
    fn a_prefix_or_substring_picks_the_shortest_and_lists_the_rest() {
        let r = pick(&bones(), "monster bone").unwrap();
        assert_eq!((r.id, r.name), (4, "Monster Bone+"));
        assert_eq!(r.others, ["Monster Bone S", "Monster Bone M", "Monster Bone L"]);
        assert_eq!(
            r.describe(),
            "Monster Bone+ (also matched: Monster Bone S, Monster Bone M, Monster Bone L)"
        );
        assert_eq!(chosen("bone+"), Some(4));
        assert_eq!(chosen("ore"), Some(7));
        assert_eq!(pick(&bones(), "iron ore").unwrap().describe(), "Iron Ore");
    }

    #[test]
    fn falls_back_to_loose_word_matching_and_gives_up_cleanly() {
        assert_eq!(chosen("monster bonem"), Some(2)); // the typo that happened to work before
        assert_eq!(chosen("hny"), Some(5));
        assert_eq!(chosen("zzzz"), None);
        assert_eq!(chosen("   "), None);
    }
}

#[cfg(test)]
mod give_tests {
    use super::*;

    #[test]
    fn a_name_is_matched_whole_then_by_its_start_then_inside() {
        let names = [(0u16, "Slagtoth Hood"), (1, "Hood of Slagtoth"), (2, "Honey")];
        assert_eq!(pick_named(&names, "slagtoth hood").map(|(s, f)| (s, f.id)), Some((0, 0)));
        assert_eq!(pick_named(&names, "slag").map(|(s, f)| (s, f.id)), Some((1, 0)));
        assert_eq!(pick_named(&names, "of slag").map(|(s, f)| (s, f.id)), Some((2, 1)));
        assert!(pick_named(&names, "slagtooth").is_none(), "no loose matching");
    }
}

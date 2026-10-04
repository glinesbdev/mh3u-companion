//! The debug command line, opened with `:` when started with `--debug-edit`.
//!
//! ```text
//! zenny 50000       set the wallet          zenny +500 / zenny -200   change it
//! give iron ore     fill a stack to 99      give honey 5              add 5 (a stack holds 99)
//! set honey 5       set exactly 5 (0 removes the item)
//! stock             make sure the item pouch and box hold everything the wishlist needs
//! scan head         look for the blacksmith's list in the game's memory (read-only; also body, arms, waist, legs)
//! scan head A, B    the same, for a menu that really shows pieces A and B (names as in the game, comma separated)
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
        return Err("type a command: zenny, give, set, stock or scan".into());
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
        other => Err(format!("unknown command '{other}': zenny, give, set, stock or scan")),
    }
}

/// Split `iron ore 20` into the item words and an optional trailing count.
fn split_item(args: &[&str]) -> (String, Option<u32>) {
    match args.split_last() {
        Some((last, rest)) if number(last).is_some() && !rest.is_empty() => (rest.join(" "), number(last)),
        _ => (args.join(" "), None),
    }
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

fn pick<'a>(names: &[(u16, &'a str)], query: &str) -> Option<Resolved<'a>> {
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
            return Some(Resolved {
                id: hits[0].2,
                name: hits[0].3,
                others: hits[1..].iter().take(3).map(|h| h.3).collect(),
            });
        }
    }
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
    fn parses_zenny() {
        assert_eq!(parse("zenny 5000"), Ok(Command::Zenny(ZennyOp::Set(5000))));
        assert_eq!(parse("zenny 50,000"), Ok(Command::Zenny(ZennyOp::Set(50_000))));
        assert_eq!(parse("zenny +500"), Ok(Command::Zenny(ZennyOp::Add(500))));
        assert_eq!(parse("zenny -200"), Ok(Command::Zenny(ZennyOp::Add(-200))));
        assert!(parse("zenny").is_err());
        assert!(parse("zenny lots").is_err());
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

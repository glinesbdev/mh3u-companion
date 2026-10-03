//! Build templates: named sets of armor pieces and a talisman, kept between sessions (one file per hunter).
//!
//! A piece is remembered by its equipment kind and id, so the template survives a reload of the game data. A talisman has no
//! fixed stats (they are in the save record), so its skills are kept with it.

/// The slots of a set in display order: head, body, arms, waist, legs, talisman.
pub const SLOTS: [u8; 6] = [5, 1, 2, 3, 4, 6];

pub fn slot_label(kind: u8) -> &'static str {
    match kind {
        5 => "Head",
        1 => "Body",
        2 => "Arms",
        3 => "Waist",
        4 => "Legs",
        _ => "Talisman",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub kind: u8,
    pub id: u16,
    /// A talisman's (skill id, points); empty for armor.
    pub skills: Vec<(u8, i8)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub name: String,
    /// At most one piece per kind.
    pub pieces: Vec<Piece>,
}

impl Template {
    /// Put `piece` in its slot, replacing what was there; `None` empties the slot of `kind`.
    pub fn set(&mut self, kind: u8, piece: Option<Piece>) {
        self.pieces.retain(|p| p.kind != kind);
        self.pieces.extend(piece);
        self.pieces.sort_by_key(|p| SLOTS.iter().position(|&k| k == p.kind));
    }
}

/// A name that is safe to keep on one line: no tabs or line breaks, no leading or trailing blanks.
pub fn clean_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn format(templates: &[Template]) -> String {
    let mut out = String::new();
    for t in templates {
        out += &format!("template\t{}\n", clean_name(&t.name));
        for p in &t.pieces {
            let skills: Vec<String> = p.skills.iter().map(|&(id, pts)| format!("{id}:{pts}")).collect();
            out += &format!("piece\t{}\t{}\t{}\n", p.kind, p.id, skills.join(","));
        }
    }
    out
}

/// Read the file; lines that make no sense are skipped, and a piece before any `template` line is dropped.
pub fn parse(text: &str) -> Vec<Template> {
    let mut out: Vec<Template> = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.as_slice() {
            ["template", name, ..] if !clean_name(name).is_empty() => out.push(Template {
                name: clean_name(name),
                pieces: Vec::new(),
            }),
            ["piece", kind, id, rest @ ..] => {
                let (Ok(kind), Ok(id)) = (kind.parse::<u8>(), id.parse::<u16>()) else {
                    continue;
                };
                if !SLOTS.contains(&kind) {
                    continue;
                }
                let skills = rest
                    .first()
                    .map(|s| {
                        s.split(',')
                            .filter_map(|pair| {
                                let (id, pts) = pair.split_once(':')?;
                                Some((id.parse().ok()?, pts.parse().ok()?))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if let Some(t) = out.last_mut() {
                    t.set(kind, Some(Piece { kind, id, skills }));
                }
            }
            _ => {}
        }
    }
    out
}

/// A name for a new template that no other template has: `Build 1`, `Build 2`, ...
pub fn next_name(existing: &[Template]) -> String {
    (1..)
        .map(|n| format!("Build {n}"))
        .find(|name| !existing.iter().any(|t| &t.name == name))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn armor(kind: u8, id: u16) -> Piece {
        Piece {
            kind,
            id,
            skills: Vec::new(),
        }
    }

    #[test]
    fn templates_round_trip_with_a_talisman_and_keep_slot_order() {
        let mut t = Template {
            name: "Attack for Rathian".into(),
            pieces: Vec::new(),
        };
        t.set(4, Some(armor(4, 12)));
        t.set(5, Some(armor(5, 10)));
        t.set(
            6,
            Some(Piece {
                kind: 6,
                id: 1,
                skills: vec![(37, 10), (11, -2)],
            }),
        );
        assert_eq!(
            t.pieces.iter().map(|p| p.kind).collect::<Vec<_>>(),
            [5, 4, 6],
            "head, legs, talisman"
        );
        let other = Template {
            name: "Empty".into(),
            pieces: Vec::new(),
        };
        let both = vec![t, other];
        assert_eq!(parse(&format(&both)), both);
    }

    #[test]
    fn setting_a_slot_replaces_it_and_none_empties_it() {
        let mut t = Template {
            name: "x".into(),
            pieces: vec![armor(5, 1)],
        };
        t.set(5, Some(armor(5, 2)));
        assert_eq!(t.pieces.iter().find(|p| p.kind == 5).map(|p| p.id), Some(2));
        t.set(5, None);
        assert!(t.pieces.iter().all(|p| p.kind != 5));
    }

    #[test]
    fn bad_lines_are_skipped() {
        let text = "piece\t5\t1\t\ntemplate\t\ntemplate\tReal\npiece\tx\t1\npiece\t9\t1\t\npiece\t5\t7\t3:4,bad,5:x\nnonsense\n";
        let parsed = parse(text);
        assert_eq!(parsed.len(), 1, "a piece before any template and a nameless template are dropped");
        assert_eq!(parsed[0].name, "Real");
        assert_eq!(
            parsed[0].pieces,
            [Piece {
                kind: 5,
                id: 7,
                skills: vec![(3, 4)]
            }],
            "unknown kinds and bad skills are skipped"
        );
    }

    #[test]
    fn names_stay_on_one_line_and_new_names_do_not_repeat() {
        assert_eq!(clean_name("  a\tb\nc "), "a b c");
        let existing = vec![
            Template {
                name: "Build 1".into(),
                pieces: Vec::new(),
            },
            Template {
                name: "Build 3".into(),
                pieces: Vec::new(),
            },
        ];
        assert_eq!(next_name(&existing), "Build 2");
        assert_eq!(next_name(&[]), "Build 1");
    }
}

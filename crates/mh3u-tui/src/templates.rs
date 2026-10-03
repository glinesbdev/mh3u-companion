//! Build templates: named sets of armor pieces and a talisman, kept between sessions (one file per hunter).
//!
//! A piece is remembered by its equipment kind and id, so the template survives a reload of the game data. A talisman has no
//! fixed stats (they are in the save record), so its skills are kept with it.

/// A slot of a set. The armor slots and the talisman hold one equipment kind each; the weapon slot holds a weapon of any type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Head,
    Body,
    Arms,
    Waist,
    Legs,
    Talisman,
    Weapon,
}

impl Slot {
    /// In display order.
    pub const ALL: [Slot; 7] = [
        Slot::Head,
        Slot::Body,
        Slot::Arms,
        Slot::Waist,
        Slot::Legs,
        Slot::Talisman,
        Slot::Weapon,
    ];

    /// The slot a piece of this equipment kind goes in, if it goes in any (kind 12 does not exist).
    pub fn of_kind(kind: u8) -> Option<Slot> {
        Some(match kind {
            5 => Slot::Head,
            1 => Slot::Body,
            2 => Slot::Arms,
            3 => Slot::Waist,
            4 => Slot::Legs,
            6 => Slot::Talisman,
            7..=11 | 13..=19 => Slot::Weapon,
            _ => return None,
        })
    }

    /// The equipment kind of an armor slot or the talisman; a weapon slot has none (any weapon type).
    pub fn kind(self) -> Option<u8> {
        match self {
            Slot::Head => Some(5),
            Slot::Body => Some(1),
            Slot::Arms => Some(2),
            Slot::Waist => Some(3),
            Slot::Legs => Some(4),
            Slot::Talisman => Some(6),
            Slot::Weapon => None,
        }
    }

    pub fn index(self) -> usize {
        Slot::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }

    pub fn label(self) -> &'static str {
        match self {
            Slot::Head => "Head",
            Slot::Body => "Body",
            Slot::Arms => "Arms",
            Slot::Waist => "Waist",
            Slot::Legs => "Legs",
            Slot::Talisman => "Talisman",
            Slot::Weapon => "Weapon",
        }
    }
}

/// The label of the slot a piece of this kind goes in.
pub fn slot_label(kind: u8) -> &'static str {
    Slot::of_kind(kind).map_or("?", Slot::label)
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
    /// At most one piece per slot.
    pub pieces: Vec<Piece>,
}

impl Template {
    /// Put `piece` in `slot`, replacing what was there; `None` empties the slot.
    pub fn set(&mut self, slot: Slot, piece: Option<Piece>) {
        self.pieces.retain(|p| Slot::of_kind(p.kind) != Some(slot));
        self.pieces.extend(piece);
        self.sort();
    }

    /// Keep the pieces in the order of the slots.
    pub fn sort(&mut self) {
        self.pieces.sort_by_key(|p| Slot::of_kind(p.kind).map(Slot::index));
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
                let Some(slot) = Slot::of_kind(kind) else { continue };
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
                    t.set(slot, Some(Piece { kind, id, skills }));
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
        t.set(Slot::Legs, Some(armor(4, 12)));
        t.set(Slot::Head, Some(armor(5, 10)));
        t.set(Slot::Weapon, Some(armor(14, 3)));
        t.set(
            Slot::Talisman,
            Some(Piece {
                kind: 6,
                id: 1,
                skills: vec![(37, 10), (11, -2)],
            }),
        );
        assert_eq!(
            t.pieces.iter().map(|p| p.kind).collect::<Vec<_>>(),
            [5, 4, 6, 14],
            "head, legs, talisman, weapon"
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
        t.set(Slot::Head, Some(armor(5, 2)));
        assert_eq!(t.pieces.iter().find(|p| p.kind == 5).map(|p| p.id), Some(2));
        t.set(Slot::Head, None);
        assert!(t.pieces.iter().all(|p| p.kind != 5));
        // a weapon of any type fills the one weapon slot
        t.set(Slot::Weapon, Some(armor(7, 1)));
        t.set(Slot::Weapon, Some(armor(17, 9)));
        assert_eq!(t.pieces.len(), 1);
        assert_eq!(t.pieces.first().map(|p| (p.kind, p.id)), Some((17, 9)));
    }

    #[test]
    fn bad_lines_are_skipped() {
        let text = "piece\t5\t1\t\ntemplate\t\ntemplate\tReal\npiece\tx\t1\npiece\t12\t1\t\npiece\t5\t7\t3:4,bad,5:x\nnonsense\n";
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
    fn slots_know_their_kinds() {
        assert_eq!(Slot::of_kind(5), Some(Slot::Head));
        assert_eq!(Slot::of_kind(6), Some(Slot::Talisman));
        for weapon in [7, 11, 13, 19] {
            assert_eq!(Slot::of_kind(weapon), Some(Slot::Weapon));
        }
        assert_eq!(Slot::of_kind(12), None);
        assert_eq!(Slot::of_kind(0), None);
        assert_eq!(Slot::Weapon.kind(), None);
        assert_eq!(Slot::ALL.iter().map(|s| s.index()).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 5, 6]);
        assert_eq!(slot_label(14), "Weapon");
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

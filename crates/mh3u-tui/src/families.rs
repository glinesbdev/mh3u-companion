//! Armor sets: pieces grouped by the family in their names (`Agnaktor Helm`, `Agnaktor Helm S`, `Agnaktor Cap Z`...).
//!
//! The game has no table of sets and no upgrade chain for armor, so this is by name: the last word is the kind of piece (Helm, Cap,
//! Mail...), a final `S`, `U`, `X` or `Z` is the variant (the base set has none), and what is left is the family.

use std::collections::BTreeMap;

/// Which version of a set a piece is. Later ones are made from tougher monsters' parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Variant {
    Base,
    S,
    U,
    X,
    Z,
}

impl Variant {
    pub const ALL: [Variant; 5] = [Variant::Base, Variant::S, Variant::U, Variant::X, Variant::Z];

    pub fn label(self) -> &'static str {
        match self {
            Variant::Base => "base",
            Variant::S => "S",
            Variant::U => "U",
            Variant::X => "X",
            Variant::Z => "Z",
        }
    }
}

/// One piece of a family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// The equipment kind: 1 body, 2 arms, 3 waist, 4 legs, 5 head.
    pub kind: u8,
    pub id: u16,
    pub variant: Variant,
    /// The name of the kind of piece in this family (`Helm`, `Cap`).
    pub word: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Family {
    pub name: String,
    /// Ordered by variant, then slot (head, body, arms, waist, legs), then id.
    pub members: Vec<Member>,
}

impl Family {
    /// The pieces of one variant in one slot (usually two: one for each gender or class).
    pub fn cell(&self, variant: Variant, kind: u8) -> impl Iterator<Item = &Member> {
        self.members.iter().filter(move |m| m.variant == variant && m.kind == kind)
    }

    /// The variants this family has, in order.
    pub fn variants(&self) -> Vec<Variant> {
        Variant::ALL
            .into_iter()
            .filter(|&v| self.members.iter().any(|m| m.variant == v))
            .collect()
    }
}

/// Split a piece name into (family, kind-of-piece word, variant). A name of one word is its own family with no piece word.
pub fn split_name(name: &str) -> (String, String, Variant) {
    let mut words: Vec<&str> = name.split_whitespace().collect();
    let variant = match words.last().copied() {
        Some("S") if words.len() > 1 => Variant::S,
        Some("U") if words.len() > 1 => Variant::U,
        Some("X") if words.len() > 1 => Variant::X,
        Some("Z") if words.len() > 1 => Variant::Z,
        _ => Variant::Base,
    };
    if variant != Variant::Base {
        words.pop();
    }
    let piece = if words.len() > 1 { words.pop().unwrap_or_default() } else { "" };
    (words.join(" "), piece.to_string(), variant)
}

/// Where a slot sits in a row of a set: head, body, arms, waist, legs.
pub const SLOT_ORDER: [u8; 5] = [5, 1, 2, 3, 4];

/// Group named pieces `(kind, id, name)` into families, alphabetically. Only armor kinds count.
pub fn group<'a>(pieces: impl IntoIterator<Item = (u8, u16, &'a str)>) -> Vec<Family> {
    let mut by_family: BTreeMap<String, Vec<Member>> = BTreeMap::new();
    for (kind, id, name) in pieces {
        if !SLOT_ORDER.contains(&kind) {
            continue;
        }
        let (family, word, variant) = split_name(name);
        if family.is_empty() {
            continue;
        }
        by_family.entry(family).or_default().push(Member { kind, id, variant, word });
    }
    by_family
        .into_iter()
        .map(|(name, mut members)| {
            members.sort_by_key(|m| (m.variant, SLOT_ORDER.iter().position(|&k| k == m.kind), m.id));
            Family { name, members }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_split_into_family_piece_and_variant() {
        assert_eq!(split_name("Agnaktor Helm"), ("Agnaktor".into(), "Helm".into(), Variant::Base));
        assert_eq!(split_name("Agnaktor Cap Z"), ("Agnaktor".into(), "Cap".into(), Variant::Z));
        assert_eq!(
            split_name("Guild Bard Bolero X"),
            ("Guild Bard".into(), "Bolero".into(), Variant::X)
        );
        assert_eq!(split_name("Archer's Turban"), ("Archer's".into(), "Turban".into(), Variant::Base));
        assert_eq!(
            split_name("Mask"),
            ("Mask".into(), String::new(), Variant::Base),
            "one word is its own family"
        );
        assert_eq!(
            split_name("X"),
            ("X".into(), String::new(), Variant::Base),
            "a lone letter is not a variant"
        );
    }

    #[test]
    fn pieces_group_by_family_and_order_by_variant_then_slot() {
        let pieces = [
            (5, 2, "Agnaktor Helm S"),
            (5, 1, "Agnaktor Helm"),
            (1, 9, "Agnaktor Mail"),
            (5, 3, "Agnaktor Cap"),
            (4, 7, "Zinogre Greaves"),
            (7, 1, "Some Sword"),
        ];
        let all = group(pieces);
        assert_eq!(
            all.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            ["Agnaktor", "Zinogre"],
            "weapons are not sets"
        );
        let agnaktor = &all[0];
        let order: Vec<(Variant, u8, u16)> = agnaktor.members.iter().map(|m| (m.variant, m.kind, m.id)).collect();
        assert_eq!(
            order,
            [
                (Variant::Base, 5, 1),
                (Variant::Base, 5, 3),
                (Variant::Base, 1, 9),
                (Variant::S, 5, 2)
            ]
        );
        assert_eq!(agnaktor.variants(), [Variant::Base, Variant::S]);
        assert_eq!(agnaktor.cell(Variant::Base, 5).count(), 2, "a Helm and a Cap");
        assert_eq!(agnaktor.cell(Variant::S, 1).count(), 0);
    }
}

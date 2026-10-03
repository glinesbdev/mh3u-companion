//! The help screen: the whole screen, as short sections of keys laid out in columns.

use super::*;
use ratatui::widgets::Padding;

/// One part of the help: a heading and rows of (keys, what they do). A row with no keys is a plain line of text.
pub(super) struct Section {
    title: &'static str,
    rows: &'static [(&'static str, &'static str)],
}

pub(super) const SECTIONS: &[Section] = &[
    Section {
        title: "Everywhere",
        rows: &[
            ("↑ ↓  j k", "move"),
            ("PgUp PgDn", "move ten rows"),
            ("Home End  g G", "top, bottom"),
            ("← →  h l  Tab", "switch tab"),
            ("/", "search (Items, Crafting, Quests, Families)"),
            ("x  Esc", "clear the search"),
            ("t", "upgrade tree of a weapon (↑ ↓ scroll, t closes)"),
            ("i", "what each skill does"),
            ("H", "choose which save slot's hunter to show (not while live)"),
            ("?", "this help"),
            ("q", "quit"),
        ],
    },
    Section {
        title: "Items",
        rows: &[
            ("p", "pouch or box"),
            ("s", "sort the box"),
            (
                "u",
                "only items with some to spare (more than the wishlist and any one piece you lack take)",
            ),
        ],
    },
    Section {
        title: "Equipment, Worn",
        rows: &[
            ("s", "sort the equipment box"),
            ("v", "put a weapon in the comparison"),
            ("Worn", "totals for what you wear"),
        ],
    },
    Section {
        title: "Crafting",
        rows: &[
            ("c", "only what you can make now"),
            ("o", "hide pieces you own"),
            ("b", "only what the blacksmith offers"),
            ("z", "only what you can pay for now"),
            ("u", "only owned pieces with no price yet"),
            ("s", "sort (the last is cheapest first)"),
            ("w", "wishlist ★, with the weapons in between its cheapest way needs; again removes"),
            ("v", "put a weapon in the comparison"),
            (
                "",
                "Search takes name, type, skill, material, male / female / blademaster / gunner and rarity 1-10: 'attack 3'.",
            ),
        ],
    },
    Section {
        title: "Wishlist",
        rows: &[
            ("w  x", "remove a piece and the parents added for it"),
            (
                "",
                "The right side shows what the piece needs and one shopping list for everything.",
            ),
        ],
    },
    Section {
        title: "Monsters",
        rows: &[
            ("s", "sort"),
            ("PgUp PgDn", "scroll the drops"),
            ("★", "the wishlist still needs it"),
        ],
    },
    Section {
        title: "Hunt plan",
        rows: &[
            (
                "",
                "The monsters to hunt and quests to do for the materials the wishlist lacks, best first.",
            ),
            ("r", "one rank only (quests only count with every rank)"),
            ("Enter", "that monster's drops, or that quest"),
        ],
    },
    Section {
        title: "Quests",
        rows: &[
            ("m", "the quest's monster (a list if there are several)"),
            ("s", "sort: game order, name, stars, wishlist first"),
            ("PgUp PgDn", "scroll"),
            ("★", "it gives something the wishlist lacks"),
        ],
    },
    Section {
        title: "Families",
        rows: &[
            ("", "Armor grouped by name, with which variants (base, S, U, X, Z) you have."),
            ("o", "only families you own a piece of"),
            ("Enter", "look the family up in Crafting"),
        ],
    },
    Section {
        title: "Skills",
        rows: &[
            ("", "Pick a skill and see every armor piece that has it, the most points first."),
            ("/", "search the skills"),
            ("o", "only armor you own or the blacksmith offers"),
            ("Enter", "add the skill to the Builds tab"),
            ("PgUp PgDn", "scroll the pieces"),
        ],
    },
    Section {
        title: "Pickups",
        rows: &[
            (
                "",
                "With --live: the items you gained, newest first. Changes within 20 seconds are one entry, so a quest's rewards show as a block.",
            ),
            ("★", "the wishlist was short of it"),
        ],
    },
    Section {
        title: "Compare",
        rows: &[
            ("v", "(on a weapon elsewhere) add or remove, up to four"),
            ("x  c", "remove the highlighted weapon, clear all"),
            ("Enter", "look it up in Crafting"),
        ],
    },
    Section {
        title: "Builds",
        rows: &[
            ("a", "add a skill (type to find it, Enter)"),
            ("+  -  x", "its points, remove"),
            ("f", "switch: skills, sets found, templates"),
            ("o", "pieces: owned, plus on offer, or everything"),
            ("m  e  c", "talisman, gender, blademaster or gunner"),
            ("p", "the weapon the set is for (its type sets the class)"),
            ("s  w", "save a set as a template, wish its missing pieces"),
        ],
    },
    Section {
        title: "Templates (Builds, f)",
        rows: &[
            ("[ ]", "pick a slot (head ... talisman, weapon)"),
            ("Enter", "swap its piece (type to find one)"),
            ("n  r  x", "save what you wear, rename, delete"),
            ("w  W", "wish all the missing pieces, only the slot's"),
        ],
    },
    Section {
        title: "What the app tells you",
        rows: &[
            (
                "",
                "On offer: the blacksmith lists a piece once a monster that drops its first material has been hunted. The app remembers what it has seen, since the blacksmith never takes a piece off.",
            ),
            (
                "",
                "Craftable: you hold the materials, or (a weapon) its parent. Upgrading uses the parent up.",
            ),
            (
                "",
                "Files are kept per hunter. With --live the app follows the hunter the game loads.",
            ),
        ],
    },
];

pub(super) const EDIT_SECTION: Section = Section {
    title: "Edit mode (--debug-edit)",
    rows: &[
        ("", "Changes go into the running game; saving in the game keeps them."),
        (":", "zenny 50000 | zenny +500"),
        (":", "give iron ore [n] | set honey 5"),
        (":", "stock | stock all (cover the wishlist)"),
    ],
};

/// Break `text` into lines of at most `width` cells at word boundaries (a word longer than a line is left whole).
pub(super) fn wrap_words(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let fits = line.is_empty() || line.chars().count() + 1 + word.chars().count() <= width;
        if !fits {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line += word;
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// The width of the keys column in a section.
const KEYS: usize = 15;

/// A section laid out for a column of `width` cells: its heading, then each row with the keys in a column and the text wrapped
/// under itself. A row with no keys is text across the full width.
pub(super) fn section_lines(section: &Section, width: usize) -> Vec<Line<'static>> {
    let mut out = vec![Line::styled(
        section.title,
        accent().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
    )];
    for &(keys, text) in section.rows {
        if keys.is_empty() {
            out.extend(wrap_words(text, width).into_iter().map(|l| Line::styled(l, muted())));
            continue;
        }
        let body = width.saturating_sub(KEYS).max(10);
        for (n, piece) in wrap_words(text, body).into_iter().enumerate() {
            let head = if n == 0 { keys } else { "" };
            out.push(Line::from(vec![
                Span::styled(format!("{head:<KEYS$}"), accent().add_modifier(Modifier::BOLD)),
                Span::raw(piece),
            ]));
        }
    }
    out
}

/// Put the sections into `columns` columns in order, never splitting one, so that the tallest column is as short as it can be.
/// Returns the lines of each column.
pub(super) fn arrange(sections: &[Vec<Line<'static>>], columns: usize) -> Vec<Vec<Line<'static>>> {
    let columns = columns.max(1);
    // a section takes its lines and, after the first in a column, one blank line before it
    let height =
        |from: usize, to: usize| -> usize { sections[from..to].iter().map(|s| s.len()).sum::<usize>() + to.saturating_sub(from + 1) };
    let n = sections.len();
    // best[c][i]: the least possible tallest column when the first `i` sections go into `c` columns; cut[c][i]: where the last starts
    let mut best = vec![vec![usize::MAX; n + 1]; columns + 1];
    let mut cut = vec![vec![0usize; n + 1]; columns + 1];
    best[0][0] = 0;
    for c in 1..=columns {
        for i in 0..=n {
            for j in 0..=i {
                if best[c - 1][j] == usize::MAX {
                    continue;
                }
                let tallest = best[c - 1][j].max(if j == i { 0 } else { height(j, i) });
                if tallest < best[c][i] {
                    best[c][i] = tallest;
                    cut[c][i] = j;
                }
            }
        }
    }
    let mut ends = vec![n];
    for c in (2..=columns).rev() {
        ends.push(cut[c][*ends.last().unwrap_or(&n)]);
    }
    ends.push(0);
    ends.reverse();
    ends.windows(2)
        .map(|w| {
            let mut column: Vec<Line> = Vec::new();
            for section in &sections[w[0]..w[1]] {
                if !column.is_empty() {
                    column.push(Line::raw(""));
                }
                column.extend(section.iter().cloned());
            }
            column
        })
        .collect()
}

/// A column is about this wide; the screen gets as many as fit, up to three.
const COLUMN: usize = 50;

pub(super) fn draw_help(f: &mut Frame, app: &mut App) {
    let area = f.area();
    f.render_widget(Clear, area);
    let block = theme::pane(" Help · ↑ ↓ PageUp PageDown scroll · any other key closes ", true).padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(area);
    f.render_widget(block, area);

    let columns = (usize::from(inner.width) / COLUMN).clamp(1, 3);
    let gap = 3;
    let width = (usize::from(inner.width) + gap) / columns - gap;
    let mut sections: Vec<&Section> = SECTIONS.iter().collect();
    if app.console.enabled {
        sections.push(&EDIT_SECTION);
    }
    let laid_out: Vec<Vec<Line>> = sections.iter().map(|s| section_lines(s, width)).collect();
    let cols = arrange(&laid_out, columns);

    let tallest = cols.iter().map(Vec::len).max().unwrap_or(0);
    let room = usize::from(inner.height);
    app.help_scroll = app.help_scroll.min(tallest.saturating_sub(room) as u16);
    let rects = Layout::horizontal(vec![Constraint::Length(width as u16); columns])
        .spacing(gap as u16)
        .split(inner);
    for (rect, lines) in rects.iter().zip(cols) {
        f.render_widget(Paragraph::new(lines).scroll((app.help_scroll, 0)), *rect);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(lines: &[Line]) -> Vec<String> {
        lines.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn words_wrap_at_the_width_and_long_words_stay_whole() {
        assert_eq!(wrap_words("aa bb cc dd", 5), ["aa bb", "cc dd"]);
        assert_eq!(wrap_words("aa bb cc", 8), ["aa bb cc"]);
        assert_eq!(wrap_words("abcdefgh ij", 4), ["abcdefgh", "ij"]);
        assert_eq!(wrap_words("", 10), [""]);
    }

    #[test]
    fn a_section_has_its_heading_keys_in_a_column_and_text_wrapped_under_itself() {
        let section = Section {
            title: "Demo",
            rows: &[("s", "one two three four five six seven"), ("", "a note")],
        };
        let lines = text(&section_lines(&section, 28));
        assert_eq!(lines[0], "Demo");
        assert_eq!(lines[1], "s              one two three");
        assert!(
            lines[2].starts_with("               "),
            "wrapped text lines up under the first: {lines:?}"
        );
        assert_eq!(lines.last().map(String::as_str), Some("a note"), "a row with no keys is plain text");
        assert!(lines.iter().all(|l| l.chars().count() <= 28));
    }

    #[test]
    fn sections_go_into_columns_in_order_and_are_never_split() {
        let sections: Vec<Vec<Line>> = [3, 3, 3, 3]
            .iter()
            .map(|&n| (0..n).map(|i| Line::raw(format!("l{i}"))).collect())
            .collect();
        let cols = arrange(&sections, 2);
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].len(), 7, "two sections and the blank line between them");
        assert_eq!(cols[1].len(), 7);
        let one = arrange(&sections, 1);
        assert_eq!(one[0].len(), 15, "four sections and three blank lines");
        assert!(arrange(&[], 3).iter().all(Vec::is_empty));
        // uneven sections: the split that keeps the tallest column shortest (8+1+2 = 11 would be worse than 8 | 2+1+2+1+2)
        let uneven: Vec<Vec<Line>> = [8, 2, 2, 2].iter().map(|&n| (0..n).map(|_| Line::raw("x")).collect()).collect();
        let heights: Vec<usize> = arrange(&uneven, 2).iter().map(Vec::len).collect();
        assert_eq!(heights, [8, 8]);
    }

    #[test]
    fn the_help_fits_a_normal_screen_in_three_columns() {
        // 180 columns of text and a 45-row terminal should show all of it without scrolling
        let sections: Vec<Vec<Line>> = SECTIONS.iter().map(|s| section_lines(s, 56)).collect();
        let tallest = arrange(&sections, 3).iter().map(Vec::len).max().unwrap();
        assert!(tallest <= 42, "the tallest of three columns is {tallest} lines");
    }
}

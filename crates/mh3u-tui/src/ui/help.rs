//! The help screen.

use super::*;

/// What the help screen says: (label, text). A label starts a row whose text wraps under itself; an empty label continues the
/// row above it with a new line; `"¶"` is a paragraph across the full width; both empty is a blank line.
pub(super) const HELP: &[(&str, &str)] = &[
    ("Move", "↑/↓ or j/k · PageUp/PageDown"),
    ("Top / bottom", "Home/End or g/G"),
    ("Switch tab", "←/→ or h/l · Tab"),
    ("Quit", "q"),
    ("", ""),
    ("Items", "/  fuzzy search the pouch and box by item name"),
    ("", "s  sort the item box (box order, name, quantity)"),
    (
        "",
        "u  only items with some to spare: more than the wishlist and any one piece you still lack take",
    ),
    (
        "Crafting",
        "/  fuzzy search: name, type, skill, material, male / female / blademaster / gunner, or a rarity number 1-10 (armor only), e.g. 'attack 3'. Several words must all match.",
    ),
    ("Esc or x", "clear the search on the current tab"),
    ("", "c  only what you can make now"),
    ("", "o  hide pieces you already own"),
    ("", "b  only pieces the blacksmith is offering (see below)"),
    ("", "u  only pieces you own that have no price recorded yet"),
    ("", "s  sort (game order, name, craftable first, owned first)"),
    (
        "",
        "w  add to the wishlist (★), with the weapons in between that its cheapest way needs; press again to remove it",
    ),
    (
        "Wishlist",
        "w or x  remove the selected piece, and the parents that were added for it if nothing else needs them",
    ),
    ("Equipment", "s  sort (box order, name, rarity, type, worn first)"),
    ("Worn", "totals for what you are wearing; i shows what each skill does"),
    ("Monsters", "what each monster drops; s sort, ★ = the wishlist still needs it"),
    (
        "Hunt plan",
        "the monsters to hunt and the quests to do, best first, for the materials the wishlist is short of: each step covers as many as it can and the next picks up the rest; r  limit it to one rank (quests only count with every rank);  Enter  show that monster's drops or that quest",
    ),
    (
        "Quests",
        "every quest in the game: its goal, client, time limit, monsters and both reward boxes with the chance of each item;  /  search by name, goal, monster or reward item;  m  show the quest's monster on the Monsters tab (a list to choose from when there are several);  s  sort (game order, name, stars, wishlist first);  ★ = it gives something the wishlist is short of;  PageUp/PageDown  scroll",
    ),
    (
        "Families",
        "armor grouped by family (Agnaktor, Zinogre...), with a table of which variants (base, S, U, X, Z) of each piece you own or the blacksmith offers;  /  search;  o  only sets you own a piece of;  Enter  show the family on the Crafting tab",
    ),
    (
        "Builds",
        "a  add a skill to look for (type to find it, Enter);  + / -  its points;  x  remove it;  f  switch between the skills, the sets found and your templates",
    ),
    (
        "",
        "o  which pieces to use: owned only, plus what the blacksmith offers, or everything in the game (to plan ahead);  m  use your talisman;  e  gender and c  blademaster/gunner filters (pieces for both always pass);  p  choose the weapon the set is for: its type picks the class (bows and bowguns wear gunner armor), and it goes into a saved template",
    ),
    (
        "",
        "s on a set saves it as a template;  w  put the missing pieces on the wishlist.  In the templates: [ ] pick a slot (head... talisman, weapon), Enter swaps its piece (type to find one), n saves what you wear, W wishes only the slot's piece, r rename, x delete. Skills, options and templates are kept per hunter.",
    ),
    (
        "Compare",
        "v  (on a weapon in Crafting, Equipment or Wishlist) put it in the comparison, again to take it out: up to four weapons side by side with rarity, attack, affinity, gem slots and what getting it costs, the best of each row marked (attack only between weapons of one type);  x  take the highlighted one out;  c  clear;  Enter  look it up in Crafting",
    ),
    (
        "Any weapon",
        "t  upgrade tree: the line down to it and everything it upgrades into (↑/↓ scroll, t or Esc close). A weapon you do not own also shows its cheapest way (forging fees) from what you hold.",
    ),
    ("", ""),
    (
        "¶",
        "The blacksmith line follows what the save shows: a piece is on offer once a monster that drops its first material has been hunted (killed or captured) at least once; materials you only hold do not count. Pieces made from high-rank drops (the S, X... sets) need a hunt in that rank. The game never takes a piece off the list, so the app remembers every piece it has seen on offer. An anvil marks those in the crafting list, and the unowned ones on the wishlist.",
    ),
    ("", ""),
    (
        "¶",
        "A piece is craftable if you have the materials to create it, or to upgrade it and you own a parent weapon. Upgrading uses up the parent. The wishlist plans a weapon by its cheapest way (fees only): making it, or upgrading it from the weapon before it, adding the weapons in between that you do not own.",
    ),
];

pub(super) const HELP_EDIT: &[(&str, &str)] = &[
    ("", ""),
    (
        "¶",
        "EDIT MODE (--debug-edit): changes go into the running game, not the file. If you save in the game they are saved too.",
    ),
    ("", ""),
    (":", "zenny 50000 | zenny +500 | give iron ore [n] | set honey 5"),
    (":", "stock (covers the wishlist) | stock all (also pieces you own)"),
];

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

/// The help rows laid out for a text width: labels in a column, their text wrapped under itself.
pub(super) fn help_lines(rows: &[(&str, &str)], width: usize) -> Vec<Line<'static>> {
    const LABEL: usize = 14;
    let mut out: Vec<Line> = Vec::new();
    for &(label, text) in rows {
        if label == "¶" {
            out.extend(wrap_words(text, width).into_iter().map(Line::from));
            continue;
        }
        if label.is_empty() && text.is_empty() {
            out.push(Line::raw(""));
            continue;
        }
        let body = width.saturating_sub(LABEL).max(10);
        for (n, piece) in wrap_words(text, body).into_iter().enumerate() {
            let head = if n == 0 { label } else { "" };
            out.push(Line::from(vec![
                Span::styled(format!("{head:<LABEL$}"), accent().add_modifier(Modifier::BOLD)),
                Span::raw(piece),
            ]));
        }
    }
    out
}

pub(super) fn draw_help(f: &mut Frame, app: &mut App) {
    use ratatui::widgets::Padding;
    let area = f.area();
    let w = area.width.min(89);
    let text_width = usize::from(w).saturating_sub(2 + 4); // borders and two cells of padding on each side
    let mut lines = help_lines(HELP, text_width);
    if app.console.enabled {
        lines.extend(help_lines(HELP_EDIT, text_width));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled("↑/↓ PageUp/PageDown scroll · any other key closes", muted()));
    let h = area.height.min(lines.len() as u16 + 2);
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    let room = usize::from(h.saturating_sub(2));
    let max_scroll = lines.len().saturating_sub(room) as u16;
    app.help_scroll = app.help_scroll.min(max_scroll);
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(lines)
            .scroll((app.help_scroll, 0))
            .block(theme::pane(" Help ", true).padding(Padding::horizontal(2))),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_wrap_at_the_width_and_long_words_stay_whole() {
        assert_eq!(wrap_words("aa bb cc dd", 5), ["aa bb", "cc dd"]);
        assert_eq!(wrap_words("aa bb cc", 8), ["aa bb cc"]);
        assert_eq!(wrap_words("abcdefgh ij", 4), ["abcdefgh", "ij"]);
        assert_eq!(wrap_words("", 10), [""]);
    }

    #[test]
    fn help_rows_wrap_under_their_label_and_fill_the_width() {
        let lines = help_lines(&[("Key", "one two three four five six seven")], 24);
        let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        assert_eq!(text[0], "Key           one two");
        assert!(
            text.iter().all(|l| l.starts_with("Key") || l.starts_with("              ")),
            "{text:?}"
        );
        assert!(text.iter().all(|l| l.trim_end().chars().count() <= 24));
        let para = help_lines(&[("¶", "alpha beta gamma delta")], 12);
        assert_eq!(
            para.iter().map(|l| l.to_string()).collect::<Vec<_>>(),
            ["alpha beta", "gamma delta"]
        );
    }
}

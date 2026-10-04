//! The Items tab and the details of an item.

use super::*;

pub(super) fn draw_items(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 40);
    let [box_area, details_area] = Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(right);
    let rows = |stacks: &[mh3u_core::save::ItemStack]| -> Vec<ListItem<'static>> {
        stacks
            .iter()
            .map(|s| {
                let mut spans = vec![
                    Span::raw(format!("{:<26}", app.game.item_name(s.id).unwrap_or("?"))),
                    Span::styled(format!(" x{:<4}", s.count), muted()),
                ];
                if let Some(p) = app.inv.spare.get(&s.id).filter(|p| p.spare > 0) {
                    // what can go from this stack (the spare count is for the pouch and box together)
                    spans.push(Span::styled(format!("spare {}", p.spare.min(u32::from(s.count))), good()));
                }
                ListItem::new(Line::from(spans))
            })
            .collect()
    };
    let pouch = rows(&app.inv.pouch_view);
    let item_box = rows(&app.inv.box_view);
    let query = app.inv.item_search.trim();
    let focus_pouch = app.items_on_pouch();
    let only = if app.inv.spare_only { " · spare only" } else { "" };
    let pouch_title = if query.is_empty() {
        format!(" Item Pouch ({}/24){only} ", app.save.pouch.len())
    } else {
        format!(" Item Pouch · \"{query}\" ({}/{}) ", app.inv.pouch_view.len(), app.save.pouch.len())
    };
    let box_title = if query.is_empty() {
        format!(" Item Box ({}/1000) · {}{only} ", app.save.item_box.len(), app.box_sort_label())
    } else {
        format!(
            " Item Box · \"{query}\" ({}/{}) · {} ",
            app.inv.box_view.len(),
            app.save.item_box.len(),
            app.box_sort_label()
        )
    };
    // Both lists reserve the room for the selection mark, so their names line up; only the focused one shows a highlight.
    let list = |rows: Vec<ListItem<'static>>, title: String, active: bool| {
        List::new(rows)
            .block(theme::pane(title, active))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK)
            .highlight_spacing(ratatui::widgets::HighlightSpacing::Always)
    };
    let (pouch_len, box_len) = (pouch.len(), item_box.len());
    if pouch.is_empty() {
        let what = if query.is_empty() {
            "Nothing in the pouch."
        } else {
            "No pouch items match."
        };
        empty_pane(f, left, pouch_title, focus_pouch, vec![Line::styled(what, muted())]);
    } else {
        let mut unfocused = ListState::default();
        let state = if focus_pouch { &mut app.inv.pouch_state } else { &mut unfocused };
        render_list(f, list(pouch, pouch_title, focus_pouch), left, state);
        scrollbar(f, left, pouch_len, app.inv.pouch_state.selected().filter(|_| focus_pouch));
    }
    if item_box.is_empty() {
        let what = if query.is_empty() {
            "The item box is empty."
        } else {
            "No box items match. Press x to clear the search."
        };
        empty_pane(f, box_area, box_title, !focus_pouch, vec![Line::styled(what, muted())]);
    } else {
        let mut unfocused = ListState::default();
        let state = if focus_pouch { &mut unfocused } else { &mut app.inv.box_state };
        render_list(f, list(item_box, box_title, !focus_pouch), box_area, state);
        scrollbar(f, box_area, box_len, app.inv.box_state.selected().filter(|_| !focus_pouch));
    }

    let highlighted = if focus_pouch {
        app.inv.pouch_state.selected().and_then(|i| app.inv.pouch_view.get(i))
    } else {
        app.inv.box_state.selected().and_then(|i| app.inv.box_view.get(i))
    };
    let lines = match highlighted {
        Some(stack) => item_details(app, stack.id),
        None => Vec::new(),
    };
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Details ", false)),
        details_area,
    );
}

/// An item: what the game says it is, how many you hold, what the wishlist needs of it and which pieces are made with it.
pub(super) fn item_details(app: &App, id: u16) -> Vec<Line<'static>> {
    let mut lines = vec![Line::styled(app.game.item_name(id).unwrap_or("?").to_string(), bold())];
    lines.push(Line::from(vec![
        Span::styled("In the pouch ", muted()),
        Span::raw(
            app.save
                .pouch
                .iter()
                .filter(|s| s.id == id)
                .map(|s| u32::from(s.count))
                .sum::<u32>()
                .to_string(),
        ),
        Span::styled("  ·  in the box ", muted()),
        Span::raw(
            app.save
                .item_box
                .iter()
                .filter(|s| s.id == id)
                .map(|s| u32::from(s.count))
                .sum::<u32>()
                .to_string(),
        ),
    ]));
    if let Some(text) = app.game.item_description(id) {
        lines.push(Line::raw(text.to_string()));
    }
    lines.extend(spare_lines(app, id));
    let (need, _) = app.shopping_need();
    if let Some(&(_, n)) = need.iter().find(|&&(item, _)| item == id) {
        let have = app.save.item_count(id);
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("Wishlist needs ", muted()),
            Span::styled(format!("{have}/{n}"), theme::progress_style(have, n)),
        ]));
    }
    let sources = app.game.drops().sources(id);
    if !sources.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("Dropped by", bold()));
        // one row per monster and kind of drop, with the chance at each rank
        let mut rows: std::collections::BTreeMap<(u16, mh3u_core::drops::Method), [Option<u8>; 3]> = Default::default();
        for s in sources {
            let slot = mh3u_core::drops::Rank::ALL.iter().position(|&r| r == s.rank).unwrap_or(0);
            rows.entry((s.monster, s.method)).or_default()[slot] = Some(s.percent);
        }
        const SHOWN: usize = 12;
        let total = rows.len();
        for ((monster, method), chances) in rows.into_iter().take(SHOWN) {
            // part names depend on the rank's break lists: use the first rank that has this drop
            let first = chances
                .iter()
                .position(Option::is_some)
                .map_or(mh3u_core::drops::Rank::Low, |i| mh3u_core::drops::Rank::ALL[i]);
            let label = app.game.drop_label(monster, first, method);
            let mut spans = vec![
                Span::raw(format!("  {:<19}", fit(app.game.monster_name(monster).unwrap_or("?"), 18))),
                Span::styled(format!("{:<18}", fit(&label, 17)), muted()),
            ];
            for (label, chance) in ["Low", "High", "G"].into_iter().zip(chances) {
                spans.push(Span::styled(format!("{label} "), muted()));
                spans.push(match chance {
                    Some(p) => Span::styled(format!("{p:>3}%  "), good()),
                    None => Span::styled("  –   ", muted()),
                });
            }
            lines.push(Line::from(spans));
        }
        if total > SHOWN {
            lines.push(Line::styled(format!("  … and {} more", total - SHOWN), muted()));
        }
    }
    let users: Vec<String> = app
        .game
        .recipes_using(id)
        .into_iter()
        .filter_map(|(kind, piece)| app.game.piece_name(kind, piece))
        .map(str::to_string)
        .collect();
    if !users.is_empty() {
        lines.push(Line::raw(""));
        const SHOWN: usize = 8;
        let more = users.len().saturating_sub(SHOWN);
        let mut text = users.iter().take(SHOWN).cloned().collect::<Vec<_>>().join(", ");
        if more > 0 {
            text.push_str(&format!(" … and {more} more"));
        }
        lines.push(Line::from(vec![
            Span::styled(format!("Used in {} piece(s): ", users.len()), bold()),
            Span::raw(text),
        ]));
    }
    lines
}

/// Lay out `items` after `label`, separated by dots, breaking lines to fit `width` and indenting the continuation lines under the
/// first item.
pub(super) fn wrap_items(label: &str, items: Vec<Vec<Span<'static>>>, width: usize) -> Vec<Line<'static>> {
    const SEP: &str = "  ·  ";
    let indent = " ".repeat(label.chars().count());
    let mut lines = Vec::new();
    let mut spans = vec![Span::styled(label.to_string(), muted())];
    let mut used = label.chars().count();
    let mut first = true;
    for item in items {
        let w: usize = item.iter().map(|s| s.content.chars().count()).sum();
        if !first && used + SEP.chars().count() + w > width {
            lines.push(Line::from(std::mem::replace(&mut spans, vec![Span::raw(indent.clone())])));
            used = indent.chars().count();
        } else if !first {
            spans.push(Span::styled(SEP, muted()));
            used += SEP.chars().count();
        }
        spans.extend(item);
        used += w;
        first = false;
    }
    lines.push(Line::from(spans));
    lines
}

/// How many of an item can go, and what the rest is kept for.
fn spare_lines(app: &App, id: u16) -> Vec<Line<'static>> {
    use mh3u_app::surplus::Why;
    let Some(p) = app.inv.spare.get(&id) else { return Vec::new() };
    let have = app.save.item_count(id);
    let (count, style) = if p.spare > 0 {
        (format!("{} of {have}, pouch and box together", p.spare), good())
    } else {
        (format!("none, keep all {have}"), warn())
    };
    let why = match p.why {
        Why::NoRecipe => "No recipe uses it.".to_string(),
        Why::OnlyOwnedPieces => "Only pieces you already own use it.".to_string(),
        Why::Wishlist => format!("Keep {}: the wishlist needs that many.", p.keep),
        Why::OnePiece => format!("Keep {}: the most one piece you do not own takes.", p.keep),
    };
    vec![
        Line::raw(""),
        Line::from(vec![Span::styled("Spare ", muted()), Span::styled(count, style)]),
        Line::styled(why, muted()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(text: &str) -> Vec<Span<'static>> {
        vec![Span::raw(text.to_string())]
    }

    #[test]
    fn items_wrap_under_the_first_item() {
        let items = vec![item("aaaa 10%"), item("bbbb 20%"), item("cccc 30%")];
        // label (4) + "aaaa 10%" (8) + "  ·  " (5) + "bbbb 20%" (8) = 25
        let lines: Vec<String> = wrap_items("Low ", items.clone(), 25).iter().map(Line::to_string).collect();
        assert_eq!(lines, ["Low aaaa 10%  ·  bbbb 20%", "    cccc 30%"]);
        let one: Vec<String> = wrap_items("Low ", items, 80).iter().map(Line::to_string).collect();
        assert_eq!(one, ["Low aaaa 10%  ·  bbbb 20%  ·  cccc 30%"]);
    }

    #[test]
    fn an_item_wider_than_the_pane_still_gets_its_own_line() {
        let lines = wrap_items("Low ", vec![item("a"), item("a very long item name 99%")], 10);
        assert_eq!(lines.len(), 2);
    }
}

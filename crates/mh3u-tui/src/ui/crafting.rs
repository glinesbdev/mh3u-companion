//! The Crafting tab.

use super::*;

pub(super) fn draw_crafting(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 45);

    let rows: Vec<ListItem> = app
        .pieces
        .iter()
        .map(|p| {
            let mark = if p.craftable { Span::styled("✔", good()) } else { Span::raw(" ") };
            let star = if app.is_wished(p.kind, p.id) {
                Span::styled("★", warn())
            } else {
                Span::raw(" ")
            };
            let rarity = match app.game.equipment_rarity(p.kind, p.id) {
                Some(r) => theme::rarity_badge(r),
                None => Span::raw("   "),
            };
            let label = app.game.equipment_kind_label(p.kind).unwrap_or("?");
            let owned = if p.owned { " owned" } else { "" };
            let reason = p.reason.as_deref().map(|r| format!(" · {r}")).unwrap_or_default();
            let anvil = if p.offered { theme::anvil() } else { theme::no_anvil() };
            ListItem::new(Line::from(vec![
                mark,
                star,
                Span::raw(" "),
                anvil,
                Span::raw(format!("{:<24}", fit(&p.name, 24))),
                rarity,
                Span::styled(format!(" {label}{owned}"), muted()),
                Span::styled(reason, warn()),
            ]))
        })
        .collect();
    let mut title = format!(" Pieces ({}) · {} ", rows.len(), app.sort_label());
    if !app.search.is_empty() {
        title = format!(" \"{}\" ({}) · {} ", app.search, rows.len(), app.sort_label());
    }
    if app.craftable_only {
        title.push_str("[craftable only] ");
    }
    if app.hide_owned {
        title.push_str("[hiding owned] ");
    }
    if app.unpriced_only {
        title.push_str("[owned, no price yet] ");
    }
    if app.blacksmith_only {
        title.push_str("[at the blacksmith] ");
    }
    if rows.is_empty() {
        let lines = vec![
            Line::from("No pieces to show."),
            Line::styled("Press x to clear the search, or c / o / u to relax the filters.", muted()),
        ];
        empty_pane(f, left, title, true, lines);
    } else {
        let len = rows.len();
        f.render_stateful_widget(
            List::new(rows)
                .block(theme::pane(title, true))
                .highlight_style(theme::selection())
                .highlight_symbol(theme::SELECTION_MARK),
            left,
            &mut app.craft_state,
        );
        scrollbar(f, left, len, app.craft_state.selected());
    }

    let lines = match app.craft_state.selected().and_then(|i| app.pieces.get(i)) {
        Some(piece) => piece_details(app, piece.kind, piece.id, &piece.name, Some(piece.craftable)),
        None => Vec::new(),
    };
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Recipe (pouch + box) ", false)),
        right,
    );
}

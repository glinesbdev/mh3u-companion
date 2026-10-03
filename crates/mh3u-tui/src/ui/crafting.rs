//! The Crafting tab.

use super::*;

pub(super) fn draw_crafting(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 45);

    let rows: Vec<ListItem> = app
        .craft
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
    if !app.craft.search.is_empty() {
        title = format!(" \"{}\" ({}) · {} ", app.craft.search, rows.len(), app.sort_label());
    }
    if app.craft.craftable_only {
        title.push_str("[craftable only] ");
    }
    if app.craft.hide_owned {
        title.push_str("[hiding owned] ");
    }
    if app.craft.unpriced_only {
        title.push_str("[owned, no price yet] ");
    }
    if app.craft.blacksmith_only {
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
            &mut app.craft.state,
        );
        scrollbar(f, left, len, app.craft.state.selected());
    }

    let lines = match app.craft.state.selected().and_then(|i| app.craft.pieces.get(i)) {
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

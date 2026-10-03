//! The Monsters tab.

use super::*;

/// A monster and what it drops, with the items the wishlist still needs picked out.
pub(super) fn draw_monsters(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 33);
    let view = app.monster_view();
    let selected = app.highlighted_monster();
    let rows: Vec<ListItem> = view
        .iter()
        .map(|&(m, wanted)| {
            let mut spans = vec![Span::raw(format!("{:<22}", fit(app.game.monster_name(m).unwrap_or("?"), 22)))];
            if wanted > 0 {
                spans.push(Span::styled(format!("★ {wanted}"), warn()));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let title = format!(" Monsters ({}) · {} ", rows.len(), app.monster_sort.label());
    if rows.is_empty() {
        empty_pane(f, left, title, true, vec![Line::styled("No monster drop data.", muted())]);
        return;
    }
    let len = rows.len();
    let mut state = ListState::default().with_selected(selected.and_then(|m| view.iter().position(|&(v, _)| v == m)));
    f.render_stateful_widget(
        List::new(rows)
            .block(theme::pane(title, true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        left,
        &mut state,
    );
    scrollbar(f, left, len, state.selected());

    let lines = selected
        .map(|m| monster_details(app, m, usize::from(right.width.saturating_sub(3))))
        .unwrap_or_default();
    let visible = usize::from(right.height.saturating_sub(2));
    let total = lines.len();
    let scroll = usize::from(app.monster_scroll).min(total.saturating_sub(visible));
    app.monster_scroll = scroll as u16;
    f.render_widget(
        Paragraph::new(lines)
            .scroll((scroll as u16, 0))
            .block(theme::pane(" Drops ", false)),
        right,
    );
    scrollbar(f, right, total, Some(scroll));
}

pub(super) fn monster_details(app: &App, monster: u16, width: usize) -> Vec<Line<'static>> {
    let missing = app.missing_for_wishlist();
    let mut lines = vec![Line::styled(app.game.monster_name(monster).unwrap_or("?").to_string(), bold())];
    lines.push(Line::from(vec![
        Span::styled("Chance in percent. ", muted()),
        Span::styled("★", warn()),
        Span::styled(" marks what your wishlist still needs.", muted()),
    ]));
    let mut current = None;
    for (method, rank, list) in app.game.drops().lists_for(monster) {
        if current != Some(method) {
            lines.push(Line::raw(""));
            lines.push(Line::styled(method.label(), bold()));
            current = Some(method);
        }
        let items: Vec<Vec<Span<'static>>> = list
            .iter()
            .map(|d| {
                let name = app.game.item_name(d.item).unwrap_or("?");
                let quantity = if d.quantity > 1 {
                    format!(" x{}", d.quantity)
                } else {
                    String::new()
                };
                let mut spans = if missing.contains_key(&d.item) {
                    vec![Span::styled(format!("★ {name}{quantity}"), warn().add_modifier(Modifier::BOLD))]
                } else {
                    vec![Span::raw(format!("{name}{quantity}"))]
                };
                spans.push(Span::styled(format!(" {}%", d.percent), muted()));
                spans
            })
            .collect();
        lines.extend(wrap_items(&format!("  {:<10}", rank.label()), items, width));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Part breaks are numbered in the game's order; which body part each one is, is not known.",
        muted(),
    ));
    lines
}

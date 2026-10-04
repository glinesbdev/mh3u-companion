//! The Pickups tab.

use super::*;

/// How long ago, in the largest whole unit: `just now`, `5 min ago`, `3 h ago`, `2 d ago`.
fn ago(seconds: u64) -> String {
    match seconds {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", seconds / 60),
        3600..=86_399 => format!("{} h ago", seconds / 3600),
        _ => format!("{} d ago", seconds / 86_400),
    }
}

/// The entries, newest first, and the items of the highlighted one.
pub(super) fn draw_gains(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 40);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let entries: Vec<_> = app.gains.log.entries.iter().rev().collect();
    let title = format!(" Pickups ({}) ", entries.len());
    if entries.is_empty() {
        let lines = vec![
            Line::styled("Nothing picked up yet.", muted()),
            Line::styled("Start with --live: items you gain while playing are logged here.", muted()),
        ];
        empty_pane(f, left, title, true, lines);
        empty_pane(f, right, " Items ".into(), false, Vec::new());
        return;
    }
    let rows: Vec<ListItem> = entries
        .iter()
        .map(|e| {
            let total: u32 = e.items.iter().map(|&(_, n)| n).sum();
            let mut spans = vec![
                Span::raw(format!("{:<11}", ago(now.saturating_sub(e.at)))),
                Span::styled(format!("{total} item(s)"), muted()),
            ];
            if e.zenny != 0 {
                let style = if e.zenny > 0 { good() } else { warn() };
                spans.push(Span::styled(format!("  {}", signed_zenny(e.zenny)), style));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let len = rows.len();
    render_list(
        f,
        List::new(rows)
            .block(theme::pane(title, true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        left,
        &mut app.gains.state,
    );
    scrollbar(f, left, len, app.gains.state.selected());

    let short = app.missing_for_wishlist();
    let lines: Vec<Line> = app
        .gains
        .state
        .selected()
        .and_then(|i| entries.get(i))
        .map(|e| {
            e.items
                .iter()
                .map(|&(id, n)| {
                    let name = app.game.item_name(id).unwrap_or("?");
                    if short.contains_key(&id) {
                        Line::from(vec![
                            Span::styled("★ ", warn()),
                            Span::styled(format!("{name} x{n}"), warn().add_modifier(Modifier::BOLD)),
                        ])
                    } else {
                        Line::raw(format!("  {name} x{n}"))
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    f.render_widget(Paragraph::new(lines).block(theme::pane(" Items ", false)), right);
}

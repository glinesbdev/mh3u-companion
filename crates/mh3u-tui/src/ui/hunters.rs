//! The hunter list popup.

use super::*;

/// The popup that lists the save slots that hold a hunter.
pub(super) fn draw_hunter_choice(f: &mut Frame, app: &mut App) {
    let Some(mut choice) = app.hunter_choice.take() else { return };
    let area = f.area();
    let rows: Vec<ListItem> = choice
        .slots
        .iter()
        .map(|(slot, name, played)| {
            let current = Some(*slot) == app.slot_shown();
            ListItem::new(Line::from(vec![
                Span::styled(format!("slot {slot}  "), muted()),
                Span::styled(format!("{name:<14}"), if current { good() } else { Style::new() }),
                Span::styled(format!("{played}  "), muted()),
                Span::styled(if current { "(shown)" } else { "" }, muted()),
            ]))
        })
        .collect();
    let (w, h) = (52.min(area.width), (rows.len() as u16 + 2).min(area.height));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, popup);
    f.render_stateful_widget(
        List::new(rows)
            .block(theme::pane(" Show which hunter? · Enter · Esc ", true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        popup,
        &mut choice.state,
    );
    app.hunter_choice = Some(choice);
}

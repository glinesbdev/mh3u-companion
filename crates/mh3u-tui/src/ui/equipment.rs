//! The Equipment tab.

use super::*;

pub(super) fn draw_equipment(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 45);
    let rows: Vec<ListItem> = app
        .equip_view
        .iter()
        .map(|&i| {
            let e = &app.save.equipment_box[i];
            let worn = if app.save.is_worn(e) {
                Span::styled("●", good())
            } else {
                Span::raw("")
            };
            let rarity = match app.game.equipment_rarity(e.kind, e.id) {
                Some(r) => theme::rarity_badge(r),
                None => Span::raw("   "),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<14} ", app.game.equipment_kind_label(e.kind).unwrap_or("?")), muted()),
                Span::raw(format!("{:<22}", app.game.equipment_name(e.kind, e.id).unwrap_or("?"))),
                rarity,
                Span::raw(" "),
                worn,
            ]))
        })
        .collect();
    let title = format!(" Equipment Box ({}/1000) · {} ", rows.len(), app.equip_sort.label());
    if rows.is_empty() {
        empty_pane(f, left, title, true, vec![Line::styled("The equipment box is empty.", muted())]);
        f.render_widget(Paragraph::new(Vec::<Line>::new()).block(theme::pane(" Details ", false)), right);
        return;
    }
    let len = rows.len();
    f.render_stateful_widget(
        List::new(rows)
            .block(theme::pane(title, true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        left,
        &mut app.equip_state,
    );
    scrollbar(f, left, len, app.equip_state.selected());

    let lines = match app.selected_equipment() {
        Some(e) => {
            let name = app.game.equipment_name(e.kind, e.id).unwrap_or("?").to_string();
            let mut lines = piece_details(app, e.kind, e.id, &name, None);
            for (id, pts) in e.talisman_skills() {
                lines.insert(
                    2,
                    Line::from(vec![
                        Span::raw(format!("  {:<18}", app.game.skill_name(id).unwrap_or("?"))),
                        Span::styled(format!("{pts:+}"), theme::signed_style(i32::from(pts))),
                    ]),
                );
            }
            if app.save.is_worn(e) {
                lines.insert(2, Line::styled("● worn", good()));
            }
            lines
        }
        None => Vec::new(),
    };
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Details (pouch + box) ", false)),
        right,
    );
}

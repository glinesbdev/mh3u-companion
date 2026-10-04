//! The Equipment tab.

use super::*;

pub(super) fn draw_equipment(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 45);
    let rows: Vec<ListItem> = app
        .inv
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
    let query = app.inv.equip_search.trim();
    let title = if query.is_empty() {
        format!(" Equipment Box ({}/1000) · {} ", rows.len(), app.inv.equip_sort.label())
    } else {
        format!(
            " Equipment · \"{query}\" ({}/{}) · {} ",
            rows.len(),
            app.save.equipment_box.len(),
            app.inv.equip_sort.label()
        )
    };
    if rows.is_empty() {
        let text = if query.is_empty() {
            "The equipment box is empty."
        } else {
            "No equipment matches."
        };
        empty_pane(f, left, title, true, vec![Line::styled(text, muted())]);
        f.render_widget(Paragraph::new(Vec::<Line>::new()).block(theme::pane(" Details ", false)), right);
        return;
    }
    let len = rows.len();
    render_list(
        f,
        List::new(rows)
            .block(theme::pane(title, true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        left,
        &mut app.inv.equip_state,
    );
    scrollbar(f, left, len, app.inv.equip_state.selected());

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
            if app.console.enabled {
                // for the debug commands `equip` and `talisman`: where the record is and what its 16 bytes are
                let mut bytes = vec![e.kind, e.upgrade];
                bytes.extend(e.id.to_be_bytes());
                bytes.extend(e.raw_tail);
                let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
                lines.push(Line::raw(""));
                lines.push(Line::styled(format!("slot {} · {}", e.slot, hex.join(" ")), muted()));
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

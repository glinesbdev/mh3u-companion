//! The Skills tab.

use super::*;
use mh3u_app::app::WithSkill;

/// Skills on the left; on the right every armor piece that has the highlighted skill, the most points first.
pub(super) fn draw_skills(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 30);
    let rows: Vec<ListItem> = app
        .skills
        .rows
        .iter()
        .map(|&id| {
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<20}", app.game.skill_name(id).unwrap_or("?"))),
                Span::styled(format!("{}", app.skills.pieces_with(id).len()), muted()),
            ]))
        })
        .collect();
    let mut title = format!(" Skills ({}) ", rows.len());
    if !app.skills.search.is_empty() {
        title = format!(" \"{}\" ({}) ", app.skills.search, rows.len());
    }
    let len = rows.len();
    if rows.is_empty() {
        empty_pane(f, left, title, true, vec![Line::styled("No skill matches.", muted())]);
    } else {
        render_list(
            f,
            List::new(rows)
                .block(theme::pane(title, true))
                .highlight_style(theme::selection())
                .highlight_symbol(theme::SELECTION_MARK),
            left,
            &mut app.skills.state,
        );
        scrollbar(f, left, len, app.skills.state.selected());
    }

    let lines = app.skills.selected().map(|id| piece_lines(app, id)).unwrap_or_default();
    let visible = usize::from(right.height.saturating_sub(2));
    let total = lines.len();
    let scroll = usize::from(app.skills.scroll).min(total.saturating_sub(visible));
    app.skills.scroll = scroll as u16;
    let mut title = " Armor with it ".to_string();
    if app.skills.reachable_only {
        title.push_str("· owned or on offer ");
    }
    f.render_widget(
        Paragraph::new(lines).scroll((scroll as u16, 0)).block(theme::pane(title, false)),
        right,
    );
    scrollbar(f, right, total, Some(scroll));
}

/// The highlighted skill and the armor that has it.
fn piece_lines(app: &App, skill: u8) -> Vec<Line<'static>> {
    let mut lines = vec![Line::styled(app.game.skill_name(skill).unwrap_or("?").to_string(), bold())];
    if let Some(text) = app.game.skill_description(skill) {
        lines.push(Line::styled(text.to_string(), muted()));
    }
    lines.push(Line::from(vec![
        Span::styled("● ", good()),
        Span::styled("owned   ", muted()),
        Span::styled(theme::anvil().content.trim().to_string(), accent()),
        Span::styled(" on offer   ", muted()),
        Span::styled("○ ", muted()),
        Span::styled("not yet.   ", muted()),
        Span::styled("Enter", accent().add_modifier(Modifier::BOLD)),
        Span::styled(" builds for it", muted()),
    ]));
    lines.push(Line::raw(""));
    let anvil = theme::anvil().content.trim().to_string();
    for &WithSkill { kind, id, points } in app.skills.pieces_with(skill) {
        let owned = app.save.owns_equipment(kind, id);
        let offered = !owned && app.at_blacksmith(kind, id);
        if app.skills.reachable_only && !owned && !offered {
            continue;
        }
        let mark = if owned {
            Span::styled("●", good())
        } else if offered {
            Span::styled(anvil.clone(), accent())
        } else {
            Span::styled("○", muted())
        };
        let rarity = app
            .game
            .equipment_rarity(kind, id)
            .map_or_else(|| Span::raw("   "), theme::rarity_badge);
        lines.push(Line::from(vec![
            Span::styled(format!("{points:+4}  "), theme::signed_style(i32::from(points))),
            mark,
            Span::raw(format!(" {:<26}", fit(app.game.equipment_name(kind, id).unwrap_or("?"), 26))),
            rarity,
            Span::styled(format!(" {}", mh3u_app::templates::slot_label(kind)), muted()),
        ]));
    }
    lines
}

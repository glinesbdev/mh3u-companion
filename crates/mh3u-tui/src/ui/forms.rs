//! The edit menu and the talisman form (feature `edit`).

use super::*;
use mh3u_app::app::{EDITORS, ROW_MAKE, ROW_SLOTS};
use mh3u_app::input::Key;

fn popup_in(area: Rect, w: u16, h: u16) -> Rect {
    let (w, h) = (w.min(area.width), h.min(area.height));
    Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h)
}

pub(super) fn draw_edit_menu(f: &mut Frame, app: &mut App) {
    let Some(mut state) = app.edit_menu.take() else { return };
    let popup = popup_in(f.area(), 40, EDITORS.len() as u16 + 2);
    f.render_widget(Clear, popup);
    let rows: Vec<ListItem> = EDITORS.iter().map(|name| ListItem::new(*name)).collect();
    render_list(
        f,
        List::new(rows)
            .block(theme::pane(" Edit · Enter · Esc ", true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        popup,
        &mut state,
    );
    app.edit_menu = Some(state);
}

/// A `−` and a `+` button that press `Left` and `Right`, drawn at `at` (two cells each, with a space between).
fn plus_minus(at: Rect) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (i, (text, key)) in [(" − ", Key::Left), (" + ", Key::Right)].into_iter().enumerate() {
        let x = at.x + (i as u16) * 4;
        spans.push(Span::styled(text, accent().add_modifier(Modifier::REVERSED)));
        spans.push(Span::raw(" "));
        HITS.with(|h| h.borrow_mut().actions.push((HitArea::new(x, at.y, 3, 1), None, key)));
    }
    spans
}

pub(super) fn draw_talisman(f: &mut Frame, app: &mut App) {
    let Some(mut form) = app.talisman.take() else { return };
    let popup = popup_in(f.area(), 64, 28);
    f.render_widget(Clear, popup);
    let title = if form.find.is_some() {
        " Talisman · type to find a skill · Enter choose · Esc back "
    } else {
        " Talisman · ↑ ↓ row · ← → change · Enter · Esc close "
    };
    f.render_widget(theme::pane(title, true), popup);
    let [rows_area, buttons, rest] =
        Layout::vertical([Constraint::Length(4), Constraint::Length(1), Constraint::Min(1)]).areas(popup.inner(Margin::new(1, 1)));

    let selected_row = form.rows.selected().unwrap_or(0);
    let rows: Vec<ListItem> = (0..=ROW_MAKE)
        .map(|row| {
            if row == ROW_MAKE {
                return ListItem::new(Line::styled("Make the talisman", good().add_modifier(Modifier::BOLD)));
            }
            if row == ROW_SLOTS {
                let gems: String = (0..3).map(|i| if i < form.slots { '◆' } else { '◇' }).collect();
                return ListItem::new(Line::from(vec![
                    Span::styled(format!("{:<10}", "Gem slots"), muted()),
                    Span::raw(format!("{gems}  {}", form.slots)),
                ]));
            }
            let label = Span::styled(format!("{:<10}", format!("Skill {}", row + 1)), muted());
            match form.skills[row] {
                Some((id, points)) => ListItem::new(Line::from(vec![
                    label,
                    Span::raw(format!("{:<28}", fit(app.game.skill_name(id).unwrap_or("?"), 28))),
                    Span::styled(format!("{points:+}"), if points >= 0 { good() } else { bad() }),
                ])),
                None => ListItem::new(Line::from(vec![label, Span::styled("none (Enter to choose)", muted())])),
            }
        })
        .collect();
    render_list_plain(
        f,
        List::new(rows)
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        rows_area,
        &mut form.rows,
    );

    // the buttons change whatever the highlighted row has: a skill's points or the gem slots
    let label = match selected_row {
        _ if form.find.is_some() => "",
        r if r == ROW_SLOTS => "slots: ",
        r if r < ROW_SLOTS => "points: ",
        _ => "",
    };
    let mut spans = vec![Span::styled(label, muted())];
    if selected_row <= ROW_SLOTS && form.find.is_none() {
        let width: u16 = spans.iter().map(|s| s.content.chars().count() as u16).sum();
        spans.extend(plus_minus(Rect::new(buttons.x + width, buttons.y, 8, 1)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), buttons);

    if let Some(find) = &mut form.find {
        let [input, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(rest);
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("skill: ", accent()),
                Span::raw(format!("{}_", find.text)),
            ])),
            input,
        );
        if find.matches.is_empty() {
            f.render_widget(Paragraph::new(Line::styled("no skill matches", muted())), list);
        } else {
            let items: Vec<ListItem> = find.matches.iter().map(|(_, name)| ListItem::new(name.clone())).collect();
            render_list_plain(
                f,
                List::new(items)
                    .highlight_style(theme::selection())
                    .highlight_symbol(theme::SELECTION_MARK),
                list,
                &mut find.state,
            );
        }
    } else {
        f.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    "A talisman has up to two skills and three gem slots. It is made as a copy of a talisman you already have in the equipment box, with these skills and slots.",
                    muted(),
                ),
                Line::raw(""),
                Line::styled("Delete clears a skill.", muted()),
            ])
            .wrap(Wrap { trim: false }),
            rest,
        );
    }
    app.talisman = Some(form);
}

//! The give picker popup (feature `edit`): type to find an item or a piece of gear, choose how many, Enter gives it.

use super::*;
use mh3u_app::input::Key;

pub(super) fn draw_give(f: &mut Frame, app: &mut App) {
    let Some(mut picker) = app.give.take() else { return };
    let area = f.area();
    let (w, h) = (72.min(area.width), 26.min(area.height));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, popup);
    f.render_widget(
        theme::pane(" Give · type to find · ↑ ↓ choose · Enter give · Esc close ", true),
        popup,
    );
    let [find, amount, list] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)]).areas(popup.inner(Margin::new(1, 1)));
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("find: ", accent()),
            Span::raw(format!("{}_", picker.text)),
        ])),
        find,
    );

    // how many, with buttons to click: [-] 5 [+] [max]
    let count = picker.count();
    let most = picker.chosen().map_or(99, |c| match c.what {
        mh3u_app::commands::GiveWhat::Item { .. } => 99,
        mh3u_app::commands::GiveWhat::Piece { .. } => 20,
    });
    let parts: [(String, Option<Key>); 6] = [
        ("how many: ".to_string(), None),
        (" − ".to_string(), Some(Key::Left)),
        (format!(" {count} "), None),
        (" + ".to_string(), Some(Key::Right)),
        ("  ".to_string(), None),
        (format!(" next: 1·10·50·{most} "), Some(Key::Tab)),
    ];
    let mut spans = Vec::new();
    let mut x = amount.x;
    for (text, key) in parts {
        let width = text.chars().count() as u16;
        if let Some(key) = key {
            spans.push(Span::styled(text, accent().add_modifier(Modifier::REVERSED)));
            HITS.with(|h| h.borrow_mut().actions.push((HitArea::new(x, amount.y, width, 1), None, key)));
        } else {
            spans.push(Span::raw(text));
        }
        x += width;
    }
    f.render_widget(Paragraph::new(Line::from(spans)), amount);

    if picker.choices.is_empty() {
        f.render_widget(Paragraph::new(Line::styled("nothing matches", muted())), list);
    } else {
        let rows: Vec<ListItem> = picker
            .choices
            .iter()
            .map(|c| {
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{:<30}", fit(&c.name, 30))),
                    Span::styled(c.kind.clone(), muted()),
                ]))
            })
            .collect();
        render_list_plain(
            f,
            List::new(rows)
                .highlight_style(theme::selection())
                .highlight_symbol(theme::SELECTION_MARK),
            list,
            &mut picker.state,
        );
    }
    app.give = Some(picker);
}

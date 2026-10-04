//! The Quests tab.

use super::*;
use mh3u_core::quest::Quest;

/// Quests on the left; the highlighted one in full on the right.
pub(super) fn draw_quests(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 42);
    let quests = app.game.quests();
    let rows: Vec<ListItem> = app
        .quests
        .rows
        .iter()
        .map(|r| {
            let q = &quests[r.quest];
            let mut spans = vec![
                Span::styled(format!("{:<2}", stars_text(q.stars)), warn()),
                Span::raw(format!(" {:<22}", fit(&one_line(&q.title), 22))),
                Span::styled(format!(" {:<26}", fit(&one_line(&q.goal), 26)), muted()),
            ];
            if r.needed > 0 {
                spans.push(Span::styled(format!("★ {}", r.needed), warn()));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let mut title = format!(" Quests ({}) · {} ", rows.len(), app.quests.sort.label());
    if !app.quests.search.is_empty() {
        title = format!(" \"{}\" ({}) · {} ", app.quests.search, rows.len(), app.quests.sort.label());
    }
    let len = rows.len();
    if rows.is_empty() {
        empty_pane(f, left, title, true, vec![Line::styled("No quest matches.", muted())]);
    } else {
        render_list(
            f,
            List::new(rows)
                .block(theme::pane(title, true))
                .highlight_style(theme::selection())
                .highlight_symbol(theme::SELECTION_MARK),
            left,
            &mut app.quests.state,
        );
        scrollbar(f, left, len, app.quests.state.selected());
    }

    let lines = app
        .quests
        .selected(app.game.quests())
        .map(|q| quest_lines(app, q))
        .unwrap_or_default();
    let visible = usize::from(right.height.saturating_sub(2));
    let total = lines.len();
    let scroll = usize::from(app.quests.scroll).min(total.saturating_sub(visible));
    app.quests.scroll = scroll as u16;
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll as u16, 0))
            .block(theme::pane(" The quest ", false)),
        right,
    );
    scrollbar(f, right, total, Some(scroll));
    draw_monster_choice(f, app);
}

/// `3★`, or `-` for a quest with no star rank.
fn stars_text(stars: u8) -> String {
    if stars == 0 { "-".to_string() } else { format!("{stars}★") }
}

/// Texts in the files break lines for the game's small windows; the list wants them on one.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn quest_lines(app: &App, q: &Quest) -> Vec<Line<'static>> {
    let missing = app.missing_for_wishlist();
    let mut lines = vec![
        Line::styled(one_line(&q.title), bold()),
        Line::raw(one_line(&q.goal)),
        Line::from(vec![
            Span::styled(q.kind.label(), accent()),
            Span::styled(
                format!(
                    "  ·  {}  ·  {}  ·  {} min  ·  quest {}",
                    q.place.label(),
                    stars_text(q.stars),
                    q.minutes,
                    q.id
                ),
                muted(),
            ),
        ]),
    ];
    let mut pay = vec![
        Span::styled("Reward ", muted()),
        Span::styled(format!("{}z", group_digits(u64::from(q.reward))), good()),
        Span::styled("  Fee ", muted()),
        Span::raw(format!("{}z", group_digits(u64::from(q.fee)))),
    ];
    if q.points > 0 && q.place == mh3u_core::quest::Place::Hall {
        pay.push(Span::styled(format!("  {} rank points", q.points), muted()));
    }
    lines.push(Line::from(pay));
    if let Some(map) = mh3u_core::quest::map_name(q.map) {
        lines.push(Line::from(vec![Span::styled("Map ", muted()), Span::raw(map)]));
    }
    let small: Vec<&str> = q.small_monsters.iter().filter_map(|&m| app.game.monster_name(m)).collect();
    if !small.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Small monsters ", muted()),
            Span::raw(small.join(", ")),
        ]));
    }
    if !q.monsters.is_empty() {
        let names: Vec<&str> = q.monsters.iter().filter_map(|&m| app.game.monster_name(m)).collect();
        lines.push(Line::from(vec![Span::styled("Monsters ", muted()), Span::raw(names.join(", "))]));
    }
    lines.push(Line::from(vec![Span::styled("Client ", muted()), Span::raw(one_line(&q.client))]));
    if !q.description.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled(one_line(&q.description), muted()));
    }
    for (rewards, name) in q.rewards.iter().zip(["Main reward", "Second reward"]) {
        if rewards.is_empty() {
            continue;
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(name, bold()));
        for r in rewards {
            let item = app.game.item_name(r.item).unwrap_or("?");
            let chance = if r.percent == 0 {
                Span::styled("always", good())
            } else {
                Span::raw(format!("{:>3}%", r.percent))
            };
            let mut spans = vec![Span::raw("  "), chance, Span::raw(format!("  {item} x{}", r.quantity))];
            if let Some(n) = missing.get(&r.item) {
                spans.push(Span::styled(format!("  ★ wishlist needs {n} more"), warn()));
            }
            lines.push(Line::from(spans));
        }
    }
    lines
}

/// The popup that asks which of a quest's monsters to show.
fn draw_monster_choice(f: &mut Frame, app: &mut App) {
    let Some(mut choice) = app.quests.choosing.take() else { return };
    let area = f.area();
    let rows: Vec<ListItem> = choice
        .monsters
        .iter()
        .map(|&m| ListItem::new(app.game.monster_name(m).unwrap_or("?").to_string()))
        .collect();
    let (w, h) = (36.min(area.width), (rows.len() as u16 + 2).min(area.height));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, popup);
    render_list(
        f,
        List::new(rows)
            .block(theme::pane(" Show which monster? · Enter · Esc ", true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        popup,
        &mut choice.state,
    );
    app.quests.choosing = Some(choice);
}

//! The Hunt plan tab.

use super::*;

/// The hunts that cover what the wishlist is still short of, best first, and the highlighted one in full.
pub(super) fn draw_hunts(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 40);
    let title = format!(" Hunt plan ({}) · {} ", app.hunts.plan.steps.len(), app.hunts.filter.label());
    let rows: Vec<ListItem> = app
        .hunts
        .plan
        .steps
        .iter()
        .enumerate()
        .map(|(n, step)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>2}  ", n + 1), muted()),
                Span::raw(format!("{:<18}", fit(app.game.monster_name(step.monster).unwrap_or("?"), 17))),
                Span::styled(format!("{:<10}", step.rank.label()), muted()),
                Span::styled(format!("{} item(s)", step.covers.len()), warn()),
            ]))
        })
        .collect();
    let len = rows.len();

    let unsourced = unsourced_line(app);
    let [list_area, note_area] = if unsourced.is_some() {
        Layout::vertical([Constraint::Min(5), Constraint::Length(7)]).areas(left)
    } else {
        [left, Rect::default()]
    };
    if rows.is_empty() {
        let text = if app.wish.items.is_empty() {
            "Put pieces on the wishlist (Crafting tab, w) and the hunts that get their materials are planned here."
        } else if unsourced.is_some() {
            "No monster in these ranks drops what you are missing."
        } else {
            "The wishlist has nothing missing."
        };
        empty_pane(f, list_area, title, true, vec![Line::styled(text, muted())]);
    } else {
        f.render_stateful_widget(
            List::new(rows)
                .block(theme::pane(title, true))
                .highlight_style(theme::selection())
                .highlight_symbol(theme::SELECTION_MARK),
            list_area,
            &mut app.hunts.state,
        );
        scrollbar(f, list_area, len, app.hunts.state.selected());
    }
    if let Some(lines) = unsourced {
        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .block(theme::pane(" Not from a hunt ", false)),
            note_area,
        );
    }

    let lines = hunt_details(app);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" The hunt ", false)),
        right,
    );
}

/// The materials no hunt in the allowed ranks gives, if there are any.
fn unsourced_line(app: &App) -> Option<Vec<Line<'static>>> {
    let plan = &app.hunts.plan;
    if plan.unsourced.is_empty() {
        return None;
    }
    let names: Vec<&str> = plan.unsourced.iter().map(|&i| app.game.item_name(i).unwrap_or("?")).collect();
    Some(vec![
        Line::raw(names.join(", ")),
        Line::styled(
            "Gathering, the shop, or a rank this plan leaves out (r changes the ranks).",
            muted(),
        ),
    ])
}

fn hunt_details(app: &App) -> Vec<Line<'static>> {
    let Some(step) = app.hunts.state.selected().and_then(|i| app.hunts.plan.steps.get(i)) else {
        return vec![
            Line::styled(
                "A hunt plan covers the materials your wishlist is short of with as few hunts as it can.",
                muted(),
            ),
            Line::raw(""),
            Line::from(vec![
                Span::styled("r", accent().add_modifier(Modifier::BOLD)),
                Span::styled(" limit it to one rank   ", muted()),
                Span::styled("Enter", accent().add_modifier(Modifier::BOLD)),
                Span::styled(" show the monster's drops", muted()),
            ]),
        ];
    };
    let monster = app.game.monster_name(step.monster).unwrap_or("?");
    let mut lines = vec![
        Line::from(vec![
            Span::styled(monster.to_string(), bold()),
            Span::styled(format!("  {}", step.rank.label()), muted()),
        ]),
        Line::styled(
            "The best chance for each material it gives. A carve or a break may give it more than once.",
            muted(),
        ),
        Line::raw(""),
    ];
    for cover in &step.covers {
        let item = app.game.item_name(cover.item).unwrap_or("?");
        let have = app.save.item_count(cover.item);
        lines.push(Line::from(vec![
            Span::raw(format!("  {:<24}", fit(item, 24))),
            Span::styled(format!("{:>3}%  ", cover.best.percent), good()),
            Span::raw(format!("{:<14}", cover.best.method.label())),
            Span::styled(format!("have {have}, need {} more", cover.missing), muted()),
        ]));
        // where else it comes from
        let others: Vec<String> = app
            .game
            .drops()
            .sources(cover.item)
            .iter()
            .filter(|s| (s.monster, s.rank) != (step.monster, step.rank))
            .filter_map(|s| Some((app.game.monster_name(s.monster)?, s)))
            .map(|(name, s)| format!("{name} ({}, {}%)", s.rank.label().replace(" rank", ""), s.percent))
            .fold(Vec::new(), |mut acc, text| {
                if !acc.contains(&text) {
                    acc.push(text);
                }
                acc
            });
        if !others.is_empty() {
            let shown: Vec<&str> = others.iter().take(4).map(String::as_str).collect();
            let more = others.len().saturating_sub(4);
            let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
            lines.push(Line::styled(format!("      also: {}{tail}", shown.join(", ")), muted()));
        }
    }
    lines
}

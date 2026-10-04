//! The Hunt plan tab.

use super::*;
use mh3u_app::hunts::{How, Origin};

/// The hunts that cover what the wishlist is still short of, best first, and the highlighted one in full.
pub(super) fn draw_hunts(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 50);
    let mut title = format!(" Hunt plan ({}) · {} ", app.hunts.plan.steps.len(), app.hunts.filter.label());
    if !app.hunts.plan.steps.is_empty() {
        title.push_str(&format!("· ~{} runs ", app.hunts.plan.total_runs()));
    }
    let rows: Vec<ListItem> = app
        .hunts
        .plan
        .steps
        .iter()
        .enumerate()
        .map(|(n, step)| {
            let (what, name, detail) = match step.origin {
                Origin::Monster { monster, rank } => (
                    "Hunt",
                    app.game.monster_name(monster).unwrap_or("?").to_string(),
                    rank.label().to_string(),
                ),
                Origin::Quest(id) => match app.game.quests().iter().find(|q| q.id == id) {
                    Some(q) => ("Quest", one_line(&q.title), format!("{}★", q.stars)),
                    None => ("Quest", format!("#{id}"), String::new()),
                },
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>2}  ", n + 1), muted()),
                Span::styled(format!("{what:<6}"), accent()),
                Span::raw(format!("{:<18}", fit(&name, 17))),
                Span::styled(format!("{detail:<10}"), muted()),
                Span::styled(format!("{:<8}", plural(step.covers.len() as u32, "item")), warn()),
                Span::styled(format!("~{}", plural(step.runs(), "run")), muted()),
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
            "Put pieces on the wishlist (Crafting tab, w) and the hunts and quests that get their materials are planned here."
        } else if unsourced.is_some() {
            "No monster or quest in these ranks gives what you are missing."
        } else {
            "The wishlist has nothing missing."
        };
        empty_pane(f, list_area, title, true, vec![Line::styled(text, muted())]);
    } else {
        render_list(
            f,
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
                .block(theme::pane(" Not from a hunt or quest ", false)),
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

/// `1 run`, `3 runs`.
fn plural(n: u32, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
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
            "Gathering, the shop, or a rank this plan leaves out (quests only count when every rank is allowed; r changes the ranks).",
            muted(),
        ),
    ])
}

fn hunt_details(app: &App) -> Vec<Line<'static>> {
    let Some(step) = app.hunts.state.selected().and_then(|i| app.hunts.plan.steps.get(i)) else {
        return vec![
            Line::styled(
                "A hunt plan covers the materials your wishlist is short of with as few hunts and quests as it can.",
                muted(),
            ),
            Line::raw(""),
            Line::from(vec![
                Span::styled("r", accent().add_modifier(Modifier::BOLD)),
                Span::styled(" limit it to one rank   ", muted()),
                Span::styled("Enter", accent().add_modifier(Modifier::BOLD)),
                Span::styled(" show the monster's drops or the quest", muted()),
            ]),
        ];
    };
    let mut lines = match step.origin {
        Origin::Monster { monster, rank } => vec![
            Line::from(vec![
                Span::styled(app.game.monster_name(monster).unwrap_or("?").to_string(), bold()),
                Span::styled(format!("  {}", rank.label()), muted()),
            ]),
            Line::styled(
                "The way that gives each material in the fewest runs. A run is one hunt: the estimate assumes 3 carves, 1 tail carve, 2 capture rolls and 1 roll per break.",
                muted(),
            ),
            Line::raw(""),
        ],
        Origin::Quest(id) => quest_heading(app, id),
    };
    for cover in &step.covers {
        let item = app.game.item_name(cover.item).unwrap_or("?");
        let have = app.save.item_count(cover.item);
        let (chance, how) = match cover.how {
            How::Drop(method) => (Span::styled(format!("{:>3}%  ", cover.percent), good()), method.label()),
            How::Reward { second_box, quantity } => (
                if cover.percent == 0 {
                    Span::styled("always ", good())
                } else {
                    Span::styled(format!("{:>3}%  ", cover.percent), good())
                },
                format!("{} x{quantity}", if second_box { "second reward" } else { "main reward" }),
            ),
        };
        lines.push(Line::from(vec![
            Span::raw(format!("  {:<22}", fit(item, 22))),
            chance,
            Span::raw(format!("{how:<17}")),
            Span::styled(cover.runs.map_or(String::new(), |r| format!("~{}", plural(r, "run"))), warn()),
        ]));
        lines.push(Line::styled(format!("      have {have}, need {} more", cover.missing), muted()));
        // where else it comes from
        let mut best: Vec<((String, &'static str), u8)> = Vec::new();
        for s in app.game.drops().sources(cover.item) {
            if step.origin
                == (Origin::Monster {
                    monster: s.monster,
                    rank: s.rank,
                })
            {
                continue;
            }
            let Some(name) = app.game.monster_name(s.monster) else { continue };
            let key = (name.to_string(), s.rank.label());
            match best.iter_mut().find(|(k, _)| *k == key) {
                Some((_, p)) => *p = (*p).max(s.percent),
                None => best.push((key, s.percent)),
            }
        }
        let others: Vec<String> = best
            .into_iter()
            .map(|((name, rank), p)| format!("{name} ({}, {p}%)", rank.replace(" rank", "")))
            .collect();
        if !others.is_empty() {
            let shown: Vec<&str> = others.iter().take(4).map(String::as_str).collect();
            let more = others.len().saturating_sub(4);
            let tail = if more > 0 { format!(" and {more} more") } else { String::new() };
            lines.push(Line::styled(format!("      also: {}{tail}", shown.join(", ")), muted()));
        }
    }
    lines
}

/// Texts in the quest files break lines for the game's small windows; the screen wants them on one.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The top of a quest step's details: its title, goal, kind and monsters.
fn quest_heading(app: &App, id: u16) -> Vec<Line<'static>> {
    let Some(q) = app.game.quests().iter().find(|q| q.id == id) else {
        return vec![Line::styled(format!("Quest {id}"), bold()), Line::raw("")];
    };
    let monsters: Vec<&str> = q.monsters.iter().filter_map(|&m| app.game.monster_name(m)).collect();
    let mut lines = vec![
        Line::from(vec![
            Span::styled(one_line(&q.title), bold()),
            Span::styled(format!("  {}★  {} min", q.stars, q.minutes), muted()),
        ]),
        Line::raw(one_line(&q.goal)),
    ];
    if !monsters.is_empty() {
        lines.push(Line::from(vec![Span::styled("Monsters ", muted()), Span::raw(monsters.join(", "))]));
    }
    lines.push(Line::styled(
        "The best way to get each material among its rewards. The runs assume the main box is rolled 3 times and the second box once.",
        muted(),
    ));
    lines.push(Line::raw(""));
    lines
}

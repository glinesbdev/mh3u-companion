//! The Builds tab: the wanted skills, the sets found and the templates, with the popups that edit them.

use super::*;

/// The build manager: the skills you want, the sets that reach them, the saved templates, and the highlighted set in full.
pub(super) fn draw_builds(f: &mut Frame, app: &mut App, area: Rect) {
    use crate::app::BuildFocus;
    let [left, right] = theme::split(area, 42);
    let wanted = app.builds.settings.targets.len();
    let targets_height = (wanted as u16 + 2).clamp(4, 9);
    let templates_height = (app.builds.templates.len() as u16 + 2).clamp(4, 8);
    let [top, middle, bottom] = Layout::vertical([
        Constraint::Length(targets_height),
        Constraint::Min(5),
        Constraint::Length(templates_height),
    ])
    .areas(left);

    let rows: Vec<ListItem> = if app.builds.settings.targets.is_empty() {
        vec![ListItem::new(Line::styled("press a to add a skill", muted()))]
    } else {
        app.builds
            .settings
            .targets
            .iter()
            .map(|t| {
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{:<20}", app.game.skill_name(t.skill).unwrap_or("?"))),
                    Span::styled(format!("{:+}", t.points), good()),
                ]))
            })
            .collect()
    };
    f.render_stateful_widget(
        focused_list(rows, " Skills wanted ".to_string(), app.builds.focus == BuildFocus::Skills),
        top,
        &mut app.builds.target_state,
    );

    let mut pool = String::from(app.builds.settings.pool.label());
    if app.builds.settings.use_talisman {
        pool += " · talisman";
    }
    if let Some(g) = app.builds.settings.gender {
        pool += &format!(" · {}", g.label().to_lowercase());
    }
    if let Some(c) = app.builds.settings.class {
        pool += &format!(" · {}", c.label().to_lowercase());
    }
    let rows: Vec<ListItem> = app
        .builds
        .results
        .iter()
        .enumerate()
        .map(|(n, found)| {
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:>3}  ", n + 1), muted()),
                Span::raw(format!("Def {:>3}  ", found.defense)),
                Span::styled(
                    format!("{}/{} owned", found.owned, found.pieces.len()),
                    if found.owned == found.pieces.len() { good() } else { muted() },
                ),
            ]))
        })
        .collect();
    let len = rows.len();
    f.render_stateful_widget(
        focused_list(rows, format!(" Sets ({len}) · {pool} "), app.builds.focus == BuildFocus::Sets),
        middle,
        &mut app.builds.result_state,
    );
    scrollbar(f, middle, len, app.builds.result_state.selected());

    let rows: Vec<ListItem> = if app.builds.templates.is_empty() {
        vec![ListItem::new(Line::styled("s on a set, or n for what you wear", muted()))]
    } else {
        app.builds
            .templates
            .iter()
            .map(|t| {
                let have = t.pieces.iter().filter(|p| app.owns_slot(p.kind, p.id)).count();
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{:<24}", fit(&t.name, 24))),
                    Span::styled(
                        format!("{have}/{} owned", t.pieces.len()),
                        if have == t.pieces.len() { good() } else { muted() },
                    ),
                ]))
            })
            .collect()
    };
    let count = app.builds.templates.len();
    f.render_stateful_widget(
        focused_list(rows, format!(" Templates ({count}) "), app.builds.focus == BuildFocus::Templates),
        bottom,
        &mut app.builds.template_state,
    );

    let (title, lines) = if app.builds.focus == BuildFocus::Templates {
        template_details(app)
    } else {
        (" The set ".to_string(), build_details(app))
    };
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(theme::pane(title, false)),
        right,
    );
}

/// One slot of a set to show: the piece and its stats.
pub(super) struct Shown {
    id: u16,
    stats: mh3u_core::armor::ArmorStats,
}

/// The slots of a set piece by piece (owned, or what it takes to make it), then what the set adds up to. `cursor` marks a slot.
pub(super) fn set_lines(app: &App, slots: &[Option<Shown>; 6], cursor: Option<usize>) -> Vec<Line<'static>> {
    let zenny = app.save.zenny;
    let mut lines: Vec<Line> = Vec::new();
    let mut parts: Vec<(u8, &mh3u_core::armor::ArmorStats)> = Vec::new();
    for (n, (&kind, shown)) in crate::templates::SLOTS.iter().zip(slots).enumerate() {
        let marker = if cursor == Some(n) {
            Span::styled("▶ ", accent())
        } else {
            Span::raw("  ")
        };
        let label = Span::styled(format!("{:<9}", crate::templates::slot_label(kind)), muted());
        let Some(c) = shown else {
            lines.push(Line::from(vec![marker, label, Span::styled("nothing", muted())]));
            continue;
        };
        parts.push((kind, &c.stats));
        let name = app.game.equipment_name(kind, c.id).unwrap_or("Talisman");
        let mut spans = vec![marker, label, Span::styled(format!("{:<24}", fit(name, 24)), bold())];
        if app.owns_slot(kind, c.id) {
            spans.push(Span::styled("● owned", good()));
        } else {
            spans.push(theme::anvil());
            let offered = app.at_blacksmith(kind, c.id);
            if app.can_make_now(kind, c.id) {
                spans.push(Span::styled("can make now", good()));
            } else if offered {
                spans.push(Span::styled("needs materials", warn()));
            } else {
                spans.push(Span::styled("not on offer yet", bad()));
            }
            if let Some((cost, _)) = app.cost(kind, c.id, Route::Create) {
                spans.push(Span::styled(
                    format!("  {} z", group_digits(u64::from(cost))),
                    if cost <= zenny { muted() } else { bad() },
                ));
            }
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::raw(""));
    let summary = crate::worn::summarize(&parts);
    lines.extend(totals_lines(app, &summary, &app.builds.settings.targets));
    lines
}

/// The highlighted set that was found, in full.
pub(super) fn build_details(app: &App) -> Vec<Line<'static>> {
    if app.builds.settings.targets.is_empty() {
        return vec![
            Line::styled("Pick the skills you want and the sets that reach them are listed.", muted()),
            Line::raw(""),
            Line::from(vec![
                Span::styled("a", accent().add_modifier(Modifier::BOLD)),
                Span::styled(" add a skill (first at 10 points, + and - change it)", muted()),
            ]),
            Line::from(vec![
                Span::styled("f", accent().add_modifier(Modifier::BOLD)),
                Span::styled(" switch between the skills, the sets and your templates", muted()),
            ]),
        ];
    }
    let Some(found) = app.builds.result_state.selected().and_then(|i| app.builds.results.get(i)) else {
        return vec![
            Line::styled("No set reaches all of those skills.", warn()),
            Line::raw(""),
            Line::styled(
                format!(
                    "{} pieces were looked at. Try fewer points or skills, o to widen the pieces (on offer, everything), m for a talisman, e and c to drop the gender and class filters.",
                    app.builds.pool.len()
                ),
                muted(),
            ),
        ];
    };
    let mut slots: [Option<Shown>; 6] = Default::default();
    for c in found.pieces.iter().map(|&i| &app.builds.pool[i]) {
        if let Some(n) = crate::templates::SLOTS.iter().position(|&k| k == c.kind) {
            slots[n] = Some(Shown {
                id: c.id,
                stats: c.stats.clone(),
            });
        }
    }
    let mut lines = set_lines(app, &slots, None);
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Defense is the pieces' base defense. Decorations are not counted. s saves the set as a template, w puts the pieces you do not own on the wishlist.",
        muted(),
    ));
    lines
}

/// The highlighted template: its slots (with the cursor), then the totals.
pub(super) fn template_details(app: &App) -> (String, Vec<Line<'static>>) {
    let Some(t) = app.builds.template_state.selected().and_then(|i| app.builds.templates.get(i)) else {
        return (
            " Template ".to_string(),
            vec![
                Line::styled("No templates yet.", muted()),
                Line::raw(""),
                Line::styled(
                    "A template is a named set you keep. Save a found set with s (in the sets list), or save what you are wearing with n here.",
                    muted(),
                ),
            ],
        );
    };
    let mut slots: [Option<Shown>; 6] = Default::default();
    for p in &t.pieces {
        if let (Some(n), Some(stats)) = (crate::templates::SLOTS.iter().position(|&k| k == p.kind), app.piece_stats(p)) {
            slots[n] = Some(Shown { id: p.id, stats });
        }
    }
    let mut lines = set_lines(app, &slots, Some(app.builds.template_slot));
    lines.push(Line::raw(""));
    lines.push(Line::from(theme::key_hints(&[
        ("[ ]", "slot"),
        ("Enter", "swap the piece"),
        ("w", "wish all"),
        ("W", "wish this piece"),
        ("r", "rename"),
        ("x", "delete"),
    ])));
    (format!(" {} ", t.name), lines)
}

/// A one-line text prompt in the middle of the screen.
pub(super) fn draw_name_prompt(f: &mut Frame, app: &App) {
    use crate::app::NameAction;
    let Some(prompt) = &app.builds.name_prompt else { return };
    let area = f.area();
    let (w, h) = (50.min(area.width), 3.min(area.height));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    let title = match prompt.action {
        NameAction::Rename(_) => " Rename the template · Enter · Esc ",
        _ => " Name the template · Enter · Esc ",
    };
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("name: ", accent()),
            Span::raw(format!("{}_", prompt.text)),
        ]))
        .block(theme::pane(title, true)),
        popup,
    );
}

/// The popup that picks the piece for one template slot.
pub(super) fn draw_piece_picker(f: &mut Frame, app: &mut App) {
    let Some(mut picker) = app.builds.piece_picker.take() else { return };
    let area = f.area();
    let (w, h) = (80.min(area.width), 22.min(area.height));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, popup);
    let [input, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(popup.inner(Margin::new(1, 1)));
    f.render_widget(
        theme::pane(
            format!(
                " {} · type to find · Enter choose · Esc close ",
                crate::templates::slot_label(picker.kind)
            ),
            true,
        ),
        popup,
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("find: ", accent()),
            Span::raw(format!("{}_", picker.text)),
        ])),
        input,
    );
    let rows: Vec<ListItem> = picker
        .choices
        .iter()
        .map(|c| {
            let status = match c.status {
                "owned" => Span::styled("● owned   ", good()),
                "on offer" => Span::styled("on offer ", warn()),
                _ => Span::raw("         "),
            };
            ListItem::new(Line::from(vec![
                status,
                Span::raw(format!("{:<22}", fit(&c.name, 22))),
                Span::styled(fit(&c.detail, (w as usize).saturating_sub(40)), muted()),
            ]))
        })
        .collect();
    f.render_stateful_widget(
        List::new(rows)
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        list,
        &mut picker.state,
    );
    app.builds.piece_picker = Some(picker);
}

/// The popup that finds a skill by typing part of its name.
pub(super) fn draw_skill_picker(f: &mut Frame, app: &mut App) {
    let Some(mut picker) = app.builds.skill_picker.take() else { return };
    let matches = app.skill_matches(&picker.text);
    let area = f.area();
    let (w, h) = (44.min(area.width), 18.min(area.height));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, popup);
    let [input, list] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(popup.inner(Margin::new(1, 1)));
    f.render_widget(theme::pane(" Add a skill · Enter add · Esc close ", true), popup);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("skill: ", accent()),
            Span::raw(format!("{}_", picker.text)),
        ])),
        input,
    );
    let rows: Vec<ListItem> = matches
        .iter()
        .map(|&id| {
            let mut spans = vec![Span::raw(app.game.skill_name(id).unwrap_or("?").to_string())];
            if app.builds.settings.targets.iter().any(|t| t.skill == id) {
                spans.push(Span::styled("  (already wanted)", muted()));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let last = matches.len().saturating_sub(1);
    if let Some(i) = picker.state.selected() {
        picker.state.select(Some(i.min(last)));
    }
    f.render_stateful_widget(
        List::new(rows)
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        list,
        &mut picker.state,
    );
    app.builds.skill_picker = Some(picker);
}

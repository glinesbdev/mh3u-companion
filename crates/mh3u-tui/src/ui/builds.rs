//! The Builds tab: the wanted skills, the sets found and the templates, with the popups that edit them.

use super::*;
use mh3u_app::input::Key;

/// The build manager: the skills you want, the sets that reach them, the saved templates, and the highlighted set in full.
pub(super) fn draw_builds(f: &mut Frame, app: &mut App, area: Rect) {
    use mh3u_app::app::BuildFocus;
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
        vec![ListItem::new(Line::styled("click here or press a to add a skill", muted()))]
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
    render_list_for(
        f,
        focused_list(rows, " Skills wanted ".to_string(), app.builds.focus == BuildFocus::Skills),
        top,
        &mut app.builds.target_state,
        Some((Focus::Builds(BuildFocus::Skills), app.builds.focus == BuildFocus::Skills)),
    );
    if app.builds.settings.targets.is_empty() {
        click_to_press(top, None, Key::Char('a'));
    }

    let mut pool = String::from(app.builds.settings.pool.label());
    if app.builds.settings.use_talisman {
        pool += " · talisman";
    }
    if let Some(g) = app.builds.settings.gender {
        pool += &format!(" · {}", g.label().to_lowercase());
    }
    if app.builds.settings.rank != mh3u_app::builds::Rank::Defense {
        pool += &format!(" · by {}", app.builds.settings.rank.label());
    }
    if app.builds.settings.craftable_only {
        pool += " · make now";
    }
    if let Some(r) = app.builds.settings.max_rarity {
        pool += &format!(" · rarity ≤ {r}");
    }
    if let Some((kind, id)) = app.builds.settings.weapon {
        let class = mh3u_app::builds::weapon_class(kind).label().to_lowercase();
        pool += &format!(" · for {} ({class})", app.game.equipment_name(kind, id).unwrap_or("?"));
    } else if let Some(c) = app.builds.settings.class {
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
                Span::raw(match app.builds.settings.rank {
                    mh3u_app::builds::Rank::Slots => format!("Slots {:>2}  ", found.slots),
                    mh3u_app::builds::Rank::Resist => format!("Res {:>+3}  ", found.resist),
                    _ => String::new(),
                }),
                Span::styled(
                    format!("{}/{} owned", found.owned, found.pieces.len()),
                    if found.owned == found.pieces.len() { good() } else { muted() },
                ),
            ]))
        })
        .collect();
    let len = rows.len();
    render_list_for(
        f,
        focused_list(rows, format!(" Sets ({len}) · {pool} "), app.builds.focus == BuildFocus::Sets),
        middle,
        &mut app.builds.result_state,
        Some((Focus::Builds(BuildFocus::Sets), app.builds.focus == BuildFocus::Sets)),
    );
    scrollbar(f, middle, len, app.builds.result_state.selected());

    let rows: Vec<ListItem> = if app.builds.templates.is_empty() {
        vec![ListItem::new(Line::styled(
            "click here or press n to save what you wear (s saves a found set)",
            muted(),
        ))]
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
    render_list_for(
        f,
        focused_list(rows, format!(" Templates ({count}) "), app.builds.focus == BuildFocus::Templates),
        bottom,
        &mut app.builds.template_state,
        Some((Focus::Builds(BuildFocus::Templates), app.builds.focus == BuildFocus::Templates)),
    );
    if app.builds.templates.is_empty() {
        click_to_press(bottom, Some(Focus::Builds(BuildFocus::Templates)), Key::Char('n'));
    }

    let (title, lines) = if app.builds.focus == BuildFocus::Templates {
        template_details(app)
    } else {
        (" The set ".to_string(), build_details(app))
    };
    // the template's slot rows can be clicked: each is a line of the pane, which may have wrapped
    if app.builds.focus == BuildFocus::Templates && app.builds.template_state.selected().is_some() {
        let inner = right.inner(Margin {
            vertical: 1,
            horizontal: 1,
        });
        let wide = usize::from(inner.width).max(1);
        let mut y = inner.y;
        for (n, line) in lines.iter().take(mh3u_app::templates::Slot::ALL.len()).enumerate() {
            let height = line.width().div_ceil(wide).max(1) as u16;
            if y + height > inner.y + inner.height {
                break;
            }
            HITS.with(|h| h.borrow_mut().slots.push((HitArea::new(inner.x, y, inner.width, height), n)));
            y += height;
        }
    }
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(theme::pane(title, false)),
        right,
    );
}

/// Make the first row of a list pane (inside its border) do what pressing `key` does when clicked.
fn click_to_press(pane: Rect, focus: Option<Focus>, key: Key) {
    let row = HitArea::new(pane.x + 1, pane.y + 1, pane.width.saturating_sub(2), 1);
    HITS.with(|h| h.borrow_mut().actions.push((row, focus, key)));
}

/// One slot of a set to show: the piece and its stats.
pub(super) struct Shown {
    kind: u8,
    id: u16,
    /// What the piece adds to a set's totals; a weapon adds none.
    stats: Option<mh3u_core::armor::ArmorStats>,
}

/// The slots of a set piece by piece (owned, or what it takes to make it), then what the set adds up to. `cursor` marks a slot.
pub(super) fn set_lines(app: &App, slots: &[Option<Shown>; 7], cursor: Option<usize>) -> Vec<Line<'static>> {
    let zenny = app.save.zenny;
    let mut lines: Vec<Line> = Vec::new();
    let mut parts: Vec<(u8, &mh3u_core::armor::ArmorStats)> = Vec::new();
    for (n, (slot, shown)) in mh3u_app::templates::Slot::ALL.iter().zip(slots).enumerate() {
        let marker = if cursor == Some(n) {
            Span::styled("▶ ", accent())
        } else {
            Span::raw("  ")
        };
        let label = Span::styled(format!("{:<9}", slot.label()), muted());
        let Some(c) = shown else {
            lines.push(Line::from(vec![marker, label, Span::styled("nothing", muted())]));
            continue;
        };
        let kind = c.kind;
        if let Some(stats) = &c.stats {
            parts.push((kind, stats));
        }
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
            // a weapon is got the cheapest way, which may be several steps; a piece of armor is made
            let cost = match app.cheapest_path(kind, c.id) {
                Some(path) => Some(path.zenny()),
                None => app.cost(kind, c.id, Route::Create).map(|(cost, _)| cost),
            };
            if let Some(cost) = cost {
                spans.push(Span::styled(
                    format!("  {} z", group_digits(u64::from(cost))),
                    if cost <= zenny { muted() } else { bad() },
                ));
            }
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::raw(""));
    let summary = mh3u_app::worn::summarize(&parts);
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
    let mut slots: [Option<Shown>; 7] = Default::default();
    for c in found.pieces.iter().map(|&i| &app.builds.pool[i]) {
        if let Some(slot) = mh3u_app::templates::Slot::of_kind(c.kind) {
            slots[slot.index()] = Some(Shown {
                kind: c.kind,
                id: c.id,
                stats: Some(c.stats.clone()),
            });
        }
    }
    // the weapon the set is for
    if let Some((kind, id)) = app.builds.settings.weapon {
        slots[mh3u_app::templates::Slot::Weapon.index()] = Some(Shown { kind, id, stats: None });
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
    let mut slots: [Option<Shown>; 7] = Default::default();
    for p in &t.pieces {
        if let Some(slot) = mh3u_app::templates::Slot::of_kind(p.kind) {
            slots[slot.index()] = Some(Shown {
                kind: p.kind,
                id: p.id,
                stats: app.piece_stats(p),
            });
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
    use mh3u_app::app::NameAction;
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
        theme::pane(format!(" {} · type to find · Enter choose · Esc close ", picker.slot.label()), true),
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
            let status = match c.availability {
                Availability::Owned => Span::styled("● owned   ", good()),
                Availability::OnOffer => Span::styled("on offer ", warn()),
                Availability::Unavailable => Span::raw("         "),
            };
            ListItem::new(Line::from(vec![
                status,
                Span::raw(format!("{:<22}", fit(&c.name, 22))),
                Span::styled(fit(&c.detail, (w as usize).saturating_sub(40)), muted()),
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
    let title = if app.builds.settings.weapon.is_some() {
        " Add a skill · the weapon's first · Enter · Esc "
    } else {
        " Add a skill · Enter add · Esc close "
    };
    f.render_widget(theme::pane(title, true), popup);
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
            if !app.skill_fits_weapon(id) {
                spans.push(Span::styled("  (not for this weapon)", muted()));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let last = matches.len().saturating_sub(1);
    if let Some(i) = picker.state.selected() {
        picker.state.select(Some(i.min(last)));
    }
    render_list_plain(
        f,
        List::new(rows)
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        list,
        &mut picker.state,
    );
    app.builds.skill_picker = Some(picker);
}

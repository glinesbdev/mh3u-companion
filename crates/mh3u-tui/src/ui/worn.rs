//! The Worn tab and the totals shown for a set of armor.

use super::*;

use mh3u_app::app::JEWELS;

/// Defense, gem slots, resistances and the skill points of a set of armor. Skills in `targets` show whether their goal is reached.
pub(super) fn totals_lines(app: &App, summary: &mh3u_app::worn::Summary, targets: &[mh3u_app::builds::Target]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Defense ", muted()),
        Span::styled(summary.defense.to_string(), bold()),
        Span::styled(" (base)", muted()),
        Span::styled(format!("  {} fully upgraded", summary.max_defense), muted()),
        Span::styled("   Gem slots ", muted()),
        Span::styled(summary.gem_slots.to_string(), bold()),
    ]));
    let mut resist = Vec::new();
    for (name, value) in ["Fire", "Water", "Thunder", "Ice", "Dragon"].into_iter().zip(summary.resist) {
        if !resist.is_empty() {
            resist.push(Span::raw("  "));
        }
        resist.push(Span::styled(name, theme::element_style(name)));
        resist.push(Span::styled(format!(" {value:+}"), theme::signed_style(value)));
    }
    lines.push(Line::from(resist));
    lines.push(Line::raw(""));
    if summary.skills.is_empty() {
        lines.push(Line::styled("No skill points.", muted()));
    }
    for t in &summary.skills {
        let goal = targets.iter().find(|g| g.skill == t.id);
        lines.push(skill_row(app, t, goal));
        if app.skill_info
            && let Some(text) = t
                .effect()
                .and_then(|e| app.game.effect_description(e))
                .or_else(|| app.game.skill_description(t.id))
        {
            lines.push(Line::styled(format!("    {text}"), muted()));
        }
    }
    if summary.torso_doubled {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Torso Up is active: the body piece's skill points count double.",
            muted(),
        ));
    }
    lines
}

/// One skill of the totals as a row: its name, points, what the points do (or whether a goal is met) and where they come from.
pub(super) fn skill_row(app: &App, t: &mh3u_app::worn::SkillTotal, goal: Option<&mh3u_app::builds::Target>) -> Line<'static> {
    let (state, style) = if let Some(g) = goal {
        if t.points >= g.points {
            (format!("✔ goal {}", g.points), good())
        } else {
            (format!("✘ goal {}", g.points), bad())
        }
    } else if t.active() || t.penalty() {
        let name = t.effect().and_then(|e| app.game.effect_name(e));
        match (name, t.active()) {
            (Some(n), true) => (format!("● {n}"), good()),
            (Some(n), false) => (format!("▼ {n}"), bad()),
            (None, true) => ("● active".to_string(), good()),
            (None, false) => ("▼ penalty".to_string(), bad()),
        }
    } else if t.points > 0 {
        (format!("{} more to activate", mh3u_app::worn::ACTIVE_AT - t.points), muted())
    } else {
        (String::new(), muted())
    };
    let parts: Vec<String> = t
        .parts
        .iter()
        .map(|&(kind, p)| {
            let label = if kind == JEWELS {
                "Jewels"
            } else {
                app.game.equipment_kind_label(kind).unwrap_or("?")
            };
            format!("{label} {p:+}")
        })
        .collect();
    Line::from(vec![
        Span::raw(format!("{:<18}", app.game.skill_name(t.id).unwrap_or("?"))),
        Span::styled(format!("{:+4} ", t.points), theme::signed_style(t.points)),
        Span::styled(format!("{state:<22}"), style),
        Span::styled(parts.join(", "), muted()),
    ])
}

/// The worn gear and what it adds up to.
pub(super) fn draw_worn(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 45);
    let armor = app.worn_armor();
    let talisman = app.worn_talisman();
    let summary = app.worn_summary();

    // Left: what is worn, slot by slot.
    let mut gear: Vec<Line> = Vec::new();
    if let Some(w) = app.worn_weapon() {
        let name = app.game.equipment_name(w.kind, w.id).unwrap_or("?");
        let mut spans = vec![
            Span::styled(format!("{:<8}", "Weapon"), muted()),
            Span::styled(format!("{:<24}", fit(name, 24)), bold()),
        ];
        if let Some(stats) = app.game.weapon_stats(w.kind, w.id) {
            spans.push(theme::rarity_badge(stats.rarity));
            spans.push(Span::styled(format!(" Attack {}", stats.attack), muted()));
        }
        gear.push(Line::from(spans));
        if let Some(e) = app.game.weapon_extras(w.kind, w.id) {
            let mut bar = vec![Span::raw(format!("{:<8}", ""))];
            bar.extend(super::pieces::sharpness_spans(&e.sharpness, 3));
            for s in &e.specials {
                let (note, style) = match (s.hidden, summary.awakened()) {
                    (false, _) => ("", theme::element_style(&s.name)),
                    (true, true) => (" (Awaken)", theme::element_style(&s.name)),
                    (true, false) => (" (needs Awaken)", muted()),
                };
                bar.push(Span::styled(format!("  {} {}{note}", s.name, s.value), style));
            }
            gear.push(Line::from(bar));
        }
    } else {
        gear.push(Line::from(vec![
            Span::styled(format!("{:<8}", "Weapon"), muted()),
            Span::styled("nothing", muted()),
        ]));
    }
    gear.push(Line::raw(""));
    let mut pending_jewels: Vec<String> = Vec::new();
    for (kind, label) in [(5u8, "Head"), (1, "Body"), (2, "Arms"), (3, "Waist"), (4, "Legs")] {
        let piece = armor.iter().find(|&&(k, _)| k == kind).map(|&(_, e)| e);
        let mut spans = vec![Span::styled(format!("{label:<8}"), muted())];
        match piece {
            Some(e) => {
                spans.push(Span::styled(
                    format!("{:<24}", fit(app.game.equipment_name(kind, e.id).unwrap_or("?"), 24)),
                    bold(),
                ));
                let jewels: Vec<&str> = e
                    .decorations()
                    .iter()
                    .map(|&c| app.game.decoration(c).and_then(|d| app.game.item_name(d.item)).unwrap_or("?"))
                    .collect();
                if !jewels.is_empty() {
                    pending_jewels.push(format!("  {label:<6}{}", jewels.join(", ")));
                }
                if let Some(a) = app.game.armor_stats(kind, e.id) {
                    spans.push(theme::rarity_badge(a.rarity));
                    spans.push(Span::styled(format!(" {}", theme::gems(a.slots)), accent()));
                    spans.push(Span::styled(format!("  Def {}", a.defense), muted()));
                }
            }
            None => spans.push(Span::styled("nothing", muted())),
        }
        gear.push(Line::from(spans));
    }
    let mut spans = vec![Span::styled(format!("{:<8}", "Charm"), muted())];
    match talisman {
        Some(t) => {
            spans.push(Span::styled(
                format!("{:<24}", fit(app.game.equipment_name(6, t.id).unwrap_or("Talisman"), 24)),
                bold(),
            ));
            spans.push(Span::styled(format!("    {}", theme::gems(t.talisman_slots())), accent()));
        }
        None => spans.push(Span::styled("nothing", muted())),
    }
    gear.push(Line::from(spans));
    gear.push(Line::raw(""));
    for line in pending_jewels {
        gear.push(Line::from(vec![Span::styled(line, muted())]));
    }
    if let Some(t) = talisman {
        for &code in t.talisman_decorations().iter().filter(|&&c| c != 0) {
            let name = app.game.decoration(code).and_then(|d| app.game.item_name(d.item)).unwrap_or("?");
            gear.push(Line::from(vec![Span::styled("  Charm ", muted()), Span::raw(name.to_string())]));
        }
    }
    f.render_widget(
        Paragraph::new(gear)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Worn gear ", true)),
        left,
    );

    // Right: defense and resistances, the skills (one is highlighted), and what lies behind that skill.
    let [header, list_area, detail_area] =
        Layout::vertical([Constraint::Length(4), Constraint::Min(5), Constraint::Length(13)]).areas(right);
    let mut head: Vec<Line> = totals_lines(app, &summary, &[]);
    head.truncate(2);
    f.render_widget(Paragraph::new(head).block(theme::pane(" Totals ", false)), header);

    let selected = app.worn_selected(&summary);
    app.worn.skills.select(selected);
    if summary.skills.is_empty() {
        empty_pane(
            f,
            list_area,
            " Skills ".to_string(),
            true,
            vec![Line::styled("No skill points.", muted())],
        );
    } else {
        let rows: Vec<ListItem> = summary.skills.iter().map(|t| ListItem::new(skill_row(app, t, None))).collect();
        render_list(f, focused_list(rows, " Skills ".to_string(), true), list_area, &mut app.worn.skills);
        scrollbar(f, list_area, summary.skills.len(), selected);
    }

    let (title, mut lines) = match selected.and_then(|i| summary.skills.get(i)) {
        Some(t) => skill_detail_lines(app, t),
        None => (" Skill ".to_string(), Vec::new()),
    };
    if summary.torso_doubled {
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            "Torso Up is active: the body piece's skill points count double.",
            muted(),
        ));
    }
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(theme::pane(title, false)),
        detail_area,
    );
}

/// Behind one skill of the totals: each piece and jewel that adds to it, then its tiers with the next one to reach.
fn skill_detail_lines(app: &App, t: &mh3u_app::worn::SkillTotal) -> (String, Vec<Line<'static>>) {
    let detail = app.worn_skill_detail(t.id);
    let name = app.game.skill_name(t.id).unwrap_or("?");
    let mut lines: Vec<Line> = Vec::new();
    for s in &detail.sources {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<44}", fit(&s.label, 44)), muted()),
            Span::styled(format!("{:+}", s.points), theme::signed_style(s.points)),
        ]));
    }
    if !detail.tiers.is_empty() {
        lines.push(Line::raw(""));
    }
    let next_at = detail.next.map(|(p, ..)| p);
    for tier in &detail.tiers {
        let effect = app.game.effect_name(tier.effect).unwrap_or("?");
        let (mark, style) = if tier.reached {
            ("●", if tier.points > 0 { good() } else { bad() })
        } else {
            ("○", muted())
        };
        let mut spans = vec![
            Span::styled(format!("  {mark} {:>+3}  ", tier.points), style),
            Span::styled(effect.to_string(), if tier.reached { style } else { muted() }),
        ];
        if next_at == Some(tier.points)
            && let Some((_, _, missing)) = detail.next
        {
            spans.push(Span::styled(format!("   ← {missing} more"), accent()));
        }
        lines.push(Line::from(spans));
    }
    if app.skill_info
        && let Some(text) = t
            .effect()
            .and_then(|e| app.game.effect_description(e))
            .or_else(|| app.game.skill_description(t.id))
    {
        lines.push(Line::raw(""));
        lines.push(Line::styled(text.to_string(), muted()));
    }
    (format!(" {name} {:+} ", detail.total), lines)
}

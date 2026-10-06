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
    // the Shakalaka sidekick's mask, with what the game says about it
    if let Some(n) = app.save.sidekick_mask {
        gear.push(Line::raw(""));
        gear.push(Line::from(vec![
            Span::styled(format!("{:<10}", "Shakalaka"), muted()),
            Span::styled(app.game.mask_name(n).unwrap_or("a mask").to_string(), bold()),
        ]));
        if let Some(text) = app.game.mask_description(n) {
            gear.push(Line::styled(format!("  {text}"), muted()));
        }
    }
    f.render_widget(
        Paragraph::new(gear)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Worn gear ", true)),
        left,
    );

    // Right: while comparing, the pieces against a template; else defense and resistances, the skills (one is highlighted), and
    // what lies behind that skill.
    if let Some(versus) = app.worn_versus() {
        draw_versus(f, app, right, &versus);
        return;
    }
    let [header, list_area, detail_area] =
        Layout::vertical([Constraint::Length(4), Constraint::Min(5), Constraint::Length(17)]).areas(right);
    let mut head: Vec<Line> = totals_lines(app, &summary, &[]);
    head.truncate(2);
    f.render_widget(Paragraph::new(head).block(theme::pane(" Totals ", false)), header);

    let selected = app.worn_selected(&summary);
    app.worn.skills.select(selected);
    let shown = app.worn_rows(&summary);
    let list_title = if app.worn.near {
        format!(" Skills within {} points of the next tier ", mh3u_app::app::NEAR)
    } else {
        " Skills ".to_string()
    };
    if shown.is_empty() {
        let what = if app.worn.near {
            "No skill is that close to a tier."
        } else {
            "No skill points."
        };
        empty_pane(f, list_area, list_title, true, vec![Line::styled(what, muted())]);
    } else {
        let rows: Vec<ListItem> = shown.iter().map(|t| ListItem::new(skill_row(app, t, None))).collect();
        render_list(f, focused_list(rows, list_title, true), list_area, &mut app.worn.skills);
        scrollbar(f, list_area, shown.len(), selected);
    }

    let (title, mut lines) = match selected.and_then(|i| shown.get(i)) {
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
    if let Some((points, effect, gap)) = detail.next {
        lines.push(Line::raw(""));
        let m = app.worn_missing(t.id, gap);
        let target = format!("{:+} {}", points, app.game.effect_name(effect).unwrap_or("?"));
        lines.push(Line::from(vec![
            Span::styled(format!("To reach {target}: {gap} more"), accent()),
            Span::styled(
                if m.free.is_empty() {
                    "  no free gem slots".to_string()
                } else {
                    format!(
                        "  free slots: {}",
                        m.free.iter().map(|(w, n)| format!("{w} {n}")).collect::<Vec<_>>().join(", ")
                    )
                },
                muted(),
            ),
        ]));
        if m.jewels.is_empty() {
            lines.push(Line::styled("  You own no jewel that adds to it.", muted()));
        }
        for j in &m.jewels {
            let costs = j.costs.as_ref().map_or(String::new(), |c| format!(", costs {c}"));
            lines.push(Line::styled(
                format!(
                    "  {} x{}: {:+} for {} slot{}{costs}",
                    j.name,
                    j.owned,
                    j.points,
                    j.slots,
                    if j.slots == 1 { "" } else { "s" }
                ),
                muted(),
            ));
        }
        if !m.fills.is_empty() {
            let way: Vec<String> = m.fills.iter().map(|p| format!("{} ← {}", p.place, p.jewel)).collect();
            let (text, style) = if m.reaches {
                (format!("  Reachable: {} ({:+})", way.join(", "), m.gained), good())
            } else {
                (
                    format!("  Best with what fits: {} ({:+} of {gap})", way.join(", "), m.gained),
                    warn(),
                )
            };
            lines.push(Line::styled(text, style));
        } else if !m.jewels.is_empty() {
            lines.push(Line::styled("  None of them fits a free gem slot.", warn()));
        }
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

/// The worn pieces against a template: the skills that differ, then each slot.
fn draw_versus(f: &mut Frame, app: &mut App, area: Rect, v: &mh3u_app::app::Versus) {
    let [list_area, pieces_area] = Layout::vertical([Constraint::Min(5), Constraint::Length(9)]).areas(area);
    let selected = app.worn.compare.selected().unwrap_or(0).min(v.rows.len().saturating_sub(1));
    app.worn.compare.select((!v.rows.is_empty()).then_some(selected));
    let effect = |skill: u8, points: i32| {
        mh3u_core::skilltiers::effect_for(skill, points)
            .and_then(|e| app.game.effect_name(e))
            .map(str::to_string)
    };
    let title = format!(" Worn vs {} · pieces only, no jewels · v back ", v.template);
    if v.rows.is_empty() {
        empty_pane(f, list_area, title, true, vec![Line::styled("Neither has skill points.", muted())]);
    } else {
        let rows: Vec<ListItem> = v
            .rows
            .iter()
            .map(|r| {
                let (was, now) = (effect(r.skill, r.worn), effect(r.skill, r.template));
                let change = if was == now {
                    now.map_or(String::new(), |n| format!("{n} (same)"))
                } else {
                    format!("{} → {}", was.as_deref().unwrap_or("none"), now.as_deref().unwrap_or("none"))
                };
                let diff_style = theme::signed_style(r.diff());
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{:<18}", app.game.skill_name(r.skill).unwrap_or("?"))),
                    Span::styled(format!("{:+4}", r.worn), theme::signed_style(r.worn)),
                    Span::styled(" → ", muted()),
                    Span::styled(format!("{:+4}", r.template), theme::signed_style(r.template)),
                    Span::styled(
                        if r.diff() == 0 {
                            "      ".to_string()
                        } else {
                            format!("  {:+3} ", r.diff())
                        },
                        diff_style,
                    ),
                    Span::styled(change, if r.diff() == 0 { muted() } else { bold() }),
                ]))
            })
            .collect();
        render_list(f, focused_list(rows, title, true), list_area, &mut app.worn.compare);
        scrollbar(f, list_area, v.rows.len(), Some(selected));
    }
    let mut lines: Vec<Line> = Vec::new();
    for d in &v.pieces {
        let name = |n: &Option<String>| n.clone().unwrap_or_else(|| "nothing".to_string());
        let mut spans = vec![Span::styled(format!("{:<9}", d.slot), muted())];
        if d.same() {
            spans.push(Span::styled(format!("{} (same)", name(&d.worn)), muted()));
        } else {
            spans.push(Span::raw(fit(&name(&d.worn), 24)));
            spans.push(Span::styled(" → ", muted()));
            spans.push(Span::styled(fit(&name(&d.template), 40), bold()));
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines).block(theme::pane(" Pieces ", false)), pieces_area);
}

/// The popup that lists the saved templates to compare the worn pieces with.
pub(super) fn draw_worn_pick(f: &mut Frame, app: &mut App) {
    let Some(mut state) = app.worn.pick.take() else { return };
    let area = f.area();
    let rows: Vec<ListItem> = app
        .builds
        .templates
        .iter()
        .map(|t| {
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<28}", fit(&t.name, 28))),
                Span::styled(format!("{} pieces", t.pieces.len()), muted()),
            ]))
        })
        .collect();
    let (w, h) = (52.min(area.width), (rows.len() as u16 + 2).clamp(3, 16).min(area.height));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, popup);
    render_list(
        f,
        List::new(rows)
            .block(theme::pane(" Compare with which template? · Enter · Esc ", true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        popup,
        &mut state,
    );
    app.worn.pick = Some(state);
}

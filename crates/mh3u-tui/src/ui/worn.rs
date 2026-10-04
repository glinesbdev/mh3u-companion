//! The Worn tab and the totals shown for a set of armor.

use super::*;

/// Defense, gem slots, resistances and the skill points of a set of armor. Skills in `targets` show whether their goal is reached.
pub(super) fn totals_lines(app: &App, summary: &mh3u_app::worn::Summary, targets: &[mh3u_app::builds::Target]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled("Defense ", muted()),
        Span::styled(summary.defense.to_string(), bold()),
        Span::styled(" (base, before upgrading)", muted()),
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
        let (state, style) = if let Some(g) = goal {
            if t.points >= g.points {
                (&*format!("✔ goal {}", g.points), good())
            } else {
                (&*format!("✘ goal {}", g.points), bad())
            }
        } else if t.active() {
            ("● active", good())
        } else if t.penalty() {
            ("▼ penalty", bad())
        } else if t.points > 0 {
            (&*format!("{} more to activate", mh3u_app::worn::ACTIVE_AT - t.points), muted())
        } else {
            ("", muted())
        };
        let parts: Vec<String> = t
            .parts
            .iter()
            .map(|&(kind, p)| format!("{} {p:+}", app.game.equipment_kind_label(kind).unwrap_or("?")))
            .collect();
        lines.push(Line::from(vec![
            Span::raw(format!("{:<18}", app.game.skill_name(t.id).unwrap_or("?"))),
            Span::styled(format!("{:+4} ", t.points), theme::signed_style(t.points)),
            Span::styled(format!("{state:<22}"), style),
            Span::styled(parts.join(", "), muted()),
        ]));
        if app.skill_info
            && let Some(text) = app.game.skill_description(t.id)
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

/// The worn gear and what it adds up to.
pub(super) fn draw_worn(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 45);
    let armor = app.worn_armor();

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
    } else {
        gear.push(Line::from(vec![
            Span::styled(format!("{:<8}", "Weapon"), muted()),
            Span::styled("nothing", muted()),
        ]));
    }
    gear.push(Line::raw(""));
    for (kind, label) in [(5u8, "Head"), (1, "Body"), (2, "Arms"), (3, "Waist"), (4, "Legs")] {
        let piece = armor.iter().find(|&&(k, _)| k == kind).map(|&(_, e)| e);
        let mut spans = vec![Span::styled(format!("{label:<8}"), muted())];
        match piece {
            Some(e) => {
                spans.push(Span::styled(
                    format!("{:<24}", fit(app.game.equipment_name(kind, e.id).unwrap_or("?"), 24)),
                    bold(),
                ));
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
    gear.push(Line::raw(""));
    gear.push(Line::styled(
        "Decorations are not read yet. A talisman is not counted either: it is not known where the save records the worn one.",
        muted(),
    ));
    f.render_widget(
        Paragraph::new(gear)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Worn gear ", true)),
        left,
    );

    // Right: the totals.
    let stats: Vec<(u8, &mh3u_core::armor::ArmorStats)> = armor
        .iter()
        .filter_map(|&(kind, e)| app.game.armor_stats(kind, e.id).map(|a| (kind, a)))
        .collect();
    let summary = mh3u_app::worn::summarize(&stats);
    let mut lines = totals_lines(app, &summary, &[]);
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "A skill's first effect starts at 10 points and its penalty at -10. Higher tiers (15, 20) are not shown.",
        muted(),
    ));
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Totals ", false)),
        right,
    );
}

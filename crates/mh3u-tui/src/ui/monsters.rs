//! The Monsters tab.

use super::*;

/// A monster and what it drops, with the items the wishlist still needs picked out.
pub(super) fn draw_monsters(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 33);
    let view = app.monster_view();
    let selected = app.highlighted_monster();
    let rows: Vec<ListItem> = view
        .iter()
        .map(|&(m, wanted)| {
            let mut spans = vec![Span::raw(format!("{:<22}", fit(app.game.monster_name(m).unwrap_or("?"), 22)))];
            if wanted > 0 {
                spans.push(Span::styled(format!("★ {wanted}"), warn()));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let query = app.monsters.search.trim();
    let title = if query.is_empty() {
        format!(" Monsters ({}) · {} ", rows.len(), app.monsters.sort.label())
    } else {
        format!(" Monsters · \"{query}\" ({}) · {} ", rows.len(), app.monsters.sort.label())
    };
    if rows.is_empty() {
        let text = if query.is_empty() {
            "No monster drop data."
        } else {
            "No monster matches."
        };
        empty_pane(f, left, title, true, vec![Line::styled(text, muted())]);
        return;
    }
    let len = rows.len();
    let mut state = ListState::default().with_selected(selected.and_then(|m| view.iter().position(|&(v, _)| v == m)));
    render_list(
        f,
        List::new(rows)
            .block(theme::pane(title, true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        left,
        &mut state,
    );
    scrollbar(f, left, len, state.selected());

    let lines = selected
        .map(|m| monster_details(app, m, usize::from(right.width.saturating_sub(3))))
        .unwrap_or_default();
    let visible = usize::from(right.height.saturating_sub(2));
    let total = lines.len();
    let scroll = usize::from(app.monsters.scroll).min(total.saturating_sub(visible));
    app.monsters.scroll = scroll as u16;
    f.render_widget(
        Paragraph::new(lines)
            .scroll((scroll as u16, 0))
            .block(theme::pane(" Drops ", false)),
        right,
    );
    scrollbar(f, right, total, Some(scroll));
}

pub(super) fn monster_details(app: &App, monster: u16, width: usize) -> Vec<Line<'static>> {
    let missing = app.missing_for_wishlist();
    let mut lines = vec![Line::styled(app.game.monster_name(monster).unwrap_or("?").to_string(), bold())];
    if let Some(note) = app.game.monster_note(monster) {
        lines.extend(super::help::wrap_words(note, width).into_iter().map(|l| Line::styled(l, muted())));
        lines.push(Line::raw(""));
    }
    lines.push(Line::from(vec![
        Span::styled("Chance in percent. ", muted()),
        Span::styled("★", warn()),
        Span::styled(" marks what your wishlist still needs.", muted()),
    ]));
    lines.extend(weak_spots(app, monster));
    let mut current = None;
    for (method, rank, list) in app.game.drops().lists_for(monster) {
        if current != Some(method) {
            lines.push(Line::raw(""));
            lines.push(Line::styled(app.game.drop_label(monster, rank, method), bold()));
            current = Some(method);
        }
        let items: Vec<Vec<Span<'static>>> = list
            .iter()
            .map(|d| {
                let name = app.game.item_name(d.item).unwrap_or("?");
                let quantity = if d.quantity > 1 {
                    format!(" x{}", d.quantity)
                } else {
                    String::new()
                };
                let mut spans = if missing.contains_key(&d.item) {
                    vec![Span::styled(format!("★ {name}{quantity}"), warn().add_modifier(Modifier::BOLD))]
                } else {
                    vec![Span::raw(format!("{name}{quantity}"))]
                };
                spans.push(Span::styled(format!(" {}%", d.percent), muted()));
                spans
            })
            .collect();
        lines.extend(wrap_items(&format!("  {:<10}", rank.label()), items, width));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Part break names come from a hand-made table and can be off.",
        muted(),
    ));
    lines
}

const ELEMENTS: [&str; 5] = ["Fire", "Water", "Ice", "Thunder", "Dragon"];

/// A damage percentage, bold green where the zone is soft: from 70 for weapons, from 25 for elements (which start much lower).
fn soft(value: u8, from: u8) -> Span<'static> {
    let style = if value >= from {
        good().add_modifier(Modifier::BOLD)
    } else {
        muted()
    };
    Span::styled(format!("{value:>5}"), style)
}

/// The hit zones in the game's order with their damage percentages, and the element that does the most anywhere. The names come from
/// a public database (the game's data names none); a zone with no name there is numbered.
fn weak_spots(app: &App, monster: u16) -> Vec<Line<'static>> {
    let zones = app.game.hit_zones(monster);
    if zones.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![Line::raw(""), Line::styled("Weak spots", bold())];
    let label = |i: usize| match app.game.zone_name(monster, i) {
        Some(name) => name.to_string(),
        None => format!("Zone {}", i + 1),
    };
    let mut header = vec![Span::styled(format!("  {:<12}", "Zone"), muted())];
    header.extend(["Cut", "Imp", "Shot", "Fire", "Wat", "Ice", "Thun", "Drag"].map(|h| Span::styled(format!("{h:>5}"), muted())));
    lines.push(Line::from(header));
    for (i, z) in zones.iter().enumerate() {
        let mut spans = vec![Span::raw(format!("  {:<12}", fit(&label(i), 12)))];
        spans.extend(z.physical().map(|v| soft(v, 70)));
        spans.extend(z.elements().map(|v| soft(v, 25)));
        lines.push(Line::from(spans));
    }
    let best = (0..ELEMENTS.len())
        .map(|e| {
            (
                e,
                zones.iter().enumerate().map(|(i, z)| (z.elements()[e], i)).max().unwrap_or((0, 0)),
            )
        })
        .max_by_key(|&(_, (value, _))| value);
    if let Some((e, (value, zone))) = best {
        lines.push(Line::from(vec![
            Span::styled("  Best element: ", muted()),
            Span::styled(ELEMENTS[e], theme::element_style(ELEMENTS[e])),
            Span::styled(format!(" {value}% at {}", label(zone)), muted()),
        ]));
    }
    lines.push(Line::styled("  Game order; names from a public database.", muted()));
    lines
}

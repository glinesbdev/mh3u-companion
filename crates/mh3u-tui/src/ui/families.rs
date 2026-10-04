//! The Sets tab.

use super::*;
use crate::families::SLOT_ORDER;

/// Armor families on the left; on the right the highlighted family as a table of variants by slot.
pub(super) fn draw_families(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 36);
    let rows: Vec<ListItem> = app
        .families
        .rows
        .iter()
        .map(|r| {
            let family = &app.families.families[r.family];
            let style = if r.owned == 0 {
                muted()
            } else if r.owned == r.total {
                good()
            } else {
                warn()
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<20}", fit(&family.name, 20))),
                Span::styled(format!("{}/{}", r.owned, r.total), style),
            ]))
        })
        .collect();
    let mut title = format!(" Sets ({}) ", rows.len());
    if !app.families.search.is_empty() {
        title = format!(" \"{}\" ({}) ", app.families.search, rows.len());
    }
    if app.families.only_owned {
        title.push_str("· owned ");
    }
    let len = rows.len();
    if rows.is_empty() {
        empty_pane(f, left, title, true, vec![Line::styled("No set matches.", muted())]);
    } else {
        render_list(
            f,
            List::new(rows)
                .block(theme::pane(title, true))
                .highlight_style(theme::selection())
                .highlight_symbol(theme::SELECTION_MARK),
            left,
            &mut app.families.state,
        );
        scrollbar(f, left, len, app.families.state.selected());
    }
    let lines = family_lines(app);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Variants ", false)),
        right,
    );
}

/// The family as a table: a row per variant, a column per slot, and a mark for each piece in the cell (there are usually two: one
/// for each gender).
fn family_lines(app: &App) -> Vec<Line<'static>> {
    let Some(family) = app.families.selected() else {
        return vec![Line::styled("Armor grouped by the family in its name.", muted())];
    };
    let anvil = theme::anvil().content.trim().to_string();
    let mut lines = vec![
        Line::styled(family.name.clone(), bold()),
        Line::from(vec![
            Span::styled("● ", good()),
            Span::styled("owned   ", muted()),
            theme::anvil(),
            Span::styled("on offer   ", muted()),
            Span::styled("○ ", muted()),
            Span::styled("not yet", muted()),
        ]),
        Line::raw(""),
    ];
    let mut head = vec![Span::styled(format!("{:<8}", "variant"), muted())];
    for kind in SLOT_ORDER {
        head.push(Span::styled(format!("{:<9}", crate::templates::slot_label(kind)), muted()));
    }
    lines.push(Line::from(head));
    for variant in family.variants() {
        let mut row = vec![Span::styled(format!("{:<8}", variant.label()), bold())];
        for kind in SLOT_ORDER {
            let marks: Vec<Span> = family
                .cell(variant, kind)
                .map(|m| {
                    if app.save.owns_equipment(m.kind, m.id) {
                        Span::styled("●", good())
                    } else if app.at_blacksmith(m.kind, m.id) {
                        Span::styled(anvil.clone(), accent())
                    } else {
                        Span::styled("○", muted())
                    }
                })
                .collect();
            let mut cell: Vec<Span> = marks.into_iter().flat_map(|s| [s, Span::raw(" ")]).collect();
            let used: usize = cell.iter().map(Span::width).sum();
            cell.push(Span::raw(" ".repeat(9usize.saturating_sub(used))));
            row.extend(cell);
        }
        lines.push(Line::from(row));
    }
    lines.push(Line::raw(""));
    // which variant of each slot is the best you have
    let best: Vec<Span> = SLOT_ORDER
        .iter()
        .flat_map(|&kind| {
            let top = family
                .members
                .iter()
                .filter(|m| m.kind == kind && app.save.owns_equipment(m.kind, m.id))
                .map(|m| m.variant)
                .max();
            let text = top.map_or("none".to_string(), |v| v.label().to_string());
            [
                Span::styled(format!("{} ", crate::templates::slot_label(kind)), muted()),
                Span::styled(format!("{text}   "), if top.is_some() { good() } else { muted() }),
            ]
        })
        .collect();
    lines.push(Line::styled("Best you own", bold()));
    lines.push(Line::from(best));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "The higher variants (S, U, X, Z) are made from tougher monsters' parts. Enter shows the family on the Crafting tab.",
        muted(),
    ));
    lines
}

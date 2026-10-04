//! The Compare tab: a few weapons side by side.

use super::*;
use mh3u_app::compare::best_marks;
use ratatui::widgets::{Cell, Row, Table};

/// What the table shows about one weapon.
struct Column {
    name: String,
    kind: u8,
    kind_label: &'static str,
    rarity: u8,
    attack: u32,
    affinity: i8,
    slots: u8,
    owned: bool,
    /// The blacksmith is offering it.
    offered: bool,
    /// Sharpness bars and elements, when the table has the weapon.
    extras: Option<mh3u_core::weapon_extras::Extras>,
    /// The fees of the cheapest way to get it (0 when owned); `None` when not all prices are known.
    zenny: Option<u32>,
}

fn columns(app: &App) -> Vec<Column> {
    app.compare
        .weapons
        .iter()
        .filter_map(|&(kind, id)| {
            let w = app.game.weapon_stats(kind, id)?;
            let owned = app.save.owns_equipment(kind, id);
            let zenny = if owned {
                Some(0)
            } else {
                app.cheapest_path(kind, id).filter(|p| p.unknown_prices() == 0).map(|p| p.zenny())
            };
            Some(Column {
                name: app.game.equipment_name(kind, id).unwrap_or("?").to_string(),
                kind,
                kind_label: app.game.equipment_kind_label(kind).unwrap_or("?"),
                rarity: w.rarity,
                attack: w.attack,
                affinity: w.affinity,
                slots: w.slots,
                extras: app.game.weapon_extras(kind, id).cloned(),
                owned,
                offered: !owned && app.at_blacksmith(kind, id),
                zenny,
            })
        })
        .collect()
}

/// The weapons in the comparison on the left; on the right a table with one column each, the best value in each row picked out.
pub(super) fn draw_compare(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 30);
    let cols = columns(app);
    let rows: Vec<ListItem> = cols
        .iter()
        .map(|c| {
            ListItem::new(Line::from(vec![
                Span::raw(format!("{:<20}", fit(&c.name, 20))),
                Span::styled(c.kind_label.to_string(), muted()),
            ]))
        })
        .collect();
    let title = format!(" Weapons ({}/{}) ", rows.len(), mh3u_app::app::MAX_COMPARED);
    if rows.is_empty() {
        empty_pane(
            f,
            left,
            title,
            true,
            vec![Line::styled(
                "Press v on a weapon in Crafting, Equipment or Wishlist to put it here.",
                muted(),
            )],
        );
    } else {
        render_list(
            f,
            List::new(rows)
                .block(theme::pane(title, true))
                .highlight_style(theme::selection())
                .highlight_symbol(theme::SELECTION_MARK),
            left,
            &mut app.compare.state,
        );
    }
    if cols.is_empty() {
        f.render_widget(Paragraph::new("").block(theme::pane(" Side by side ", false)), right);
        return;
    }

    let same_type = cols.iter().all(|c| c.kind == cols[0].kind);
    let mark = |values: Vec<Option<i64>>, higher: bool| best_marks(&values, higher);
    let attack = if same_type {
        mark(cols.iter().map(|c| Some(i64::from(c.attack))).collect(), true)
    } else {
        vec![false; cols.len()]
    };
    let affinity = mark(cols.iter().map(|c| Some(i64::from(c.affinity))).collect(), true);
    let slots = mark(cols.iter().map(|c| Some(i64::from(c.slots))).collect(), true);
    let sharp_total = |c: &Column| {
        c.extras
            .as_ref()
            .map(|e| i64::from(e.sharpness.iter().map(|&v| u32::from(v)).sum::<u32>()))
    };
    let sharp = mark(cols.iter().map(sharp_total).collect(), true);
    let cost = mark(cols.iter().map(|c| c.zenny.map(i64::from)).collect(), false);
    let cell = |text: String, best: bool| {
        if best {
            Cell::from(Span::styled(format!("{text} ✔"), good().add_modifier(Modifier::BOLD)))
        } else {
            Cell::from(text)
        }
    };
    let row =
        |label: &'static str, cells: Vec<Cell<'static>>| Row::new(std::iter::once(Cell::from(Span::styled(label, muted()))).chain(cells));
    let table_rows = vec![
        row("Type", cols.iter().map(|c| Cell::from(c.kind_label)).collect()),
        row(
            "Rarity",
            cols.iter().map(|c| Cell::from(Line::from(theme::rarity_badge(c.rarity)))).collect(),
        ),
        row(
            "Attack",
            cols.iter().zip(&attack).map(|(c, &b)| cell(c.attack.to_string(), b)).collect(),
        ),
        row(
            "Affinity",
            cols.iter()
                .zip(&affinity)
                .map(|(c, &b)| cell(format!("{:+}%", c.affinity), b))
                .collect(),
        ),
        row(
            "Element",
            cols.iter()
                .map(|c| match c.extras.as_ref().filter(|e| !e.specials.is_empty()) {
                    Some(e) => Cell::from(Line::from(
                        e.specials
                            .iter()
                            .flat_map(|s| {
                                let style = theme::element_style(&s.name);
                                [
                                    Span::styled(
                                        format!("{} {}", s.name, s.value),
                                        if s.hidden { style.add_modifier(Modifier::DIM) } else { style },
                                    ),
                                    Span::raw(" "),
                                ]
                            })
                            .collect::<Vec<_>>(),
                    )),
                    None => Cell::from(Span::styled("none", muted())),
                })
                .collect(),
        ),
        row(
            "Sharpness",
            cols.iter()
                .zip(&sharp)
                .map(|(c, &best)| match &c.extras {
                    Some(e) => {
                        let mut spans = super::pieces::sharpness_spans(&e.sharpness, 5);
                        if best {
                            spans.push(Span::styled(" ✔", good().add_modifier(Modifier::BOLD)));
                        }
                        Cell::from(Line::from(spans))
                    }
                    None => Cell::from(Span::styled("?", muted())),
                })
                .collect(),
        ),
        row(
            "With Sharp +1",
            cols.iter()
                .map(|c| match &c.extras {
                    Some(e) => Cell::from(Line::from(super::pieces::sharpness_spans(&e.plus, 5))),
                    None => Cell::from(Span::styled("?", muted())),
                })
                .collect(),
        ),
        row(
            "Gem slots",
            cols.iter().zip(&slots).map(|(c, &b)| cell(theme::gems(c.slots), b)).collect(),
        ),
        row(
            "You",
            cols.iter()
                .map(|c| {
                    if c.owned {
                        Cell::from(Span::styled("● owned", good()))
                    } else if c.offered {
                        Cell::from(Span::styled("on offer", accent()))
                    } else {
                        Cell::from(Span::styled("not yet", muted()))
                    }
                })
                .collect(),
        ),
        row(
            "Cheapest way",
            cols.iter()
                .zip(&cost)
                .map(|(c, &b)| match (c.owned, c.zenny) {
                    (true, _) => cell("owned".to_string(), b),
                    (false, Some(z)) => cell(format!("{} z", group_digits(u64::from(z))), b),
                    (false, None) => Cell::from(Span::styled("price not seen", muted())),
                })
                .collect(),
        ),
    ];
    let header = Row::new(std::iter::once(Cell::from("")).chain(cols.iter().map(|c| Cell::from(Span::styled(fit(&c.name, 22), bold())))));
    let mut widths = vec![Constraint::Length(14)];
    widths.extend(cols.iter().map(|_| Constraint::Fill(1)));
    let [table_area, note_area] = Layout::vertical([Constraint::Min(5), Constraint::Length(4)]).areas(right);
    f.render_widget(
        Table::new(table_rows, widths)
            .header(header)
            .column_spacing(2)
            .block(theme::pane(" Side by side ", false)),
        table_area,
    );
    let mut notes = vec![Line::styled(
        "✔ marks the best in a row. Sharpness, element and hidden element (dimmed) come from a public database, not the game.",
        muted(),
    )];
    if !same_type {
        notes.push(Line::styled(
            "Attack is not compared: the types are different, and each type scales attack in its own way.",
            muted(),
        ));
    }
    f.render_widget(Paragraph::new(notes).wrap(Wrap { trim: false }), note_area);
}

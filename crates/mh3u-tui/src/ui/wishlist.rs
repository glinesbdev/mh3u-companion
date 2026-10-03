//! The Wishlist tab.

use super::*;

pub(super) fn draw_wishlist(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 45);

    let rows: Vec<ListItem> = app
        .wishlist
        .iter()
        .map(|&(kind, id)| {
            let name = app.game.equipment_name(kind, id).unwrap_or("?");
            let label = app.game.equipment_kind_label(kind).unwrap_or("?");
            let state = if app.save.owns_equipment(kind, id) {
                Span::styled("● owned", good())
            } else if app.can_make_now(kind, id) {
                Span::styled("✔ ready", good())
            } else {
                Span::raw("")
            };
            // an anvil marks a piece the blacksmith is offering that you don't own yet
            let anvil = if !app.save.owns_equipment(kind, id) && app.at_blacksmith(kind, id) {
                theme::anvil()
            } else {
                theme::no_anvil()
            };
            ListItem::new(Line::from(vec![
                anvil,
                Span::raw(format!("{:<20}", fit(name, 20))),
                Span::styled(format!("{label:<14}"), muted()),
                state,
            ]))
        })
        .collect();
    let title = format!(" Wishlist ({}) ", rows.len());
    let len = rows.len();
    if rows.is_empty() {
        empty_pane(f, left, title, true, vec![Line::styled("Nothing here yet.", muted())]);
    } else {
        f.render_stateful_widget(
            List::new(rows)
                .block(theme::pane(title, true))
                .highlight_style(theme::selection())
                .highlight_symbol(theme::SELECTION_MARK),
            left,
            &mut app.wish_state,
        );
        scrollbar(f, left, len, app.wish_state.selected());
    }

    if app.wishlist.is_empty() {
        let hint = vec![
            Line::from("Your wishlist is empty."),
            Line::styled("On the Crafting tab, press w on a piece to add it (★).", muted()),
            Line::styled("Parent weapons it needs are added with it.", muted()),
        ];
        f.render_widget(Paragraph::new(hint).block(theme::pane(" Details ", false)), right);
        return;
    }

    // Top right: what the highlighted piece needs on its own.
    let zenny = app.save.zenny;
    let selected = app.wish_state.selected().and_then(|i| app.wishlist.get(i)).copied();
    let mut piece_title = " Selected ".to_string();
    let mut piece_lines: Vec<Line> = Vec::new();
    if let Some((kind, id)) = selected {
        piece_title = format!(" {} ", app.game.equipment_name(kind, id).unwrap_or("?"));
        if app.save.owns_equipment(kind, id) {
            piece_lines.push(Line::styled("You already own this piece (left out of the shopping list).", good()));
            // still show what making another copy takes; `stock all` covers it
            if let Some(plan) = app.plan(kind, id) {
                for m in &plan.materials {
                    let have = app.save.item_count(m.id);
                    piece_lines.push(Line::styled(
                        format!("  {:<24}{have:>3} / {:<3}", app.game.item_name(m.id).unwrap_or("?"), m.count),
                        muted(),
                    ));
                }
            }
        } else if let Some(plan) = app.plan(kind, id) {
            let how = match (plan.via, plan.parent_owned) {
                (Via::Create, _) => "Create from scratch",
                (Via::Upgrade, true) => "Upgrade (parent weapon owned)",
                (Via::Upgrade, false) => "Upgrade (you don't own a parent weapon)",
            };
            piece_lines.push(Line::styled(how, muted()));
            if plan.via == Via::Create {
                piece_lines.extend(unlock_line(app, kind, id));
            }
            let route = if plan.via == Via::Create { Route::Create } else { Route::Upgrade };
            piece_lines.push(match app.cost(kind, id, route) {
                Some((c, _)) => {
                    let mut spans = vec![Span::raw("Cost: ")];
                    spans.extend(cost_spans(c, zenny));
                    Line::from(spans)
                }
                None => Line::styled("Cost: not seen yet", muted()),
            });
            for m in &plan.materials {
                piece_lines.push(theme::material_line(
                    app.game.item_name(m.id).unwrap_or("?"),
                    app.save.item_count(m.id),
                    u32::from(m.count),
                ));
            }
            piece_lines.push(if app.can_make_now(kind, id) {
                Line::styled("✔ You can make this now.", good())
            } else {
                Line::styled("✘ Not available yet.", bad())
            });
        } else {
            piece_lines.push(Line::styled("No recipe", muted()));
        }
    }
    let top_height = (piece_lines.len() as u16 + 2).min(right.height.saturating_sub(5)).max(3);
    let [top, bottom] = Layout::vertical([Constraint::Length(top_height), Constraint::Min(3)]).areas(right);
    f.render_widget(Paragraph::new(piece_lines).block(theme::pane(piece_title, false)), top);

    // Bottom right: the running total for every wishlisted piece you don't own yet.
    let (mut need, unowned) = app.shopping_need();
    need.sort_by_key(|&(item, n)| {
        (
            std::cmp::Reverse(n.saturating_sub(app.save.item_count(item))),
            app.game.item_name(item).unwrap_or("?").to_lowercase(),
        )
    });
    let mut lines: Vec<Line> = Vec::new();
    let mut missing_kinds = 0;
    for (item, n) in need {
        let have = app.save.item_count(item);
        let ok = have >= n;
        missing_kinds += usize::from(!ok);
        let mut line = theme::material_line(app.game.item_name(item).unwrap_or("?"), have, n);
        if !ok {
            line.push_span(Span::styled(format!("  need {} more", n - have), muted()));
        }
        lines.push(line);
    }
    lines.push(Line::raw(""));
    lines.push(if unowned == 0 {
        Line::styled("You already own everything on the wishlist.", good())
    } else if missing_kinds == 0 {
        Line::styled("✔ You have everything.", good())
    } else {
        Line::styled(format!("Short on {missing_kinds} material(s)."), bad())
    });
    let (known, unknown) = app.wishlist_cost();
    if unowned > 0 {
        let have = u64::from(app.save.zenny);
        let mut spans = vec![Span::raw("Zenny: "), Span::styled(format!("{} z", group_digits(known)), bold())];
        spans.push(Span::raw(" known"));
        if unknown > 0 {
            spans.push(Span::styled(format!(" + {unknown} piece(s) not seen yet"), muted()));
        }
        spans.push(Span::raw(" · you have "));
        let enough = if known > have { bad() } else { good() };
        spans.push(Span::styled(group_digits(have), enough));
        if known > have {
            spans.push(Span::styled(format!(" (short {} z)", group_digits(known - have)), bad()));
        }
        lines.push(Line::from(spans));
    }
    let title = format!(" Shopping list · all {unowned} unowned piece(s) ");
    f.render_widget(Paragraph::new(lines).block(theme::pane(title, false)), bottom);
}

//! The Wishlist tab.

use super::*;

pub(super) fn draw_wishlist(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 45);

    let rows = wish_rows(app);
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
            &mut app.wish.state,
        );
        scrollbar(f, left, len, app.wish.state.selected());
    }

    if app.wish.items.is_empty() {
        let hint = vec![
            Line::from("Your wishlist is empty."),
            Line::styled("On the Crafting tab, press w on a piece to add it (★).", muted()),
            Line::styled("Parent weapons it needs are added with it.", muted()),
        ];
        f.render_widget(Paragraph::new(hint).block(theme::pane(" Details ", false)), right);
        return;
    }

    // Top right: what the highlighted piece needs on its own.
    let selected = app.wish.state.selected().and_then(|i| app.wish.items.get(i)).copied();
    let (piece_title, piece_lines) = match selected {
        Some((kind, id)) => (
            format!(" {} ", app.game.equipment_name(kind, id).unwrap_or("?")),
            wish_piece_lines(app, kind, id),
        ),
        None => (" Selected ".to_string(), Vec::new()),
    };
    let top_height = (piece_lines.len() as u16 + 2).min(right.height.saturating_sub(5)).max(3);
    let [top, bottom] = Layout::vertical([Constraint::Length(top_height), Constraint::Min(3)]).areas(right);
    f.render_widget(Paragraph::new(piece_lines).block(theme::pane(piece_title, false)), top);

    // Bottom right: the running total for every wishlisted piece you don't own yet.
    let (lines, unowned) = shopping_lines(app);
    let title = format!(" Shopping list · all {unowned} unowned piece(s) ");
    f.render_widget(Paragraph::new(lines).block(theme::pane(title, false)), bottom);
}

/// One row per wishlisted piece: an anvil if the blacksmith offers it and you lack it, its name and type, and whether you own it
/// or can make it now.
fn wish_rows(app: &App) -> Vec<ListItem<'static>> {
    app.wish
        .items
        .iter()
        .map(|&(kind, id)| {
            let name = app.game.equipment_name(kind, id).unwrap_or("?");
            let label = app.game.equipment_kind_label(kind).unwrap_or("?");
            let owned = app.save.owns_equipment(kind, id);
            let state = if owned {
                Span::styled("● owned", good())
            } else if app.can_make_now(kind, id) {
                Span::styled("✔ ready", good())
            } else {
                Span::raw("")
            };
            let anvil = if !owned && app.at_blacksmith(kind, id) {
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
        .collect()
}

/// What one wishlisted piece takes on its own: the route, the cost, the materials you have and whether you can make it.
fn wish_piece_lines(app: &App, kind: u8, id: u16) -> Vec<Line<'static>> {
    let zenny = app.save.zenny;
    let mut lines: Vec<Line> = Vec::new();
    if app.save.owns_equipment(kind, id) {
        lines.push(Line::styled("You already own this piece (left out of the shopping list).", good()));
        // still show what making another copy takes; `stock all` covers it
        if let Some(plan) = app.plan(kind, id) {
            for m in &plan.materials {
                let have = app.save.item_count(m.id);
                lines.push(Line::styled(
                    format!("  {:<24}{have:>3} / {:<3}", app.game.item_name(m.id).unwrap_or("?"), m.count),
                    muted(),
                ));
            }
        }
        return lines;
    }
    let Some(plan) = app.plan(kind, id) else {
        lines.push(Line::styled("No recipe", muted()));
        return lines;
    };
    let how = match (plan.via, plan.parent_owned) {
        (Via::Create, _) => "Create from scratch",
        (Via::Upgrade, true) => "Upgrade (parent weapon owned)",
        (Via::Upgrade, false) => "Upgrade (you don't own a parent weapon)",
    };
    lines.push(Line::styled(how, muted()));
    if plan.via == Via::Create {
        lines.extend(unlock_line(app, kind, id));
    }
    let route = if plan.via == Via::Create { Route::Create } else { Route::Upgrade };
    lines.push(match app.cost(kind, id, route) {
        Some((c, _)) => {
            let mut spans = vec![Span::raw("Cost: ")];
            spans.extend(cost_spans(c, zenny));
            Line::from(spans)
        }
        None => Line::styled("Cost: not seen yet", muted()),
    });
    for m in &plan.materials {
        lines.push(theme::material_line(
            app.game.item_name(m.id).unwrap_or("?"),
            app.save.item_count(m.id),
            u32::from(m.count),
        ));
    }
    lines.push(if app.can_make_now(kind, id) {
        Line::styled("✔ You can make this now.", good())
    } else {
        Line::styled("✘ Not available yet.", bad())
    });
    lines
}

/// The running total for every wishlisted piece you do not own yet: each material, the verdict and the zenny. Also returns the
/// number of unowned pieces.
fn shopping_lines(app: &App) -> (Vec<Line<'static>>, usize) {
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
    if unowned > 0 {
        lines.push(zenny_line(app));
    }
    (lines, unowned)
}

/// `Zenny: 12,000 z known + 2 piece(s) not seen yet · you have 9,000 (short 3,000 z)`
fn zenny_line(app: &App) -> Line<'static> {
    let (known, unknown) = app.wishlist_cost();
    let have = u64::from(app.save.zenny);
    let mut spans = vec![Span::raw("Zenny: "), Span::styled(format!("{} z", group_digits(known)), bold())];
    spans.push(Span::raw(" known"));
    if unknown > 0 {
        spans.push(Span::styled(format!(" + {unknown} piece(s) not seen yet"), muted()));
    }
    spans.push(Span::raw(" · you have "));
    spans.push(Span::styled(group_digits(have), if known > have { bad() } else { good() }));
    if known > have {
        spans.push(Span::styled(format!(" (short {} z)", group_digits(known - have)), bad()));
    }
    Line::from(spans)
}

use crate::app::{App, Tab, Via, group_digits, signed_zenny};
use crate::theme::{self, accent, bad, bold, good, muted, warn};
use mh3u_core::prices::{Route, Source};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Tabs, Wrap},
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let [tabs, body, footer] = Layout::vertical([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)]).areas(f.area());

    let selected = Tab::ALL.iter().position(|&t| t == app.tab).unwrap_or(0);
    let edit_badge = if app.edit_mode {
        Span::styled("✎ EDIT ", bad().add_modifier(Modifier::BOLD))
    } else {
        Span::raw("")
    };
    let badge = match &app.live {
        Some(l) if l.connected => Span::styled("● live ", good().add_modifier(Modifier::BOLD)),
        Some(_) => Span::styled("◌ waiting for the game ", warn()),
        None => Span::raw(""),
    };
    let change = match app.zenny_change() {
        Some(d) if d > 0 => Span::styled(format!("▲ {} ", signed_zenny(d)), good().add_modifier(Modifier::BOLD)),
        Some(d) => Span::styled(format!("▼ {} ", signed_zenny(d)), bad().add_modifier(Modifier::BOLD)),
        None => Span::raw(""),
    };
    let title = Line::from(vec![
        Span::styled(" MH3U Companion ", accent().add_modifier(Modifier::BOLD)),
        Span::styled("— ", muted()),
        Span::styled(app.save.hunter_name.clone(), bold()),
        Span::styled(" · ", muted()),
        Span::styled(
            format!("{} z ", group_digits(u64::from(app.save.zenny))),
            warn().add_modifier(Modifier::BOLD),
        ),
        change,
        edit_badge,
        badge,
    ]);
    let tab_titles: Vec<Line> = Tab::ALL
        .iter()
        .map(|&t| {
            let count = match t {
                Tab::Equipment => Some(app.save.equipment_box.len()),
                Tab::Wishlist => Some(app.wishlist.len()),
                _ => None,
            };
            let mut spans = vec![Span::raw(t.title())];
            if let Some(n) = count {
                spans.push(Span::styled(format!(" {n}"), muted()));
            }
            Line::from(spans)
        })
        .collect();
    f.render_widget(
        Tabs::new(tab_titles)
            .select(selected)
            .divider(Span::styled("│", muted()))
            .block(theme::pane(title, false))
            .highlight_style(accent().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
        tabs,
    );

    match app.tab {
        Tab::Items => draw_items(f, app, body),
        Tab::Equipment => draw_equipment(f, app, body),
        Tab::Crafting => draw_crafting(f, app, body),
        Tab::Wishlist => draw_wishlist(f, app, body),
    }

    draw_footer(f, app, footer);

    if app.show_help {
        draw_help(f, app);
    }
}

/// The key hints for the current tab, or the prompt while typing a search or command.
fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let clear = !app.active_search().is_empty();
    let mut spans: Vec<Span> = if app.is_commanding() {
        vec![
            Span::styled(": ", accent()),
            Span::raw(format!("{}_", app.command)),
            Span::styled(
                "   Enter run · Esc cancel · zenny N | give ITEM [N] | set ITEM N | stock [all]",
                muted(),
            ),
        ]
    } else if app.confirm_quit {
        vec![Span::styled(
            "Cemu is still running. Quitting ends live updates for it. Quit anyway? (y/n)",
            warn(),
        )]
    } else if app.searching {
        vec![
            Span::styled("search: ", accent()),
            Span::raw(format!("{}_", app.active_search())),
            Span::styled("   Enter apply · Esc clear", muted()),
        ]
    } else {
        let mut keys: Vec<(&str, &str)> = vec![("←/→", "tab")];
        match app.tab {
            Tab::Crafting => {
                keys.push(("/", "search"));
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.extend([("c", "craftable"), ("o", "hide owned"), ("s", "sort"), ("w", "wish")]);
            }
            Tab::Items => {
                keys.extend([("↑/↓", "move"), ("/", "search")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.push(("s", "sort"));
            }
            Tab::Wishlist => keys.extend([("↑/↓", "move"), ("w", "remove")]),
            Tab::Equipment => keys.push(("↑/↓", "move")),
        }
        keys.extend([("?", "help"), ("q", "quit")]);
        theme::key_hints(&keys)
    };
    spans.push(Span::raw("   "));
    spans.push(Span::styled(app.status.clone(), muted()));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_help(f: &mut Frame, app: &App) {
    let text = "\
Move          ↑/↓ or j/k · PageUp/PageDown
Switch tab    ←/→ or h/l · Tab
Quit          q

Items         /  fuzzy search the pouch and box by item name
              s  sort the item box (box order, name, quantity)
Crafting      /  fuzzy search: name, type, skill, material,
                 male / female / blademaster / gunner, or a rarity
                 number 1-10 (armor only), e.g. 'attack 3'.
                 Several words must all match.
Esc or x      clear the search on the current tab
              c  only what you can make now
              o  hide pieces you already own
              u  only pieces you own that have no price recorded yet
              s  sort (game order, name, craftable first, owned first)
              w  add to the wishlist (★), with the parent weapons
                 it needs; press again to remove it
Wishlist      w or x  remove the selected piece, and the parents
                 that were added for it if nothing else needs them

A piece is craftable if you have the materials to create it, or to
upgrade it and you own a parent weapon. Upgrading uses up the parent,
so the wishlist plans a piece as 'create from scratch' whenever that
is possible and as an upgrade only when there is no other way.

Press any key to close.";
    let text = if app.edit_mode {
        format!(
            "{text}\n\nEDIT MODE (--debug-edit): changes go into the running game, not the file.\n\
             If you save in the game they are saved too.\n\
             \n\
             :  zenny 50000 | zenny +500 | give iron ore [n] | set honey 5\n\
             :  stock (covers the wishlist) | stock all (also pieces you own)"
        )
    } else {
        text.to_string()
    };
    let area = f.area();
    let (w, h) = (area.width.min(74), area.height.min(29));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: false }).block(theme::pane(" Help ", true)),
        popup,
    );
}

/// A thin scroll bar on a list's right border, only when the list is longer than the pane.
fn scrollbar(f: &mut Frame, area: Rect, len: usize, selected: Option<usize>) {
    if len + 2 <= usize::from(area.height) {
        return;
    }
    let mut state = ScrollbarState::new(len).position(selected.unwrap_or(0));
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .style(muted()),
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut state,
    );
}

/// What a list pane says when it has no rows.
fn empty_pane(f: &mut Frame, area: Rect, title: String, active: bool, lines: Vec<Line<'static>>) {
    f.render_widget(Paragraph::new(lines).block(theme::pane(title, active)), area);
}

fn draw_items(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 40);
    let rows = |stacks: &[mh3u_core::save::ItemStack], indent: &str| -> Vec<ListItem<'static>> {
        stacks
            .iter()
            .map(|s| {
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{indent}{:<26}", app.game.item_name(s.id).unwrap_or("?"))),
                    Span::styled(format!(" x{}", s.count), muted()),
                ]))
            })
            .collect()
    };
    // the box list has a selection mark column; the pouch lines up with it
    let pouch = rows(&app.pouch_view, "  ");
    let item_box = rows(&app.box_view, "");
    let query = app.item_search.trim();
    let pouch_title = if query.is_empty() {
        format!(" Item Pouch ({}/24) ", app.save.pouch.len())
    } else {
        format!(" Item Pouch · \"{query}\" ({}/{}) ", app.pouch_view.len(), app.save.pouch.len())
    };
    let box_title = if query.is_empty() {
        format!(" Item Box ({}/1000) · {} ", app.save.item_box.len(), app.box_sort_label())
    } else {
        format!(
            " Item Box · \"{query}\" ({}/{}) · {} ",
            app.box_view.len(),
            app.save.item_box.len(),
            app.box_sort_label()
        )
    };
    if pouch.is_empty() {
        let what = if query.is_empty() {
            "Nothing in the pouch."
        } else {
            "No pouch items match."
        };
        empty_pane(f, left, pouch_title, false, vec![Line::styled(what, muted())]);
    } else {
        f.render_widget(List::new(pouch).block(theme::pane(pouch_title, false)), left);
    }
    if item_box.is_empty() {
        let what = if query.is_empty() {
            "The item box is empty."
        } else {
            "No box items match. Press x to clear the search."
        };
        empty_pane(f, right, box_title, true, vec![Line::styled(what, muted())]);
        return;
    }
    let len = item_box.len();
    f.render_stateful_widget(
        List::new(item_box)
            .block(theme::pane(box_title, true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        right,
        &mut app.box_state,
    );
    scrollbar(f, right, len, app.box_state.selected());
}

fn draw_equipment(f: &mut Frame, app: &mut App, area: Rect) {
    let rows: Vec<ListItem> = app
        .save
        .equipment_box
        .iter()
        .map(|e| {
            let worn = if app.save.is_worn(e) {
                Span::styled("● worn", good())
            } else {
                Span::raw("")
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<16} ", app.game.equipment_kind_label(e.kind).unwrap_or("?")), muted()),
                Span::raw(format!("{:<28}", app.game.equipment_name(e.kind, e.id).unwrap_or("?"))),
                worn,
            ]))
        })
        .collect();
    let title = format!(" Equipment Box ({}/1000) ", rows.len());
    if rows.is_empty() {
        empty_pane(f, area, title, true, vec![Line::styled("The equipment box is empty.", muted())]);
        return;
    }
    let len = rows.len();
    f.render_stateful_widget(
        List::new(rows)
            .block(theme::pane(title, true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        area,
        &mut app.equip_state,
    );
    scrollbar(f, area, len, app.equip_state.selected());
}

/// A section heading with the forging cost beside it: `Create from scratch · 300 z (seen)`.
/// The cost is green when the hunter can pay it, and red with the shortfall when not.
fn cost_heading(title: &str, cost: Option<(u32, Source)>, zenny: u32) -> Line<'static> {
    let mut spans = vec![Span::styled(title.to_string(), bold())];
    match cost {
        Some((c, source)) => {
            let from = match source {
                Source::Seen => "seen",
                Source::Notes => "from your notes",
                Source::Learned => "recipe and cost learned from your game",
                Source::Game => "game data",
            };
            spans.push(Span::raw(" · "));
            spans.extend(cost_spans(c, zenny));
            spans.push(Span::styled(format!(" ({from})"), muted()));
        }
        None => spans.push(Span::styled(" · cost not seen yet", muted())),
    }
    Line::from(spans)
}

/// `1,150 z` in green if affordable, or `1,150 z` in red followed by `(short 400 z)`.
fn cost_spans(cost: u32, zenny: u32) -> Vec<Span<'static>> {
    let amount = format!("{} z", group_digits(u64::from(cost)));
    if cost <= zenny {
        vec![Span::styled(amount, good())]
    } else {
        vec![
            Span::styled(amount, bad()),
            Span::styled(format!(" (short {} z)", group_digits(u64::from(cost - zenny))), bad()),
        ]
    }
}

/// An armor piece's stats, one fact per line with the colors the rest of the screen uses.
fn armor_lines(app: &App, a: &mh3u_core::armor::ArmorStats) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        theme::rarity_badge(a.rarity),
        Span::raw(" "),
        Span::styled(theme::gems(a.slots), accent()),
        Span::styled(format!("  Defense {}", a.defense), bold()),
        Span::styled(" (base)", muted()),
    ])];
    let class = match a.class {
        Some(mh3u_core::armor::ArmorClass::Both) => "Blademaster & Gunner",
        Some(c) => c.label(),
        None => "?",
    };
    let gender = match a.gender {
        Some(mh3u_core::armor::Gender::Both) => "Male & Female",
        Some(g) => g.label(),
        None => "?",
    };
    lines.push(Line::from(vec![
        Span::styled("For ", muted()),
        Span::raw(class),
        Span::styled(" · ", muted()),
        Span::raw(gender),
    ]));
    let [fire, water, thunder, ice, dragon] = a.resist;
    let mut resist = Vec::new();
    for (name, value) in [
        ("Fire", fire),
        ("Water", water),
        ("Ice", ice),
        ("Thunder", thunder),
        ("Dragon", dragon),
    ] {
        if !resist.is_empty() {
            resist.push(Span::raw("  "));
        }
        resist.push(Span::styled(name, theme::element_style(name)));
        resist.push(Span::styled(format!(" {value:+}"), theme::signed_style(i32::from(value))));
    }
    lines.push(Line::from(resist));
    for &(id, pts) in &a.skills {
        lines.push(Line::from(vec![
            Span::raw(format!("  {:<18}", app.game.skill_name(id).unwrap_or("?"))),
            Span::styled(format!("{pts:+}"), theme::signed_style(i32::from(pts))),
        ]));
    }
    lines
}

fn draw_crafting(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 45);

    let rows: Vec<ListItem> = app
        .pieces
        .iter()
        .map(|p| {
            let mark = if p.craftable { Span::styled("✔", good()) } else { Span::raw(" ") };
            let star = if app.is_wished(p.kind, p.id) {
                Span::styled("★", warn())
            } else {
                Span::raw(" ")
            };
            let rarity = match app.game.armor_stats(p.kind, p.id) {
                Some(a) => theme::rarity_badge(a.rarity),
                None => Span::raw("   "),
            };
            let label = app.game.equipment_kind_label(p.kind).unwrap_or("?");
            let owned = if p.owned { " owned" } else { "" };
            let reason = p.reason.as_deref().map(|r| format!(" · {r}")).unwrap_or_default();
            ListItem::new(Line::from(vec![
                mark,
                star,
                Span::raw(format!(" {:<26}", p.name)),
                rarity,
                Span::styled(format!(" {label}{owned}"), muted()),
                Span::styled(reason, warn()),
            ]))
        })
        .collect();
    let mut title = format!(" Pieces ({}) · {} ", rows.len(), app.sort_label());
    if !app.search.is_empty() {
        title = format!(" \"{}\" ({}) · {} ", app.search, rows.len(), app.sort_label());
    }
    if app.craftable_only {
        title.push_str("[craftable only] ");
    }
    if app.hide_owned {
        title.push_str("[hiding owned] ");
    }
    if app.unpriced_only {
        title.push_str("[owned, no price yet] ");
    }
    if rows.is_empty() {
        let lines = vec![
            Line::from("No pieces to show."),
            Line::styled("Press x to clear the search, or c / o / u to relax the filters.", muted()),
        ];
        empty_pane(f, left, title, true, lines);
    } else {
        let len = rows.len();
        f.render_stateful_widget(
            List::new(rows)
                .block(theme::pane(title, true))
                .highlight_style(theme::selection())
                .highlight_symbol(theme::SELECTION_MARK),
            left,
            &mut app.craft_state,
        );
        scrollbar(f, left, len, app.craft_state.selected());
    }

    let mut lines: Vec<Line> = Vec::new();
    let zenny = app.save.zenny;
    if let Some(piece) = app.craft_state.selected().and_then(|i| app.pieces.get(i)) {
        lines.push(Line::styled(piece.name.clone(), bold()));
        lines.push(Line::styled(app.game.equipment_kind_label(piece.kind).unwrap_or("?"), muted()));
        if let Some(a) = app.game.armor_stats(piece.kind, piece.id) {
            lines.extend(armor_lines(app, a));
        }
        lines.push(Line::raw(""));
        if let Some(recipe) = app.create_recipe(piece.kind, piece.id) {
            lines.push(cost_heading(
                "Create from scratch",
                app.cost(piece.kind, piece.id, Route::Create),
                zenny,
            ));
            for m in &recipe.materials {
                lines.push(theme::material_line(
                    app.game.item_name(m.id).unwrap_or("?"),
                    app.save.item_count(m.id),
                    u32::from(m.count),
                ));
            }
            lines.push(Line::raw(""));
        }
        if let Some(up) = app.upgrade_recipe(piece.kind, piece.id) {
            lines.push(cost_heading("Upgrade from", app.cost(piece.kind, piece.id, Route::Upgrade), zenny));
            for &parent in &up.parents {
                let name = app.game.equipment_name(piece.kind, parent).unwrap_or("?");
                let (mark, style) = if app.save.owns_equipment(piece.kind, parent) {
                    ("● owned", good())
                } else {
                    ("○ not owned", bad())
                };
                lines.push(Line::from(vec![Span::raw(format!("  {name:<24}")), Span::styled(mark, style)]));
            }
            for m in &up.materials {
                lines.push(theme::material_line(
                    app.game.item_name(m.id).unwrap_or("?"),
                    app.save.item_count(m.id),
                    u32::from(m.count),
                ));
            }
            lines.push(Line::raw(""));
        }
        lines.push(if piece.craftable {
            Line::styled("✔ You can make this now.", good())
        } else {
            Line::styled("✘ Not available yet.", bad())
        });
        lines.push(Line::raw(""));
        lines.push(Line::styled("Costs are learned by watching you craft in live mode.", muted()));
    }
    f.render_widget(Paragraph::new(lines).block(theme::pane(" Recipe (pouch + box) ", false)), right);
}

fn draw_wishlist(f: &mut Frame, app: &mut App, area: Rect) {
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
            ListItem::new(Line::from(vec![
                Span::raw(format!("{name:<26}")),
                Span::styled(format!("{label:<15}"), muted()),
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

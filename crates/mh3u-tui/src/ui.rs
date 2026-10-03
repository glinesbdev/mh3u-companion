use crate::app::{App, Tab, Via, group_digits, signed_zenny};
use mh3u_core::prices::{Route, Source};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Clear, List, ListItem, Paragraph, Tabs, Wrap},
};

const HIGHLIGHT: Style = Style::new().add_modifier(Modifier::REVERSED);

pub fn draw(f: &mut Frame, app: &mut App) {
    let [tabs, body, footer] = Layout::vertical([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)]).areas(f.area());

    let selected = Tab::ALL.iter().position(|&t| t == app.tab).unwrap_or(0);
    let edit_badge = if app.edit_mode {
        Span::styled("✎ EDIT ", Style::new().red().bold())
    } else {
        Span::raw("")
    };
    let badge = match &app.live {
        Some(l) if l.connected => Span::styled("● live ", Style::new().green().bold()),
        Some(_) => Span::styled("◌ waiting for the game ", Style::new().yellow()),
        None => Span::raw(""),
    };
    let change = match app.zenny_change() {
        Some(d) if d > 0 => Span::styled(format!("▲ {} ", signed_zenny(d)), Style::new().green().bold()),
        Some(d) => Span::styled(format!("▼ {} ", signed_zenny(d)), Style::new().red().bold()),
        None => Span::raw(""),
    };
    let title = Line::from(vec![
        Span::raw(format!(
            " MH3U Companion — {} · {} z ",
            app.save.hunter_name,
            group_digits(u64::from(app.save.zenny))
        )),
        change,
        edit_badge,
        badge,
    ]);
    f.render_widget(
        Tabs::new(Tab::ALL.iter().map(|t| t.title()))
            .select(selected)
            .block(Block::bordered().title(title))
            .highlight_style(Style::new().yellow().bold()),
        tabs,
    );

    match app.tab {
        Tab::Items => draw_items(f, app, body),
        Tab::Equipment => draw_equipment(f, app, body),
        Tab::Crafting => draw_crafting(f, app, body),
        Tab::Wishlist => draw_wishlist(f, app, body),
    }

    let clear = if app.active_search().is_empty() { "" } else { " · x clear" };
    let help = match (app.tab, app.searching) {
        _ if app.is_commanding() => format!(
            ": {}_   (Enter run · Esc cancel · zenny N | give ITEM [N] | set ITEM N | stock [all])",
            app.command
        ),
        _ if app.confirm_quit => "Cemu is still running. Quitting ends live updates for it. Quit anyway? (y/n)".to_string(),
        (_, true) => format!("search: {}_   (Enter apply · Esc clear)", app.active_search()),
        (Tab::Crafting, _) => format!("←/→ tab · / search{clear} · c craftable · o hide owned · s sort · w wish · ? help · q quit"),
        (Tab::Items, _) => format!("←/→ tab · ↑/↓ move · / search{clear} · s sort · ? help · q quit"),
        (Tab::Wishlist, _) => "←/→ tab · ↑/↓ move · w remove · ? help · q quit".to_string(),
        _ => "←/→ tab · ↑/↓ move · ? help · q quit".to_string(),
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(help),
            Span::raw("   "),
            Span::styled(app.status.clone(), Style::new().dark_gray()),
        ])),
        footer,
    );

    if app.show_help {
        draw_help(f, app);
    }
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
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(Block::bordered().title(" Help ")),
        popup,
    );
}

fn draw_items(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(area);
    let rows = |stacks: &[mh3u_core::save::ItemStack]| -> Vec<ListItem<'static>> {
        stacks
            .iter()
            .map(|s| ListItem::new(format!("{:<26} x{}", app.game.item_name(s.id).unwrap_or("?"), s.count)))
            .collect()
    };
    let pouch = rows(&app.pouch_view);
    let item_box = rows(&app.box_view);
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
    f.render_widget(List::new(pouch).block(Block::bordered().title(pouch_title)), left);
    f.render_stateful_widget(
        List::new(item_box)
            .block(Block::bordered().title(box_title))
            .highlight_style(HIGHLIGHT),
        right,
        &mut app.box_state,
    );
}

fn draw_equipment(f: &mut Frame, app: &mut App, area: Rect) {
    let rows: Vec<ListItem> = app
        .save
        .equipment_box
        .iter()
        .map(|e| {
            let worn = if app.save.is_worn(e) {
                Span::styled("worn", Style::new().green())
            } else {
                Span::raw("")
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!(
                    "{:<16} {:<28}",
                    app.game.equipment_kind_label(e.kind).unwrap_or("?"),
                    app.game.equipment_name(e.kind, e.id).unwrap_or("?")
                )),
                worn,
            ]))
        })
        .collect();
    let title = format!(" Equipment Box ({}/1000) ", rows.len());
    f.render_stateful_widget(
        List::new(rows).block(Block::bordered().title(title)).highlight_style(HIGHLIGHT),
        area,
        &mut app.equip_state,
    );
}

/// A section heading with the forging cost beside it: `Create from scratch · 300 z (seen)`.
fn cost_heading(title: &str, cost: Option<(u32, Source)>) -> Line<'static> {
    let mut spans = vec![Span::styled(title.to_string(), Style::new().bold())];
    match cost {
        Some((c, source)) => {
            let from = match source {
                Source::Seen => "seen",
                Source::Notes => "from your notes",
                Source::Learned => "recipe and cost learned from your game",
                Source::Game => "game data",
            };
            spans.push(Span::raw(format!(" · {} z ", group_digits(u64::from(c)))));
            spans.push(Span::styled(format!("({from})"), Style::new().dark_gray()));
        }
        None => spans.push(Span::styled(" · cost not seen yet", Style::new().dark_gray())),
    }
    Line::from(spans)
}

fn draw_crafting(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(area);

    let rows: Vec<ListItem> = app
        .pieces
        .iter()
        .map(|p| {
            let mark = if p.craftable {
                Span::styled("✔", Style::new().green())
            } else {
                Span::raw(" ")
            };
            let star = if app.is_wished(p.kind, p.id) {
                Span::styled("★", Style::new().yellow())
            } else {
                Span::raw(" ")
            };
            let label = app.game.equipment_kind_label(p.kind).unwrap_or("?");
            let owned = if p.owned { " owned" } else { "" };
            let reason = p.reason.as_deref().map(|r| format!(" · {r}")).unwrap_or_default();
            ListItem::new(Line::from(vec![
                mark,
                star,
                Span::raw(format!(" {:<26}", p.name)),
                Span::styled(format!("{label}{owned}"), Style::new().dark_gray()),
                Span::styled(reason, Style::new().yellow()),
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
    f.render_stateful_widget(
        List::new(rows).block(Block::bordered().title(title)).highlight_style(HIGHLIGHT),
        left,
        &mut app.craft_state,
    );

    let mut lines: Vec<Line> = Vec::new();
    if let Some(piece) = app.craft_state.selected().and_then(|i| app.pieces.get(i)) {
        lines.push(Line::from(piece.name.clone().bold()));
        lines.push(Line::from(app.game.equipment_kind_label(piece.kind).unwrap_or("?").dark_gray()));
        if let Some(a) = app.game.armor_stats(piece.kind, piece.id) {
            lines.push(Line::raw(format!(
                "Rarity {} · Slots {} · Defense {} (base)",
                a.rarity, a.slots, a.defense
            )));
            lines.push(Line::raw(format!(
                "Gender: {} · Type: {}",
                a.gender.map_or("?", |g| g.label()),
                a.class.map_or("?", |c| c.label())
            )));
            let [fire, water, thunder, ice, dragon] = a.resist;
            lines.push(Line::raw(format!(
                "Fire {fire:+}  Water {water:+}  Ice {ice:+}  Thunder {thunder:+}  Dragon {dragon:+}"
            )));
            let skills: Vec<String> = a
                .skills
                .iter()
                .map(|&(id, pts)| format!("{} {pts:+}", app.game.skill_name(id).unwrap_or("?")))
                .collect();
            lines.push(Line::raw(format!("Skills: {}", skills.join(", "))));
        }
        lines.push(Line::raw(""));
        let material_lines = |lines: &mut Vec<Line>, stacks: &[mh3u_core::save::ItemStack]| {
            for m in stacks {
                let have = app.save.item_count(m.id);
                let style = if have >= m.count as u32 {
                    Style::new().green()
                } else {
                    Style::new().red()
                };
                lines.push(Line::from(vec![
                    Span::raw(format!("  {:<24}", app.game.item_name(m.id).unwrap_or("?"))),
                    Span::styled(format!("{have:>3} / {:<3}", m.count), style),
                ]));
            }
        };
        if let Some(recipe) = app.create_recipe(piece.kind, piece.id) {
            lines.push(cost_heading("Create from scratch", app.cost(piece.kind, piece.id, Route::Create)));
            material_lines(&mut lines, &recipe.materials);
            lines.push(Line::raw(""));
        }
        if let Some(up) = app.upgrade_recipe(piece.kind, piece.id) {
            lines.push(cost_heading("Upgrade from", app.cost(piece.kind, piece.id, Route::Upgrade)));
            for &parent in &up.parents {
                let name = app.game.equipment_name(piece.kind, parent).unwrap_or("?");
                let owned = app.save.owns_equipment(piece.kind, parent);
                let (mark, style) = if owned {
                    ("owned", Style::new().green())
                } else {
                    ("not owned", Style::new().red())
                };
                lines.push(Line::from(vec![Span::raw(format!("  {name:<24}")), Span::styled(mark, style)]));
            }
            material_lines(&mut lines, &up.materials);
            lines.push(Line::raw(""));
        }
        lines.push(if piece.craftable {
            Line::from("You can make this now.".green())
        } else {
            Line::from("Not available yet.".red())
        });
        lines.push(Line::raw(""));
        lines.push(Line::from("Costs are learned by watching you craft in live mode.".dark_gray()));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(" Recipe (pouch + box) ")),
        right,
    );
}

fn draw_wishlist(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)]).areas(area);

    let rows: Vec<ListItem> = app
        .wishlist
        .iter()
        .map(|&(kind, id)| {
            let name = app.game.equipment_name(kind, id).unwrap_or("?");
            let label = app.game.equipment_kind_label(kind).unwrap_or("?");
            let state = if app.save.owns_equipment(kind, id) {
                Span::styled("owned", Style::new().green())
            } else if app.can_make_now(kind, id) {
                Span::styled("ready", Style::new().green())
            } else {
                Span::raw("")
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!("{name:<26}")),
                Span::styled(format!("{label:<15}"), Style::new().dark_gray()),
                state,
            ]))
        })
        .collect();
    let title = format!(" Wishlist ({}) ", rows.len());
    f.render_stateful_widget(
        List::new(rows).block(Block::bordered().title(title)).highlight_style(HIGHLIGHT),
        left,
        &mut app.wish_state,
    );

    if app.wishlist.is_empty() {
        let hint = vec![
            Line::from("Nothing here yet."),
            Line::from("On the Crafting tab, press w on a piece to add it.".dark_gray()),
        ];
        f.render_widget(Paragraph::new(hint).block(Block::bordered().title(" Details ")), right);
        return;
    }

    // Top right: what the highlighted piece needs on its own.
    let selected = app.wish_state.selected().and_then(|i| app.wishlist.get(i)).copied();
    let mut piece_title = " Selected ".to_string();
    let mut piece_lines: Vec<Line> = Vec::new();
    if let Some((kind, id)) = selected {
        piece_title = format!(" {} ", app.game.equipment_name(kind, id).unwrap_or("?"));
        if app.save.owns_equipment(kind, id) {
            piece_lines.push(Line::from("You already own this piece (left out of the shopping list).".green()));
            // still show what making another copy takes; `stock all` covers it
            if let Some(plan) = app.plan(kind, id) {
                for m in &plan.materials {
                    let have = app.save.item_count(m.id);
                    piece_lines.push(Line::from(
                        format!("  {:<24}{have:>3} / {:<3}", app.game.item_name(m.id).unwrap_or("?"), m.count).dark_gray(),
                    ));
                }
            }
        } else if let Some(plan) = app.plan(kind, id) {
            let how = match (plan.via, plan.parent_owned) {
                (Via::Create, _) => "Create from scratch",
                (Via::Upgrade, true) => "Upgrade (parent weapon owned)",
                (Via::Upgrade, false) => "Upgrade (you don't own a parent weapon)",
            };
            piece_lines.push(Line::from(how.dark_gray()));
            let route = if plan.via == Via::Create { Route::Create } else { Route::Upgrade };
            piece_lines.push(match app.cost(kind, id, route) {
                Some((c, _)) => Line::raw(format!("Cost: {} z", group_digits(u64::from(c)))),
                None => Line::from("Cost: not seen yet".dark_gray()),
            });
            for m in &plan.materials {
                let have = app.save.item_count(m.id);
                let style = if have >= m.count as u32 {
                    Style::new().green()
                } else {
                    Style::new().red()
                };
                piece_lines.push(Line::from(vec![
                    Span::raw(format!("  {:<24}", app.game.item_name(m.id).unwrap_or("?"))),
                    Span::styled(format!("{have:>3} / {:<3}", m.count), style),
                ]));
            }
            piece_lines.push(if app.can_make_now(kind, id) {
                Line::from("You can make this now.".green())
            } else {
                Line::from("Not available yet.".red())
            });
        } else {
            piece_lines.push(Line::from("No recipe".dark_gray()));
        }
    }
    let top_height = (piece_lines.len() as u16 + 2).min(right.height.saturating_sub(5)).max(3);
    let [top, bottom] = Layout::vertical([Constraint::Length(top_height), Constraint::Min(3)]).areas(right);
    f.render_widget(Paragraph::new(piece_lines).block(Block::bordered().title(piece_title)), top);

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
        lines.push(Line::from(vec![
            Span::raw(format!("  {:<24}", app.game.item_name(item).unwrap_or("?"))),
            Span::styled(
                format!("{have:>3} / {n:<3}"),
                if ok { Style::new().green() } else { Style::new().red() },
            ),
            Span::raw(if ok { String::new() } else { format!("  need {} more", n - have) }),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(if unowned == 0 {
        Line::from("You already own everything on the wishlist.".green())
    } else if missing_kinds == 0 {
        Line::from("You have everything.".green())
    } else {
        Line::from(format!("Short on {missing_kinds} material(s).").red())
    });
    let (known, unknown) = app.wishlist_cost();
    if unowned > 0 {
        let have = u64::from(app.save.zenny);
        let mut text = format!("Zenny: {} z known", group_digits(known));
        if unknown > 0 {
            text += &format!(" + {unknown} piece(s) not seen yet");
        }
        text += &format!(" · you have {}", group_digits(have));
        lines.push(if known > have { Line::from(text.red()) } else { Line::raw(text) });
    }
    let title = format!(" Shopping list · all {unowned} unowned piece(s) ");
    f.render_widget(Paragraph::new(lines).block(Block::bordered().title(title)), bottom);
}

use crate::app::{App, Offer, Tab, TreeView, Via, group_digits, signed_zenny};
use crate::theme::{self, accent, bad, bold, good, muted, warn};
use mh3u_core::prices::{Route, Source};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Tabs, Wrap},
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
        Span::styled(app.save.hunter_name.clone(), theme::plain().add_modifier(Modifier::BOLD)),
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
        Tab::Worn => draw_worn(f, app, body),
        Tab::Monsters => draw_monsters(f, app, body),
        Tab::Crafting => draw_crafting(f, app, body),
        Tab::Wishlist => draw_wishlist(f, app, body),
    }

    draw_footer(f, app, footer);

    if app.show_help {
        draw_help(f, app);
    }
    if app.tree.is_some() {
        draw_tree(f, app);
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
                keys.extend([
                    ("c", "craftable"),
                    ("o", "hide owned"),
                    ("b", "blacksmith"),
                    ("s", "sort"),
                    ("w", "wish"),
                    ("t", "tree"),
                ]);
            }
            Tab::Items => {
                keys.extend([("↑/↓", "move"), ("p", "pouch/box"), ("/", "search")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.push(("s", "sort"));
            }
            Tab::Wishlist => keys.extend([("↑/↓", "move"), ("w", "remove"), ("t", "tree")]),
            Tab::Equipment => keys.extend([("↑/↓", "move"), ("s", "sort"), ("t", "tree"), ("i", "skill info")]),
            Tab::Worn => keys.push(("i", "skill info")),
            Tab::Monsters => keys.extend([("↑/↓", "move"), ("PgUp/PgDn", "scroll drops"), ("s", "sort")]),
        }
        keys.extend([("?", "help"), ("q", "quit")]);
        theme::key_hints(&keys)
    };
    spans.push(Span::raw("   "));
    spans.push(Span::styled(app.status.clone(), muted()));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// The upgrade tree popup: the line down to the weapon, then everything it upgrades into.
fn draw_tree(f: &mut Frame, app: &mut App) {
    let Some(view) = app.tree.take() else { return };
    let kind = view.kind;
    let kind_label = app.game.equipment_kind_label(kind).unwrap_or("Weapon");
    let mut lines: Vec<Line> = Vec::new();
    for row in &view.tree.rows {
        let name = app.game.equipment_name(kind, row.id).unwrap_or("?");
        let owned = app.save.owns_equipment(kind, row.id);
        let mut spans = vec![Span::styled(row.prefix.clone(), theme::faint())];
        let marker = if owned { "● " } else { "○ " };
        let name_style = if row.selected {
            theme::selection()
        } else if owned {
            good()
        } else {
            Style::new()
        };
        spans.push(Span::styled(marker, if owned { good() } else { muted() }));
        spans.push(Span::styled(name.to_string(), name_style));
        if row.selected {
            spans.push(Span::styled(" ◀", accent()));
        }
        if let Some(r) = app.game.equipment_rarity(kind, row.id) {
            spans.push(Span::raw(" "));
            spans.push(theme::rarity_badge(r));
        }
        if let Some(w) = app.game.weapon_stats(kind, row.id) {
            // the rarity badge is three cells wide (`R4 `, `R10`), so this space keeps `R10` off the attack
            spans.push(Span::styled(format!(" atk {}", w.attack), muted()));
        }
        if !owned && app.can_make_now(kind, row.id) {
            spans.push(Span::styled("  ✔ can make", good()));
        }
        if row.repeat {
            spans.push(Span::styled("  (shown above)", muted()));
        }
        if row.other_branches > 0 {
            spans.push(Span::styled(format!("  +{} other upgrade(s)", row.other_branches), muted()));
        }
        if !row.also_from.is_empty() {
            let names: Vec<&str> = row
                .also_from
                .iter()
                .map(|&p| app.game.equipment_name(kind, p).unwrap_or("?"))
                .collect();
            spans.push(Span::styled(format!("  also from {}", names.join(", ")), muted()));
        }
        lines.push(Line::from(spans));
    }
    if view.tree.omitted > 0 {
        lines.push(Line::styled(format!("… and {} more", view.tree.omitted), muted()));
    }
    let area = f.area();
    let wanted = lines.len() as u16 + 2;
    let (w, h) = (area.width.min(96), wanted.clamp(5, area.height.saturating_sub(2).max(5)));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    let visible = usize::from(h.saturating_sub(2));
    let max_scroll = lines.len().saturating_sub(visible) as u16;
    let scroll = view.scroll.min(max_scroll);
    let title = format!(" {kind_label} upgrade tree ");
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(lines)
            .scroll((scroll, 0))
            .block(theme::pane(title, true).title_bottom(Line::from(key_line(&[("↑/↓", "scroll"), ("t", "close")])).right_aligned())),
        popup,
    );
    scrollbar(
        f,
        popup,
        view.tree.rows.len() + usize::from(view.tree.omitted > 0),
        Some(usize::from(scroll)),
    );
    app.tree = Some(TreeView { scroll, ..view });
}

fn key_line(pairs: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut spans = vec![Span::raw(" ")];
    spans.extend(theme::key_hints(pairs));
    spans.push(Span::raw(" "));
    spans
}

fn draw_help(f: &mut Frame, app: &App) {
    let text = "\
Move          ↑/↓ or j/k · PageUp/PageDown
Top / bottom  Home/End or g/G
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
              b  only pieces the blacksmith is offering (see below)
              u  only pieces you own that have no price recorded yet
              s  sort (game order, name, craftable first, owned first)
              w  add to the wishlist (★), with the parent weapons
                 it needs; press again to remove it
Wishlist      w or x  remove the selected piece, and the parents
                 that were added for it if nothing else needs them
Equipment     s  sort (box order, name, rarity, type, worn first)
Worn          totals for what you are wearing; i shows what each
                 skill does
Monsters      what each monster drops; s sort, ★ = the wishlist
                 still needs it
Any weapon    t  upgrade tree: the line down to it and everything
                 it upgrades into (↑/↓ scroll, t or Esc close)

The blacksmith line follows what the save shows: a piece is on offer
once a monster that drops its first material has been hunted (killed
or captured) at least once; materials you only hold do not count.
Pieces made from high-rank drops (the S, X... sets) need a hunt in
that rank. The game never takes a piece off the list, so the app
remembers every piece it has seen on offer. An anvil marks those in the
crafting list, and the unowned ones on the wishlist.

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
    let lines = text.lines().count() as u16;
    let (w, h) = (area.width.min(74), area.height.min(lines + 2));
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
            .style(theme::faint()),
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
    let [box_area, details_area] = Layout::vertical([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(right);
    let rows = |stacks: &[mh3u_core::save::ItemStack]| -> Vec<ListItem<'static>> {
        stacks
            .iter()
            .map(|s| {
                ListItem::new(Line::from(vec![
                    Span::raw(format!("{:<26}", app.game.item_name(s.id).unwrap_or("?"))),
                    Span::styled(format!(" x{}", s.count), muted()),
                ]))
            })
            .collect()
    };
    let pouch = rows(&app.pouch_view);
    let item_box = rows(&app.box_view);
    let query = app.item_search.trim();
    let focus_pouch = app.items_on_pouch();
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
    // Both lists reserve the room for the selection mark, so their names line up; only the focused one shows a highlight.
    let list = |rows: Vec<ListItem<'static>>, title: String, active: bool| {
        List::new(rows)
            .block(theme::pane(title, active))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK)
            .highlight_spacing(ratatui::widgets::HighlightSpacing::Always)
    };
    let (pouch_len, box_len) = (pouch.len(), item_box.len());
    if pouch.is_empty() {
        let what = if query.is_empty() {
            "Nothing in the pouch."
        } else {
            "No pouch items match."
        };
        empty_pane(f, left, pouch_title, focus_pouch, vec![Line::styled(what, muted())]);
    } else {
        let mut unfocused = ListState::default();
        let state = if focus_pouch { &mut app.pouch_state } else { &mut unfocused };
        f.render_stateful_widget(list(pouch, pouch_title, focus_pouch), left, state);
        scrollbar(f, left, pouch_len, app.pouch_state.selected().filter(|_| focus_pouch));
    }
    if item_box.is_empty() {
        let what = if query.is_empty() {
            "The item box is empty."
        } else {
            "No box items match. Press x to clear the search."
        };
        empty_pane(f, box_area, box_title, !focus_pouch, vec![Line::styled(what, muted())]);
    } else {
        let mut unfocused = ListState::default();
        let state = if focus_pouch { &mut unfocused } else { &mut app.box_state };
        f.render_stateful_widget(list(item_box, box_title, !focus_pouch), box_area, state);
        scrollbar(f, box_area, box_len, app.box_state.selected().filter(|_| !focus_pouch));
    }

    let highlighted = if focus_pouch {
        app.pouch_state.selected().and_then(|i| app.pouch_view.get(i))
    } else {
        app.box_state.selected().and_then(|i| app.box_view.get(i))
    };
    let lines = match highlighted {
        Some(stack) => item_details(app, stack.id),
        None => Vec::new(),
    };
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Details ", false)),
        details_area,
    );
}

/// An item: what the game says it is, how many you hold, what the wishlist needs of it and which pieces are made with it.
fn item_details(app: &App, id: u16) -> Vec<Line<'static>> {
    let mut lines = vec![Line::styled(app.game.item_name(id).unwrap_or("?").to_string(), bold())];
    lines.push(Line::from(vec![
        Span::styled("In the pouch ", muted()),
        Span::raw(
            app.save
                .pouch
                .iter()
                .filter(|s| s.id == id)
                .map(|s| u32::from(s.count))
                .sum::<u32>()
                .to_string(),
        ),
        Span::styled("  ·  in the box ", muted()),
        Span::raw(
            app.save
                .item_box
                .iter()
                .filter(|s| s.id == id)
                .map(|s| u32::from(s.count))
                .sum::<u32>()
                .to_string(),
        ),
    ]));
    if let Some(text) = app.game.item_description(id) {
        lines.push(Line::raw(text.to_string()));
    }
    let (need, _) = app.shopping_need();
    if let Some(&(_, n)) = need.iter().find(|&&(item, _)| item == id) {
        let have = app.save.item_count(id);
        lines.push(Line::raw(""));
        lines.push(Line::from(vec![
            Span::styled("Wishlist needs ", muted()),
            Span::styled(format!("{have}/{n}"), theme::progress_style(have, n)),
        ]));
    }
    let sources = app.game.drops().sources(id);
    if !sources.is_empty() {
        lines.push(Line::raw(""));
        lines.push(Line::styled("Dropped by", bold()));
        // one row per monster and kind of drop, with the chance at each rank
        let mut rows: std::collections::BTreeMap<(u16, mh3u_core::drops::Method), [Option<u8>; 3]> = Default::default();
        for (monster, rank, method, percent) in sources {
            let slot = mh3u_core::drops::Rank::ALL.iter().position(|&r| r == rank).unwrap_or(0);
            rows.entry((monster, method)).or_default()[slot] = Some(percent);
        }
        const SHOWN: usize = 12;
        let total = rows.len();
        for ((monster, method), chances) in rows.into_iter().take(SHOWN) {
            let mut spans = vec![
                Span::raw(format!("  {:<19}", fit(app.game.monster_name(monster).unwrap_or("?"), 18))),
                Span::styled(format!("{:<15}", method.label()), muted()),
            ];
            for (label, chance) in ["Low", "High", "G"].into_iter().zip(chances) {
                spans.push(Span::styled(format!("{label} "), muted()));
                spans.push(match chance {
                    Some(p) => Span::styled(format!("{p:>3}%  "), good()),
                    None => Span::styled("  –   ", muted()),
                });
            }
            lines.push(Line::from(spans));
        }
        if total > SHOWN {
            lines.push(Line::styled(format!("  … and {} more", total - SHOWN), muted()));
        }
    }
    let users: Vec<String> = app
        .game
        .recipes_using(id)
        .into_iter()
        .filter_map(|(kind, piece)| app.game.equipment_name(kind, piece).filter(|n| !n.is_empty() && *n != "DUMMY"))
        .map(str::to_string)
        .collect();
    if !users.is_empty() {
        lines.push(Line::raw(""));
        const SHOWN: usize = 8;
        let more = users.len().saturating_sub(SHOWN);
        let mut text = users.iter().take(SHOWN).cloned().collect::<Vec<_>>().join(", ");
        if more > 0 {
            text.push_str(&format!(" … and {more} more"));
        }
        lines.push(Line::from(vec![
            Span::styled(format!("Used in {} piece(s): ", users.len()), bold()),
            Span::raw(text),
        ]));
    }
    lines
}

/// The worn gear and what it adds up to.
fn draw_worn(f: &mut Frame, app: &mut App, area: Rect) {
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
    let summary = crate::worn::summarize(&stats);
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
        let (state, style) = if t.active() {
            ("● active", good())
        } else if t.penalty() {
            ("▼ penalty", bad())
        } else if t.points > 0 {
            (&*format!("{} more to activate", crate::worn::ACTIVE_AT - t.points), muted())
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

/// A monster and what it drops, with the items the wishlist still needs picked out.
fn draw_monsters(f: &mut Frame, app: &mut App, area: Rect) {
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
    let title = format!(" Monsters ({}) · {} ", rows.len(), app.monster_sort.label());
    if rows.is_empty() {
        empty_pane(f, left, title, true, vec![Line::styled("No monster drop data.", muted())]);
        return;
    }
    let len = rows.len();
    let mut state = ListState::default().with_selected(selected.and_then(|m| view.iter().position(|&(v, _)| v == m)));
    f.render_stateful_widget(
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
    let scroll = usize::from(app.monster_scroll).min(total.saturating_sub(visible));
    app.monster_scroll = scroll as u16;
    f.render_widget(
        Paragraph::new(lines)
            .scroll((scroll as u16, 0))
            .block(theme::pane(" Drops ", false)),
        right,
    );
    scrollbar(f, right, total, Some(scroll));
}

/// Lay out `items` after `label`, separated by dots, breaking lines to fit `width` and indenting the continuation lines under the
/// first item.
fn wrap_items(label: &str, items: Vec<Vec<Span<'static>>>, width: usize) -> Vec<Line<'static>> {
    const SEP: &str = "  ·  ";
    let indent = " ".repeat(label.chars().count());
    let mut lines = Vec::new();
    let mut spans = vec![Span::styled(label.to_string(), muted())];
    let mut used = label.chars().count();
    let mut first = true;
    for item in items {
        let w: usize = item.iter().map(|s| s.content.chars().count()).sum();
        if !first && used + SEP.chars().count() + w > width {
            lines.push(Line::from(std::mem::replace(&mut spans, vec![Span::raw(indent.clone())])));
            used = indent.chars().count();
        } else if !first {
            spans.push(Span::styled(SEP, muted()));
            used += SEP.chars().count();
        }
        spans.extend(item);
        used += w;
        first = false;
    }
    lines.push(Line::from(spans));
    lines
}

fn monster_details(app: &App, monster: u16, width: usize) -> Vec<Line<'static>> {
    let missing = app.missing_for_wishlist();
    let mut lines = vec![Line::styled(app.game.monster_name(monster).unwrap_or("?").to_string(), bold())];
    lines.push(Line::from(vec![
        Span::styled("Chance in percent. ", muted()),
        Span::styled("★", warn()),
        Span::styled(" marks what your wishlist still needs.", muted()),
    ]));
    let mut current = None;
    for (method, rank, list) in app.game.drops().lists_for(monster) {
        if current != Some(method) {
            lines.push(Line::raw(""));
            lines.push(Line::styled(method.label(), bold()));
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
        "Part breaks are numbered in the game's order; which body part each one is, is not known.",
        muted(),
    ));
    lines
}

fn draw_equipment(f: &mut Frame, app: &mut App, area: Rect) {
    let [left, right] = theme::split(area, 45);
    let rows: Vec<ListItem> = app
        .equip_view
        .iter()
        .map(|&i| {
            let e = &app.save.equipment_box[i];
            let worn = if app.save.is_worn(e) {
                Span::styled("●", good())
            } else {
                Span::raw("")
            };
            let rarity = match app.game.equipment_rarity(e.kind, e.id) {
                Some(r) => theme::rarity_badge(r),
                None => Span::raw("   "),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<14} ", app.game.equipment_kind_label(e.kind).unwrap_or("?")), muted()),
                Span::raw(format!("{:<22}", app.game.equipment_name(e.kind, e.id).unwrap_or("?"))),
                rarity,
                Span::raw(" "),
                worn,
            ]))
        })
        .collect();
    let title = format!(" Equipment Box ({}/1000) · {} ", rows.len(), app.equip_sort.label());
    if rows.is_empty() {
        empty_pane(f, left, title, true, vec![Line::styled("The equipment box is empty.", muted())]);
        f.render_widget(Paragraph::new(Vec::<Line>::new()).block(theme::pane(" Details ", false)), right);
        return;
    }
    let len = rows.len();
    f.render_stateful_widget(
        List::new(rows)
            .block(theme::pane(title, true))
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        left,
        &mut app.equip_state,
    );
    scrollbar(f, left, len, app.equip_state.selected());

    let lines = match app.selected_equipment() {
        Some(e) => {
            let name = app.game.equipment_name(e.kind, e.id).unwrap_or("?").to_string();
            let mut lines = piece_details(app, e.kind, e.id, &name, None);
            for (id, pts) in e.talisman_skills() {
                lines.insert(
                    2,
                    Line::from(vec![
                        Span::raw(format!("  {:<18}", app.game.skill_name(id).unwrap_or("?"))),
                        Span::styled(format!("{pts:+}"), theme::signed_style(i32::from(pts))),
                    ]),
                );
            }
            if app.save.is_worn(e) {
                lines.insert(2, Line::styled("● worn", good()));
            }
            lines
        }
        None => Vec::new(),
    };
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Details (pouch + box) ", false)),
        right,
    );
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
        if app.skill_info
            && let Some(text) = app.game.skill_description(id)
        {
            lines.push(Line::styled(format!("    {text}"), muted()));
        }
    }
    lines
}

/// A weapon's stats: rarity, gem slots, attack and affinity.
fn weapon_lines(w: &mh3u_core::weapons::Weapon) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        theme::rarity_badge(w.rarity),
        Span::raw(" "),
        Span::styled(theme::gems(w.slots), accent()),
        Span::styled(format!("  Attack {}", w.attack), bold()),
    ])];
    if w.affinity != 0 {
        lines.push(Line::from(vec![
            Span::styled("Affinity ", muted()),
            Span::styled(format!("{:+}%", w.affinity), theme::signed_style(i32::from(w.affinity))),
        ]));
    }
    lines
}

/// Whether the blacksmith offers a piece, by the rule in `mh3u_core::blacksmith` and from pieces seen on offer before.
fn unlock_line(app: &App, kind: u8, id: u16) -> Option<Line<'static>> {
    use mh3u_core::{blacksmith::Unlock, drops::Rank};
    let label = |text: &str, style: Style| -> Line<'static> {
        Line::from(vec![Span::styled("Blacksmith  ", muted()), Span::styled(text.to_string(), style)])
    };
    let rank = |rank: Rank| match rank {
        Rank::Low => "low rank",
        Rank::High => "high rank (6★+ quests when playing alone)",
        Rank::G => "G rank (after Throne of the Abyss)",
    };
    let monsters = |ids: &[u16]| {
        let mut names: Vec<&str> = ids.iter().filter_map(|&m| app.game.monster_name(m)).collect();
        names.dedup();
        let more = names.len().saturating_sub(4);
        names.truncate(4);
        let mut text = names.join(", ");
        if more > 0 {
            text += &format!(" and {more} more");
        }
        text
    };
    Some(match app.offer(kind, id)? {
        Offer::Earlier => label("✔ on offer (seen earlier; the list never shrinks)", good()),
        Offer::Rule(Unlock::Starter) => label("✔ starting gear, always on offer", good()),
        Offer::Rule(Unlock::Hunted(m)) => label(
            &format!(
                "✔ on offer (you have hunted {}, which drops its first material)",
                app.game.monster_name(m).unwrap_or("?")
            ),
            good(),
        ),
        Offer::Rule(Unlock::NeedsHunt(r, ids)) => label(&format!("not on offer yet: hunt {} in {}", monsters(&ids), rank(r)), warn()),
        Offer::Rule(Unlock::NeedsRank(r)) => label(&format!("not on offer yet: its first material drops only in {}", rank(r)), warn()),
        Offer::Rule(Unlock::Special) => label("a special piece, unlocked some other way", muted()),
        Offer::Rule(Unlock::Unknown(_)) => label("unknown how this is unlocked (its first material is not a monster drop)", muted()),
    })
}

/// The details of one piece of equipment: stats, then what it takes to create or upgrade it with the materials you have.
/// `craftable` is `Some` on the Crafting tab, where the piece may not be owned; `None` on the Equipment tab.
fn piece_details(app: &App, kind: u8, id: u16, name: &str, craftable: Option<bool>) -> Vec<Line<'static>> {
    let zenny = app.save.zenny;
    let mut lines: Vec<Line> = vec![
        Line::styled(name.to_string(), bold()),
        Line::styled(app.game.equipment_kind_label(kind).unwrap_or("?").to_string(), muted()),
    ];
    if let Some(text) = app.game.equipment_description(kind, id) {
        lines.push(Line::styled(text.to_string(), muted()));
    }
    if let Some(a) = app.game.armor_stats(kind, id) {
        lines.extend(armor_lines(app, a));
    } else if let Some(w) = app.game.weapon_stats(kind, id) {
        lines.extend(weapon_lines(w));
    }
    lines.extend(unlock_line(app, kind, id));
    lines.push(Line::raw(""));
    if let Some(recipe) = app.create_recipe(kind, id) {
        lines.push(cost_heading("Create from scratch", app.cost(kind, id, Route::Create), zenny));
        for m in &recipe.materials {
            lines.push(theme::material_line(
                app.game.item_name(m.id).unwrap_or("?"),
                app.save.item_count(m.id),
                u32::from(m.count),
            ));
        }
        lines.push(Line::raw(""));
    }
    if let Some(up) = app.upgrade_recipe(kind, id) {
        lines.push(cost_heading("Upgrade from", app.cost(kind, id, Route::Upgrade), zenny));
        for &parent in &up.parents {
            let parent_name = app.game.equipment_name(kind, parent).unwrap_or("?");
            let (mark, style) = if app.save.owns_equipment(kind, parent) {
                ("● owned", good())
            } else {
                ("○ not owned", bad())
            };
            lines.push(Line::from(vec![
                Span::raw(format!("  {parent_name:<24}")),
                Span::styled(mark, style),
            ]));
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
    let children = app.upgrade_children(kind, id);
    if !children.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Upgrades into", bold()),
            Span::styled("  (t for the tree)", muted()),
        ]));
        for child in children {
            let child_name = app.game.equipment_name(kind, child).unwrap_or("?");
            let (mark, style) = if app.save.owns_equipment(kind, child) {
                ("● owned", good())
            } else {
                ("", muted())
            };
            lines.push(Line::from(vec![
                Span::raw(format!("  {child_name:<24}")),
                Span::styled(mark, style),
            ]));
        }
        lines.push(Line::raw(""));
    }
    if let Some(craftable) = craftable {
        lines.push(if craftable {
            Line::styled("✔ You can make this now.", good())
        } else {
            Line::styled("✘ Not available yet.", bad())
        });
    }
    let any_cost_missing = (app.create_recipe(kind, id).is_some() && app.cost(kind, id, Route::Create).is_none())
        || (app.upgrade_recipe(kind, id).is_some() && app.cost(kind, id, Route::Upgrade).is_none());
    if any_cost_missing {
        lines.push(Line::raw(""));
        lines.push(Line::styled("Costs are learned by watching you craft in live mode.", muted()));
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
            let rarity = match app.game.equipment_rarity(p.kind, p.id) {
                Some(r) => theme::rarity_badge(r),
                None => Span::raw("   "),
            };
            let label = app.game.equipment_kind_label(p.kind).unwrap_or("?");
            let owned = if p.owned { " owned" } else { "" };
            let reason = p.reason.as_deref().map(|r| format!(" · {r}")).unwrap_or_default();
            let anvil = if p.offered { theme::anvil() } else { theme::no_anvil() };
            ListItem::new(Line::from(vec![
                mark,
                star,
                Span::raw(" "),
                anvil,
                Span::raw(format!("{:<24}", fit(&p.name, 24))),
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
    if app.blacksmith_only {
        title.push_str("[at the blacksmith] ");
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

    let lines = match app.craft_state.selected().and_then(|i| app.pieces.get(i)) {
        Some(piece) => piece_details(app, piece.kind, piece.id, &piece.name, Some(piece.craftable)),
        None => Vec::new(),
    };
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(theme::pane(" Recipe (pouch + box) ", false)),
        right,
    );
}

/// `text` cut to at most `width` characters, with an ellipsis when it was cut.
fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(width - 1).collect::<String>())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn item(text: &str) -> Vec<Span<'static>> {
        vec![Span::raw(text.to_string())]
    }

    #[test]
    fn items_wrap_under_the_first_item() {
        let items = vec![item("aaaa 10%"), item("bbbb 20%"), item("cccc 30%")];
        // label (4) + "aaaa 10%" (8) + "  ·  " (5) + "bbbb 20%" (8) = 25
        let lines: Vec<String> = wrap_items("Low ", items.clone(), 25).iter().map(Line::to_string).collect();
        assert_eq!(lines, ["Low aaaa 10%  ·  bbbb 20%", "    cccc 30%"]);
        let one: Vec<String> = wrap_items("Low ", items, 80).iter().map(Line::to_string).collect();
        assert_eq!(one, ["Low aaaa 10%  ·  bbbb 20%  ·  cccc 30%"]);
    }

    #[test]
    fn an_item_wider_than_the_pane_still_gets_its_own_line() {
        let lines = wrap_items("Low ", vec![item("a"), item("a very long item name 99%")], 10);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn fit_cuts_with_an_ellipsis() {
        assert_eq!(fit("Rathalos", 20), "Rathalos");
        assert_eq!(fit("Guild Bard Bolero X", 8), "Guild B…");
    }
}

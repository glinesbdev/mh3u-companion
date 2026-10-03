use crate::app::{App, Availability, Offer, Tab, TreeView, Via, group_digits, signed_zenny};
use crate::theme::{self, accent, bad, bold, good, muted, warn};
use mh3u_core::prices::{Route, Source};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Tabs, Wrap},
};

mod builds;
mod crafting;
mod equipment;
mod families;
mod help;
mod hunts;
mod items;
mod monsters;
mod pieces;
mod quests;
mod tree;
mod wishlist;
mod worn;

// the pieces of the screen that the submodules share (their `use super::*` picks these up)
use builds::{draw_builds, draw_name_prompt, draw_piece_picker, draw_skill_picker};
use crafting::draw_crafting;
use equipment::draw_equipment;
use families::draw_families;
use help::draw_help;
use hunts::draw_hunts;
use items::{draw_items, wrap_items};
use monsters::draw_monsters;
use pieces::{cost_spans, piece_details, unlock_line};
use quests::draw_quests;
use tree::draw_tree;
use wishlist::draw_wishlist;
use worn::{draw_worn, totals_lines};

pub fn draw(f: &mut Frame, app: &mut App) {
    let [tabs, body, footer] = Layout::vertical([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)]).areas(f.area());

    let selected = Tab::ALL.iter().position(|&t| t == app.tab).unwrap_or(0);
    let edit_badge = if app.console.enabled {
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
                Tab::Wishlist => Some(app.wish.items.len()),
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
        Tab::Hunts => draw_hunts(f, app, body),
        Tab::Quests => draw_quests(f, app, body),
        Tab::Families => draw_families(f, app, body),
        Tab::Builds => draw_builds(f, app, body),
    }

    draw_footer(f, app, footer);

    if app.show_help {
        draw_help(f, app);
    }
    if app.tree.is_some() {
        draw_tree(f, app);
    }
    if app.builds.skill_picker.is_some() {
        draw_skill_picker(f, app);
    }
    if app.builds.piece_picker.is_some() {
        draw_piece_picker(f, app);
    }
    draw_name_prompt(f, app);
}

/// The key hints for the current tab, or the prompt while typing a search or command.
fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let clear = !app.active_search().is_empty();
    let mut spans: Vec<Span> = if app.is_commanding() {
        vec![
            Span::styled(": ", accent()),
            Span::raw(format!("{}_", app.console.text)),
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
                keys.extend([("↑/↓", "move"), ("p", "pouch/box"), ("/", "search"), ("u", "spare")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.push(("s", "sort"));
            }
            Tab::Wishlist => keys.extend([("↑/↓", "move"), ("w", "remove"), ("t", "tree")]),
            Tab::Equipment => keys.extend([("↑/↓", "move"), ("s", "sort"), ("t", "tree"), ("i", "skill info")]),
            Tab::Worn => keys.push(("i", "skill info")),
            Tab::Monsters => keys.extend([("↑/↓", "move"), ("PgUp/PgDn", "scroll drops"), ("s", "sort")]),
            Tab::Hunts => keys.extend([("↑/↓", "move"), ("r", "rank"), ("Enter", "drops")]),
            Tab::Quests => {
                keys.extend([("↑/↓", "move"), ("/", "search")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.extend([("s", "sort"), ("PgUp/PgDn", "scroll")]);
            }
            Tab::Families => {
                keys.extend([("↑/↓", "move"), ("/", "search")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.extend([("o", "owned"), ("Enter", "craft")]);
            }
            Tab::Builds => {
                use crate::app::BuildFocus;
                keys.push(("f", "switch list"));
                match app.builds.focus {
                    BuildFocus::Skills => keys.extend([("a", "add skill"), ("+/-", "points"), ("x", "remove"), ("p", "weapon")]),
                    BuildFocus::Sets => keys.extend([("s", "save as template"), ("w", "wish"), ("p", "weapon")]),
                    BuildFocus::Templates => keys.extend([("Enter", "swap piece"), ("n", "from worn"), ("w", "wish"), ("x", "delete")]),
                }
                keys.extend([("o", "pieces"), ("m", "talisman"), ("e", "gender"), ("c", "class")]);
            }
        }
        keys.extend([("?", "help"), ("q", "quit")]);
        theme::key_hints(&keys)
    };
    spans.push(Span::raw("   "));
    spans.push(Span::styled(app.status.clone(), muted()));
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn key_line(pairs: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut spans = vec![Span::raw(" ")];
    spans.extend(theme::key_hints(pairs));
    spans.push(Span::raw(" "));
    spans
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

/// A list pane's look: the highlighted row only stands out in the list that has the keys.
fn focused_list<'a>(rows: Vec<ListItem<'a>>, title: String, focused: bool) -> List<'a> {
    List::new(rows)
        .block(theme::pane(title, focused))
        .highlight_style(if focused { theme::selection() } else { Style::new() })
        .highlight_symbol(if focused { theme::SELECTION_MARK } else { "  " })
}

/// `text` cut to at most `width` characters, with an ellipsis when it was cut.
fn fit(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(width - 1).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_cuts_with_an_ellipsis() {
        assert_eq!(fit("Rathalos", 20), "Rathalos");
        assert_eq!(fit("Guild Bard Bolero X", 8), "Guild B…");
    }
}

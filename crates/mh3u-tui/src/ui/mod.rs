use crate::theme::{self, accent, bad, bold, good, muted, warn};
use mh3u_app::app::{App, Availability, Offer, Tab, TreeView, Via, group_digits, signed_zenny};
use mh3u_app::hits::{Area as HitArea, Focus, Hits, ListHit};
use mh3u_app::select::ListState;
use mh3u_core::prices::{Route, Source};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Tabs, Wrap},
};

mod builds;
mod compare;
mod crafting;
mod equipment;
mod families;
#[cfg(feature = "edit")]
mod forms;
mod gains;
#[cfg(feature = "edit")]
mod give;
mod help;
mod hunters;
mod hunts;
mod items;
mod monsters;
mod pieces;
mod quests;
#[cfg(test)]
mod screen_tests;
mod settings;
mod skills;
mod tree;
mod wishlist;
mod worn;

// the pieces of the screen that the submodules share (their `use super::*` picks these up)
use builds::{draw_builds, draw_name_prompt, draw_piece_picker, draw_skill_picker};
use compare::draw_compare;
use crafting::draw_crafting;
use equipment::draw_equipment;
use families::draw_families;
use help::draw_help;
use hunters::draw_hunter_choice;
use hunts::draw_hunts;
use items::{draw_items, wrap_items};
use monsters::draw_monsters;
use pieces::{cost_spans, piece_details, unlock_line};
use quests::draw_quests;
use settings::draw_settings;
use skills::draw_skills;
use tree::draw_tree;
use wishlist::draw_wishlist;
use worn::{draw_worn, draw_worn_pick, totals_lines};

thread_local! {
    /// What this frame has drawn that can be clicked; handed to the app when the frame is done.
    static HITS: std::cell::RefCell<Hits> = std::cell::RefCell::new(Hits::default());
}

fn hit_area(a: Rect) -> HitArea {
    HitArea::new(a.x, a.y, a.width, a.height)
}

pub fn draw(f: &mut Frame, app: &mut App) {
    theme::apply(&app.config);
    HITS.with(|h| *h.borrow_mut() = Hits::default());
    let [tabs, body, footer] = Layout::vertical([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)]).areas(f.area());

    let selected = Tab::ALL.iter().position(|&t| t == app.tab).unwrap_or(0);
    let edit_badge = match (app.edit_enabled(), app.edits_off_online()) {
        (_, true) => Span::styled("⛔ ONLINE: edits off ", warn().add_modifier(Modifier::BOLD)),
        (true, false) => Span::styled("✎ EDIT ", bad().add_modifier(Modifier::BOLD)),
        (false, false) => Span::raw(""),
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
    let mut tab_titles: Vec<Line> = Tab::ALL
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
    // When all the tabs do not fit, start the row further along so that the selected tab shows (a `‹` says some are hidden).
    let first = first_visible_tab(
        &tab_titles.iter().map(Line::width).collect::<Vec<_>>(),
        selected,
        usize::from(tabs.width.saturating_sub(2)),
    );
    tab_titles = tab_titles.split_off(first);
    if first > 0 {
        tab_titles[0].spans.insert(0, Span::styled("‹ ", muted()));
    }
    // each tab is its title with a cell of padding either side, and a divider after it
    let mut x = tabs.x + 1;
    let right = tabs.x + tabs.width.saturating_sub(1);
    for (title, &tab) in tab_titles.iter().zip(&Tab::ALL[first..]) {
        let width = (title.width() + 2) as u16;
        if x + width > right {
            break;
        }
        HITS.with(|h| h.borrow_mut().tabs.push((HitArea::new(x, tabs.y + 1, width, 1), tab)));
        x += width + 1;
    }
    f.render_widget(
        Tabs::new(tab_titles)
            .select(selected - first)
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
        Tab::Skills => draw_skills(f, app, body),
        Tab::Compare => draw_compare(f, app, body),
        Tab::Builds => draw_builds(f, app, body),
        Tab::Gains => gains::draw_gains(f, app, body),
    }

    draw_footer(f, app, footer);
    HITS.with(|h| {
        let mut h = h.borrow_mut();
        h.tab_lists = h.lists.len();
        h.tab_actions = h.actions.len();
    });

    if app.show_help {
        draw_help(f, app);
    }
    if app.tree.is_some() {
        draw_tree(f, app);
    }
    if app.builds.skill_picker.is_some() {
        draw_skill_picker(f, app);
    }
    if app.hunter_choice.is_some() {
        draw_hunter_choice(f, app);
    }
    if app.settings.is_some() {
        draw_settings(f, app);
    }
    if app.builds.piece_picker.is_some() {
        draw_piece_picker(f, app);
    }
    draw_name_prompt(f, app);
    draw_worn_pick(f, app);
    #[cfg(feature = "edit")]
    {
        forms::draw_edit_menu(f, app);
        give::draw_give(f, app);
        forms::draw_talisman(f, app);
    }
    app.hits = HITS.with(|h| std::mem::take(&mut *h.borrow_mut()));
}

/// The first tab to draw so that the selected one is on screen. `widths` are the tab titles' widths; each takes two cells of padding
/// and a divider. Tabs are dropped from the left only as far as they have to be.
pub(super) fn first_visible_tab(widths: &[usize], selected: usize, room: usize) -> usize {
    const EXTRA: usize = 3; // padding either side and the divider
    const MORE: usize = 2; // the `‹ ` that says tabs are hidden
    let width_from = |first: usize| -> usize {
        widths[first..=selected.min(widths.len().saturating_sub(1))]
            .iter()
            .map(|w| w + EXTRA)
            .sum::<usize>()
            + if first > 0 { MORE } else { 0 }
    };
    if widths.is_empty() {
        return 0;
    }
    let mut first = 0;
    while first < selected && width_from(first) > room {
        first += 1;
    }
    first
}

/// The key hints for the current tab, or the prompt while typing a search or command.
fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let clear = !app.active_search().is_empty();
    let mut spans: Vec<Span> = if app.is_commanding() {
        vec![
            Span::styled(": ", accent()),
            Span::raw(format!("{}_", app.command_text())),
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
                    ("z", "affordable"),
                    ("o", "hide owned"),
                    ("b", "blacksmith"),
                    ("s", "sort"),
                    ("w", "wish"),
                    ("t", "tree"),
                    ("v", "compare"),
                ]);
            }
            Tab::Items => {
                keys.extend([("↑/↓", "move"), ("p", "pouch/box"), ("/", "search"), ("u", "spare")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.push(("s", "sort"));
            }
            Tab::Wishlist => keys.extend([
                ("↑/↓", "move"),
                ("w", "remove"),
                ("d", "done"),
                ("s", "sort"),
                ("e", "export list"),
                ("t", "tree"),
                ("v", "compare"),
            ]),
            Tab::Equipment => {
                keys.extend([("↑/↓", "move"), ("/", "search")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.extend([("s", "sort"), ("t", "tree"), ("v", "compare"), ("i", "skill info")]);
            }
            Tab::Worn => keys.extend([
                ("↑/↓", "move"),
                ("m", "near a tier"),
                ("v", "compare with a template"),
                ("i", "skill info"),
            ]),
            Tab::Monsters => {
                keys.extend([("↑/↓", "move"), ("/", "search")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.extend([("PgUp/PgDn", "scroll drops"), ("s", "sort")]);
            }
            Tab::Hunts => keys.extend([("↑/↓", "move"), ("r", "rank"), ("g", "goal"), ("Enter", "drops")]),
            Tab::Quests => {
                keys.extend([("↑/↓", "move"), ("/", "search")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.extend([("m", "monster"), ("s", "sort"), ("PgUp/PgDn", "scroll")]);
            }
            Tab::Skills => {
                keys.extend([("↑/↓", "move"), ("/", "search")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.extend([("o", "reachable"), ("Enter", "build"), ("PgUp/PgDn", "scroll")]);
            }
            Tab::Compare => keys.extend([("↑/↓", "move"), ("x", "remove"), ("c", "clear"), ("Enter", "craft")]),
            Tab::Gains => keys.push(("↑/↓", "move")),
            Tab::Families => {
                keys.extend([("↑/↓", "move"), ("/", "search")]);
                if clear {
                    keys.push(("x", "clear"));
                }
                keys.extend([("o", "owned"), ("Enter", "craft")]);
            }
            Tab::Builds => {
                use mh3u_app::app::BuildFocus;
                keys.push(("f", "switch list"));
                match app.builds.focus {
                    BuildFocus::Skills => keys.extend([("a", "add skill"), ("+/-", "points"), ("x", "remove"), ("p", "weapon")]),
                    BuildFocus::Sets => keys.extend([("s", "save as template"), ("w", "wish"), ("p", "weapon")]),
                    BuildFocus::Templates => keys.extend([("Enter", "swap piece"), ("n", "from worn"), ("w", "wish"), ("x", "delete")]),
                }
                keys.extend([
                    ("o", "pieces"),
                    ("u", "make now"),
                    ("l", "rarity"),
                    ("t", "rank by"),
                    ("m", "talisman"),
                    ("e", "gender"),
                    ("c", "class"),
                ]);
            }
        }
        keys.extend([("H", "hunter"), ("S", "settings"), ("?", "help"), ("q", "quit")]);
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

/// Draw a list with the app's own selection state. ratatui keeps the scroll position in a state of its own, so it is copied in and
/// the new position copied back.
fn render_list(f: &mut Frame, list: List, area: Rect, state: &mut ListState) {
    render_list_for(f, list, area, state, None);
}

/// [`render_list`] for a list without a border (the rows fill `area`), such as one under a popup's input line.
fn render_list_plain(f: &mut Frame, list: List, area: Rect, state: &mut ListState) {
    draw_list(f, list, area, state, None, false);
}

/// [`render_list`] for one of several lists on a tab: `focus` is what a click on it gives the keys to, and whether it has them now.
fn render_list_for(f: &mut Frame, list: List, area: Rect, state: &mut ListState, focus: Option<(Focus, bool)>) {
    draw_list(f, list, area, state, focus, true);
}

fn draw_list(f: &mut Frame, list: List, area: Rect, state: &mut ListState, focus: Option<(Focus, bool)>, bordered: bool) {
    let len = list.len();
    let mut drawn = ratatui::widgets::ListState::default()
        .with_offset(state.offset())
        .with_selected(state.selected());
    f.render_stateful_widget(list, area, &mut drawn);
    state.set_offset(drawn.offset());
    HITS.with(|h| {
        h.borrow_mut().lists.push(ListHit {
            area: hit_area(area),
            offset: drawn.offset(),
            len,
            selected: state.selected().filter(|_| focus.is_none_or(|(_, has_keys)| has_keys)),
            focus: focus.map(|(f, _)| f),
            bordered,
        });
    });
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
    fn the_tab_row_starts_far_enough_along_to_show_the_selected_tab() {
        let widths = [5, 9, 4, 8, 8, 8, 9, 6, 8, 6, 7, 6]; // 12 tabs
        let all = widths.iter().map(|w| w + 3).sum::<usize>();
        assert_eq!(first_visible_tab(&widths, 3, all), 0, "everything fits");
        assert_eq!(first_visible_tab(&widths, 0, 30), 0, "the first tab is always shown from the start");
        let first = first_visible_tab(&widths, 11, 60);
        assert!(first > 0, "the last tab needs the row to move along");
        let shown: usize = widths[first..].iter().map(|w| w + 3).sum::<usize>() + 2;
        assert!(shown <= 60, "{shown}");
        assert_eq!(first_visible_tab(&[], 0, 10), 0);
    }

    #[test]
    fn fit_cuts_with_an_ellipsis() {
        assert_eq!(fit("Rathalos", 20), "Rathalos");
        assert_eq!(fit("Guild Bard Bolero X", 8), "Guild B…");
    }
}

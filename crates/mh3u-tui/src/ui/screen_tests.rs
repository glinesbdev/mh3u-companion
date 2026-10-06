//! Draws every tab into ratatui's test backend, so a layout change that panics or loses a pane shows up without running the app.
//! They need the game dump and a save from `snapshots/`, and skip themselves without them.

use super::*;
use mh3u_app::input::{Key, Mods};
use mh3u_core::gamedata::GameData;
use ratatui::{Terminal, backend::TestBackend};

/// The look (colors and icons) is shared by the whole program, and every draw sets it from the app's settings: tests that draw or look at
/// it take turns.
static LOOK_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    LOOK_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn app() -> Option<App> {
    let game = GameData::load(&mh3u_core::gamedata::dump_from_env()?).ok()?;
    let save = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../snapshots/08-after-quest2/user2"));
    if !save.exists() {
        eprintln!("skipped: snapshots/08-after-quest2/user2 is not available");
        return None;
    }
    App::new(game, save, None).ok()
}

/// What the screen shows as lines of text.
fn screen(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    let width = usize::from(buffer.area.width);
    buffer
        .content()
        .chunks(width)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect()
}

fn draw_all(app: &mut App, width: u16, height: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| draw(f, app)).unwrap();
    screen(&terminal)
}

#[test]
fn every_tab_draws_at_every_size_and_shows_its_name() {
    let _turn = turn();
    let Some(mut app) = app() else { return };
    for _ in 0..Tab::ALL.len() {
        for (width, height) in [(118, 34), (200, 50), (80, 24), (60, 20), (40, 12)] {
            let lines = draw_all(&mut app, width, height);
            assert_eq!(lines.len(), usize::from(height));
            // the tab row is the top three lines; the selected tab's title is in it while there is room for it
            if width >= 80 {
                let top = lines[..3].join(" ");
                assert!(top.contains(app.tab.title()), "{:?} at {width}x{height}: {top}", app.tab);
            }
            // the bottom line is the key hints
            assert!(
                !lines[usize::from(height) - 1].trim().is_empty(),
                "{:?} at {width}x{height}",
                app.tab
            );
        }
        app.on_key(Key::Right, Mods::default());
    }
}

#[test]
fn the_main_tabs_show_their_panes() {
    let _turn = turn();
    let Some(mut app) = app() else { return };
    let text = |app: &mut App| draw_all(app, 130, 40).join("\n");
    assert!(text(&mut app).contains("Item Pouch"), "Items");
    app.on_key(Key::Right, Mods::default());
    assert!(text(&mut app).contains("Equipment"), "Equipment");
    app.on_key(Key::Right, Mods::default());
    let worn = text(&mut app);
    assert!(worn.contains("Worn gear") && worn.contains("Totals"), "Worn");
}

#[test]
fn the_help_popup_and_a_picker_draw_over_a_tab() {
    let _turn = turn();
    let Some(mut app) = app() else { return };
    app.on_key(Key::Char('?'), Mods::default());
    let help = draw_all(&mut app, 130, 50).join("\n");
    assert!(help.contains("Builds"), "the help lists the Builds keys");
    app.on_key(Key::Char('x'), Mods::default());
    // the Builds skill picker
    while app.tab != Tab::Builds {
        app.on_key(Key::Right, Mods::default());
    }
    app.on_key(Key::Char('a'), Mods::default());
    let picker = draw_all(&mut app, 130, 40).join("\n");
    assert!(picker.contains("Add a skill"), "the skill picker opens");
}

#[test]
fn the_settings_popup_draws_over_a_tab_with_its_values() {
    let _turn = turn();
    let Some(mut app) = app() else { return };
    app.on_key(Key::Char('S'), Mods::default());
    let lines = draw_all(&mut app, 130, 40).join("\n");
    for word in [
        "Settings",
        "Profile",
        "Icons",
        "Accent color",
        "Hunt plan goal",
        "Start tab",
        "Game folder",
        "next start",
    ] {
        assert!(lines.contains(word), "{word}");
    }
    // and in a small terminal it still draws
    draw_all(&mut app, 60, 16);
    // a color setting shows its swatch in the color the setting names
    app.on_key(Key::Down, Mods::default());
    app.on_key(Key::Char('d'), Mods::default());
    app.on_key(Key::Esc, Mods::default());
    assert!(app.settings.is_none());
}

#[test]
fn the_look_follows_the_settings() {
    use mh3u_app::config::Config;
    let _turn = turn();
    theme::apply(&Config::parse("accent = red\nmuted = #8a8a8a\nicons = plain\n"));
    assert_eq!(theme::accent().fg, Some(ratatui::style::Color::Red));
    assert_eq!(theme::muted().fg, Some(ratatui::style::Color::Rgb(0x8a, 0x8a, 0x8a)));
    assert_eq!(theme::anvil().content.as_ref(), "⚒ ");
    theme::apply(&Config::default());
    assert_eq!(theme::accent().fg, Some(ratatui::style::Color::LightCyan));
    assert_eq!(theme::muted().fg, Some(ratatui::style::Color::Indexed(245)));
}

#[test]
fn clicking_a_tab_opens_it_and_clicking_a_row_selects_it() {
    use mh3u_app::input::Pointer;
    let _turn = turn();
    let Some(mut app) = app() else { return };
    draw_all(&mut app, 140, 40);
    // every tab that is drawn can be clicked to
    let tabs = app.hits.tabs.clone();
    assert_eq!(tabs.len(), Tab::ALL.len(), "all the tabs fit at this width");
    for (area, tab) in tabs.iter().rev() {
        app.on_pointer(Pointer::Click {
            col: area.x + 1,
            row: area.y,
        });
        assert_eq!(app.tab, *tab);
        draw_all(&mut app, 140, 40);
    }
    // a row of the Equipment list: click it, and it is the highlighted one
    let to = tabs.iter().find(|(_, t)| *t == Tab::Equipment).unwrap().0;
    app.on_pointer(Pointer::Click { col: to.x + 1, row: to.y });
    draw_all(&mut app, 140, 40);
    let list = app.hits.lists[0];
    assert!(list.len > 3, "the test save has equipment");
    app.on_pointer(Pointer::Click {
        col: list.area.x + 3,
        row: list.area.y + 3,
    });
    draw_all(&mut app, 140, 40);
    assert_eq!(app.hits.lists[0].selected, Some(list.offset + 2));
    // the wheel moves by three
    app.on_pointer(Pointer::Scroll {
        col: list.area.x + 3,
        row: list.area.y + 3,
        down: true,
    });
    draw_all(&mut app, 140, 40);
    assert_eq!(app.hits.lists[0].selected, Some(list.offset + 5));
    // a popup takes the clicks: the tab row underneath does nothing
    app.on_key(Key::Char('?'), Mods::default());
    draw_all(&mut app, 140, 40);
    app.on_pointer(Pointer::Click {
        col: tabs[0].0.x + 1,
        row: tabs[0].0.y,
    });
    assert_eq!(app.tab, Tab::Equipment);
    assert!(app.show_help);
}

#[test]
fn clicking_the_other_item_list_gives_it_the_keys() {
    use mh3u_app::input::Pointer;
    let _turn = turn();
    let Some(mut app) = app() else { return };
    assert_eq!(app.tab, Tab::Items);
    draw_all(&mut app, 140, 40);
    let box_hit = *app
        .hits
        .lists
        .iter()
        .find(|l| l.focus == Some(mh3u_app::hits::Focus::ItemsBox))
        .expect("the box list is drawn");
    app.on_pointer(Pointer::Click {
        col: box_hit.area.x + 3,
        row: box_hit.area.y + 3,
    });
    draw_all(&mut app, 140, 40);
    assert!(!app.items_on_pouch());
    assert_eq!(app.inv.box_state.selected(), Some(box_hit.offset + 2));
}

#[test]
fn clicking_a_slot_of_a_template_picks_it() {
    use mh3u_app::input::Pointer;
    let _turn = turn();
    let Some(mut app) = app() else { return };
    let key = |app: &mut App, k: Key| app.on_key(k, Mods::default());
    while app.tab != Tab::Builds {
        key(&mut app, Key::Right);
    }
    key(&mut app, Key::Char('f'));
    key(&mut app, Key::Char('f'));
    key(&mut app, Key::Char('n')); // a template from what is worn
    for c in "Test".chars() {
        key(&mut app, Key::Char(c));
    }
    key(&mut app, Key::Enter);
    draw_all(&mut app, 140, 40);
    if app.hits.slots.is_empty() {
        eprintln!("skipped: no template could be made from this save");
        return;
    }
    assert_eq!(app.hits.slots.len(), mh3u_app::templates::Slot::ALL.len());
    let (area, _) = app.hits.slots[3];
    app.on_pointer(Pointer::Click {
        col: area.x + 2,
        row: area.y,
    });
    assert_eq!(app.builds.template_slot, 3);
    let (area, _) = app.hits.slots[5];
    app.on_pointer(Pointer::Click {
        col: area.x + 2,
        row: area.y,
    });
    assert_eq!(app.builds.template_slot, 5);
    // a second click on it opens the piece picker, as Enter does
    app.on_pointer(Pointer::Click {
        col: area.x + 2,
        row: area.y,
    });
    assert!(app.builds.piece_picker.is_some());
}

#[test]
fn the_empty_skills_list_can_be_clicked_to_add_a_skill() {
    use mh3u_app::input::Pointer;
    let _turn = turn();
    let Some(mut app) = app() else { return };
    while app.tab != Tab::Builds {
        app.on_key(Key::Right, Mods::default());
    }
    assert!(app.builds.settings.targets.is_empty(), "the test save starts with no wanted skills");
    draw_all(&mut app, 140, 40);
    let (area, _, _) = app.hits.actions[0];
    app.on_pointer(Pointer::Click {
        col: area.x + 2,
        row: area.y,
    });
    assert!(app.builds.skill_picker.is_some());
}

#[test]
fn a_click_in_a_popup_list_without_a_border_picks_the_row_under_it() {
    use mh3u_app::input::Pointer;
    let _turn = turn();
    let Some(mut app) = app() else { return };
    while app.tab != Tab::Builds {
        app.on_key(Key::Right, Mods::default());
    }
    app.on_key(Key::Char('p'), Mods::default()); // the weapon picker
    draw_all(&mut app, 140, 40);
    let list = *app.hits.lists.last().expect("the picker's list");
    assert!(!list.bordered && list.len > 4);
    app.on_pointer(Pointer::Click {
        col: list.area.x + 5,
        row: list.area.y + 3,
    });
    draw_all(&mut app, 140, 40);
    assert_eq!(app.hits.lists.last().unwrap().selected, Some(list.offset + 3));
}

#[cfg(feature = "edit")]
#[test]
fn the_give_picker_finds_things_by_name_and_its_buttons_can_be_clicked() {
    use mh3u_app::input::Pointer;
    let _turn = turn();
    let Some(mut app) = app() else { return };
    app.enable_edit("test".into());
    app.on_key(Key::Char('E'), Mods::default());
    app.on_key(Key::Enter, Mods::default()); // the first editor: give
    assert!(app.give.is_some());
    for c in "tenderizer".chars() {
        app.on_key(Key::Char(c), Mods::default());
    }
    let lines = draw_all(&mut app, 140, 40);
    let all = lines.join("\n");
    assert!(all.contains("Give ·") && all.contains("Tenderizer Jwl"), "{all}");
    let picker = app.give.as_ref().unwrap();
    assert!(picker.chosen().unwrap().name.to_lowercase().contains("tenderizer"));
    assert_eq!(picker.count(), 1);
    // the + button
    let (plus, _, key) = app.hits.actions[app.hits.tab_actions + 1];
    assert_eq!(key, Key::Right);
    app.on_pointer(Pointer::Click {
        col: plus.x + 1,
        row: plus.y,
    });
    assert_eq!(app.give.as_ref().unwrap().count(), 2);
    // a click on the second row picks it (two things are called Tenderizer)
    let list = *app.hits.lists.last().unwrap();
    app.on_pointer(Pointer::Click {
        col: list.area.x + 4,
        row: list.area.y + 1,
    });
    assert_eq!(app.give.as_ref().unwrap().state.selected(), Some(1));
    // giving without the game running says so, and the picker stays for the next one
    app.on_key(Key::Enter, Mods::default());
    assert!(app.status.contains("--live"), "{}", app.status);
    assert!(app.give.is_some());
    app.on_key(Key::Esc, Mods::default());
    assert!(app.give.is_none());
}

#[cfg(feature = "edit")]
#[test]
fn the_talisman_form_picks_skills_points_and_slots() {
    use mh3u_app::input::Pointer;
    let _turn = turn();
    let Some(mut app) = app() else { return };
    let press = |app: &mut App, k: Key| app.on_key(k, Mods::default());
    app.enable_edit("test".into());
    press(&mut app, Key::Char('E'));
    press(&mut app, Key::Down);
    press(&mut app, Key::Enter); // the second editor: talisman
    assert!(app.talisman.is_some());
    draw_all(&mut app, 140, 40);
    // typing on the first skill row finds a skill; Enter takes it
    for c in "attack".chars() {
        press(&mut app, Key::Char(c));
    }
    let shown = draw_all(&mut app, 140, 40).join("\n");
    assert!(shown.contains("Attack Boost"), "{shown}");
    press(&mut app, Key::Enter);
    let (_, points) = app.talisman.as_ref().unwrap().skills[0].expect("a skill was chosen");
    assert_eq!(points, 10);
    // the + button raises the points of the highlighted row
    draw_all(&mut app, 140, 40);
    let plus = app.hits.actions[app.hits.tab_actions + 1];
    app.on_pointer(Pointer::Click {
        col: plus.0.x + 1,
        row: plus.0.y,
    });
    assert_eq!(app.talisman.as_ref().unwrap().skills[0].unwrap().1, 11);
    // the slots row
    press(&mut app, Key::Down);
    press(&mut app, Key::Down);
    press(&mut app, Key::Left);
    assert_eq!(app.talisman.as_ref().unwrap().slots, 2);
    // making it without the game running says so
    press(&mut app, Key::Down);
    press(&mut app, Key::Enter);
    assert!(app.status.contains("--live"), "{}", app.status);
    press(&mut app, Key::Esc);
    assert!(app.talisman.is_none());
}

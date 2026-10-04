//! Draws every tab into ratatui's test backend, so a layout change that panics or loses a pane shows up without running the app.
//! They need the game dump and a save from `snapshots/`, and skip themselves without them.

use super::*;
use mh3u_app::input::{Key, Mods};
use mh3u_core::gamedata::GameData;
use ratatui::{Terminal, backend::TestBackend};

fn app() -> Option<App> {
    let game = GameData::load(&mh3u_app::guess_game_dir()?).ok()?;
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

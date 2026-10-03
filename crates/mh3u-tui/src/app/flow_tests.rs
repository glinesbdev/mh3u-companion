//! Whole-app checks: keys in, state out. They need the game dump and a save from `snapshots/`, and skip themselves without them.

use super::*;

fn press(app: &mut App, keys: &str) {
    for c in keys.chars() {
        app.on_key(KeyCode::Char(c), KeyModifiers::NONE);
    }
}

fn key(app: &mut App, code: KeyCode) {
    app.on_key(code, KeyModifiers::NONE);
}

/// An app on the second hunter's save (after the Arzuros quest) with its files in a fresh temporary folder.
fn app_in(dir: &std::path::Path) -> Option<App> {
    let game = GameData::load(&crate::guess_game_dir()?).ok()?;
    let save = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../snapshots/08-after-quest2/user2"));
    if !save.exists() {
        eprintln!("skipped: snapshots/08-after-quest2/user2 is not available");
        return None;
    }
    let files = Files::in_dirs(&dir.join("config"), &dir.join("data"), 2);
    App::new(game, save, Some(files)).ok()
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mh3u-flow-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn tabs_wrap_around_and_the_help_scrolls_and_closes() {
    let dir = temp_dir("tabs");
    let Some(mut app) = app_in(&dir) else { return };
    assert_eq!(app.tab, Tab::Items);
    key(&mut app, KeyCode::Left);
    assert_eq!(app.tab, Tab::Builds, "left of the first tab is the last");
    key(&mut app, KeyCode::Right);
    assert_eq!(app.tab, Tab::Items);
    press(&mut app, "?");
    assert!(app.show_help);
    press(&mut app, "jj");
    assert_eq!(app.help_scroll, 2);
    press(&mut app, "x");
    assert!(!app.show_help && app.help_scroll == 0, "any other key closes it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_crafting_filters_toggle_and_narrow_the_list() {
    let dir = temp_dir("crafting");
    let Some(mut app) = app_in(&dir) else { return };
    for _ in 0..3 {
        key(&mut app, KeyCode::Right);
    }
    assert_eq!(app.tab, Tab::Crafting);
    let all = app.craft.pieces.len();
    press(&mut app, "b");
    assert!(app.craft.blacksmith_only);
    assert!(app.craft.pieces.len() < all, "only what the blacksmith offers");
    assert!(
        app.craft
            .pieces
            .iter()
            .all(|p| p.offered || p.owned || app.at_blacksmith(p.kind, p.id))
    );
    press(&mut app, "b");
    assert_eq!(app.craft.pieces.len(), all);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_build_is_searched_saved_as_a_template_edited_and_remembered_for_the_hunter() {
    let dir = temp_dir("builds");
    let Some(mut app) = app_in(&dir) else { return };
    key(&mut app, KeyCode::Left); // Builds
    press(&mut app, "a");
    assert!(app.builds.skill_picker.is_some());
    press(&mut app, "auto");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.builds.settings.targets.len(), 1, "Auto-Guard at 10 points");
    assert!(!app.builds.results.is_empty(), "the Pawn Talisman alone reaches it");

    // the sets list, save the first set as a template with the suggested name
    press(&mut app, "fs");
    assert!(app.builds.name_prompt.is_some());
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.builds.templates.len(), 1);
    assert_eq!(app.builds.templates[0].name, "Build 1");

    // the templates list: swap the head for nothing
    press(&mut app, "f");
    assert_eq!(app.builds.focus, BuildFocus::Templates);
    key(&mut app, KeyCode::Enter);
    assert!(app.builds.piece_picker.is_some());
    key(&mut app, KeyCode::Enter); // the first choice empties the slot
    assert!(app.builds.templates[0].pieces.iter().all(|p| p.kind != 5), "no head piece any more");

    // the pool setting cycles and is kept
    press(&mut app, "o");
    assert_eq!(app.builds.settings.pool, builds::Pool::All);

    // a new app on the same hunter's files sees all of it; another hunter's files are separate
    let again = App::new(
        GameData::load(&crate::guess_game_dir().unwrap()).unwrap(),
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../snapshots/08-after-quest2/user2")),
        Some(Files::in_dirs(&dir.join("config"), &dir.join("data"), 2)),
    )
    .unwrap();
    assert_eq!(again.builds.templates, app.builds.templates);
    assert_eq!(again.builds.settings, app.builds.settings);
    assert!(dir.join("config/builds-2.txt").exists() && !dir.join("config/builds.txt").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_hunt_plan_follows_the_wishlist_and_opens_the_monsters_drops() {
    let dir = temp_dir("hunts");
    let Some(mut app) = app_in(&dir) else { return };
    // a few head pieces with a material the hunter lacks and a monster drops
    let lacking: Vec<(u8, u16)> = app
        .game
        .piece_ids(5)
        .filter(|&id| {
            !app.save.owns_equipment(5, id)
                && app.plan(5, id).is_some_and(|p| {
                    p.materials
                        .iter()
                        .any(|m| app.save.item_count(m.id) < u32::from(m.count) && !app.game.drops().sources(m.id).is_empty())
                })
        })
        .take(3)
        .map(|id| (5, id))
        .collect();
    assert!(!lacking.is_empty());
    app.wish.items = lacking;
    app.after_wishlist_change();
    for _ in 0..6 {
        key(&mut app, KeyCode::Right);
    }
    assert_eq!(app.tab, Tab::Hunts);
    assert!(
        !app.hunts.stale && !app.hunts.plan.steps.is_empty(),
        "a plan was made when the tab opened"
    );
    let first = app.hunts.plan.steps[0].monster;
    press(&mut app, "r");
    assert_eq!(app.hunts.filter.label(), "Low rank");
    let shown = app.hunts.plan.steps.first().map(|s| s.monster);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.tab, Tab::Monsters, "Enter shows the monster");
    assert_eq!(app.monsters.selected, shown);
    let _ = first;
    app.wish.items.clear();
    app.after_wishlist_change();
    assert!(app.hunts.stale, "a wishlist change makes the plan stale");
    let _ = std::fs::remove_dir_all(&dir);
}

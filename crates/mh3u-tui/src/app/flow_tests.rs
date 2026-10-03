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
    assert_eq!(app.tab, Tab::Gains, "left of the first tab is the last");
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
    key(&mut app, KeyCode::Left);
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
    press(&mut app, "r");
    assert_eq!(app.hunts.filter.label(), "Low rank");
    let first = app.hunts.plan.steps.first().expect("a step").origin;
    key(&mut app, KeyCode::Enter);
    match first {
        crate::hunts::Origin::Monster { monster, .. } => {
            assert_eq!(app.tab, Tab::Monsters, "Enter shows the monster");
            assert_eq!(app.monsters.selected, Some(monster));
        }
        crate::hunts::Origin::Quest(id) => {
            assert_eq!(app.tab, Tab::Quests, "Enter shows the quest");
            assert_eq!(app.quests.selected(app.game.quests()).map(|q| q.id), Some(id));
        }
    }
    app.wish.items.clear();
    app.after_wishlist_change();
    assert!(app.hunts.stale, "a wishlist change makes the plan stale");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_wishlist_follows_the_cheapest_route_and_its_shopping_list_matches_it() {
    let dir = temp_dir("route");
    let Some(mut app) = app_in(&dir) else { return };
    // a great sword whose cheapest way here is a chain of several steps
    let target = app
        .game
        .piece_ids(7)
        .find(|&id| {
            !app.save.owns_equipment(7, id)
                && app
                    .cheapest_path(7, id)
                    .is_some_and(|p| p.steps.len() >= 3 && p.parents_needed().len() >= 2)
        })
        .expect("a great sword that takes a chain of steps");
    let path = app.cheapest_path(7, target).unwrap();
    app.toggle_wish(7, target);
    let mut expected: Vec<(u8, u16)> = path.parents_needed().into_iter().map(|p| (7, p)).collect();
    expected.push((7, target));
    assert_eq!(app.wish.items, expected, "the parents it needs, first to last, then the weapon");

    // every step is planned the way the cheapest route says, so the shopping list is the route's materials
    let (need, unowned) = app.shopping_need();
    assert_eq!(unowned, expected.len());
    let mut got: Vec<(u16, u32)> = need;
    got.sort_unstable();
    let want: Vec<(u16, u32)> = path.materials().into_iter().map(|m| (m.id, u32::from(m.count))).collect();
    assert_eq!(got, want);

    // taking it off takes its automatic parents with it
    app.toggle_wish(7, target);
    assert!(app.wish.items.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_families_tab_groups_armor_searches_and_opens_the_crafting_search() {
    let dir = temp_dir("families");
    let Some(mut app) = app_in(&dir) else { return };
    assert!(app.families.rows.len() > 50, "dozens of armor families");
    for _ in 0..8 {
        key(&mut app, KeyCode::Right);
    }
    assert_eq!(app.tab, Tab::Families);
    press(&mut app, "/arzuros");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.families.rows.len(), 1);
    let family = app.families.selected().expect("a family");
    assert_eq!(family.name, "Arzuros");
    assert_eq!(family.variants().len(), 3, "base, S and X");
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.tab, Tab::Crafting);
    assert_eq!(app.craft.search, "Arzuros");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_spare_filter_keeps_only_items_with_something_to_spare() {
    let dir = temp_dir("spare");
    let Some(mut app) = app_in(&dir) else { return };
    let all = app.inv.box_view.len();
    assert!(
        app.inv.spare.values().all(|p| p.keep + p.spare >= p.spare),
        "keep and spare are counts"
    );
    for stack in app.save.item_box.iter() {
        let p = app.inv.spare[&stack.id];
        assert!(p.spare <= app.save.item_count(stack.id), "never more spare than held");
    }
    press(&mut app, "u");
    assert!(app.inv.spare_only);
    assert!(app.inv.box_view.len() < all, "the hunter holds items things still need");
    assert!(!app.inv.box_view.is_empty(), "and plenty to spare");
    assert!(app.inv.box_view.iter().all(|s| app.inv.spare[&s.id].spare > 0));
    // wishing for a piece that takes an item raises what is kept
    press(&mut app, "u");
    assert_eq!(app.inv.box_view.len(), all);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_weapon_sets_the_armor_class_and_goes_into_a_template_where_it_can_be_swapped() {
    use mh3u_core::armor::ArmorClass;
    let dir = temp_dir("weapon");
    let Some(mut app) = app_in(&dir) else { return };
    key(&mut app, KeyCode::Left);
    key(&mut app, KeyCode::Left); // Builds
    press(&mut app, "aauto");
    key(&mut app, KeyCode::Enter);
    let before = app.builds.pool.len();

    // choose a bow: gunner armor only from then on
    let bow = app.game.piece_ids(17).next().expect("a bow");
    let name = app.game.piece_name(17, bow).unwrap().to_string();
    press(&mut app, "p");
    assert!(app.builds.piece_picker.is_some());
    press(&mut app, &name);
    key(&mut app, KeyCode::Enter);
    let (kind, _) = app.builds.settings.weapon.expect("a weapon was chosen");
    assert_eq!(kind, 17);
    assert_eq!(app.builds.settings.effective_class(), Some(ArmorClass::Gunner));
    assert!(
        app.builds
            .pool
            .iter()
            .filter(|c| c.kind != 6)
            .all(|c| c.stats.class != Some(ArmorClass::Blademaster))
    );
    assert!(app.builds.pool.len() < before, "the blademaster-only armor dropped out");

    // saved as a template with the weapon, and the weapon slot can be emptied
    press(&mut app, "fs");
    key(&mut app, KeyCode::Enter);
    assert!(
        app.builds.templates[0].pieces.iter().any(|p| p.kind == 17),
        "the weapon is in the template"
    );
    press(&mut app, "f");
    press(&mut app, "]]]]]]"); // head, body, arms, waist, legs, talisman, then the weapon
    key(&mut app, KeyCode::Enter);
    key(&mut app, KeyCode::Enter); // the first choice empties the slot
    assert!(app.builds.templates[0].pieces.iter().all(|p| p.kind != 17));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_quests_tab_finds_a_quest_by_its_monster_or_a_reward_and_stars_what_the_wishlist_needs() {
    let dir = temp_dir("quests");
    let Some(mut app) = app_in(&dir) else { return };
    assert!(app.quests.rows.len() > 300, "every quest is listed");
    for _ in 0..7 {
        key(&mut app, KeyCode::Right);
    }
    assert_eq!(app.tab, Tab::Quests);
    press(&mut app, "/arzuros capture");
    key(&mut app, KeyCode::Enter);
    let quest = app.quests.selected(app.game.quests()).expect("a quest");
    assert_eq!((quest.id, quest.title.as_str(), quest.stars), (1204, "Bear Trap", 2));
    assert_eq!(quest.monsters, [42], "Arzuros");

    // search by a reward item instead
    press(&mut app, "x");
    press(&mut app, "/rathian shell");
    key(&mut app, KeyCode::Enter);
    assert!(!app.quests.rows.is_empty());
    let best = &app.game.quests()[app.quests.rows[0].quest];
    assert!(
        best.monsters.contains(&1)
            || best
                .rewards
                .iter()
                .flatten()
                .any(|r| app.game.item_name(r.item) == Some("Rathian Shell")),
        "the best match is a Rathian quest or gives a Rathian Shell: {}",
        best.title
    );

    // a wished piece that takes an Arzuros part puts a star on the quests that give it
    press(&mut app, "x");
    let piece = app
        .game
        .piece_ids(5)
        .find(|&id| app.game.piece_name(5, id) == Some("Arzuros Helm S"))
        .expect("Arzuros Helm S");
    app.wish.items = vec![(5, piece)];
    app.after_wishlist_change();
    assert!(
        app.quests.rows.iter().any(|r| r.needed > 0),
        "some quest gives a part the wishlist lacks"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_hunt_plan_sends_you_to_a_quest_for_what_no_monster_drops() {
    use crate::hunts::Origin;
    let dir = temp_dir("questplan");
    let Some(mut app) = app_in(&dir) else { return };
    // a head piece with a missing material that no monster drops but some quest rewards
    let quest_items: HashSet<u16> = app
        .game
        .quests()
        .iter()
        .filter(|q| q.stars > 0)
        .flat_map(|q| q.rewards.iter().flatten().map(|r| r.item))
        .collect();
    let piece = app
        .game
        .piece_ids(5)
        .find(|&id| {
            !app.save.owns_equipment(5, id)
                && app.plan(5, id).is_some_and(|p| {
                    p.materials.iter().any(|m| {
                        app.save.item_count(m.id) < u32::from(m.count)
                            && app.game.drops().sources(m.id).is_empty()
                            && quest_items.contains(&m.id)
                    })
                })
        })
        .expect("a piece that needs a quest reward");
    app.wish.items = vec![(5, piece)];
    app.after_wishlist_change();
    for _ in 0..6 {
        key(&mut app, KeyCode::Right);
    }
    assert!(
        app.hunts.plan.steps.iter().any(|s| matches!(s.origin, Origin::Quest(_))),
        "a quest step: {:?}",
        app.hunts.plan
    );
    // quests only count when every rank is allowed
    press(&mut app, "r");
    assert!(app.hunts.plan.steps.iter().all(|s| matches!(s.origin, Origin::Monster { .. })));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn m_on_a_quest_shows_its_monster_and_asks_which_when_there_are_several() {
    let dir = temp_dir("questmonster");
    let Some(mut app) = app_in(&dir) else { return };

    // one monster: straight to the Monsters tab
    app.show_quest(1204);
    press(&mut app, "m");
    assert_eq!(app.tab, Tab::Monsters);
    assert_eq!(app.monsters.selected, Some(42), "Arzuros");

    // two monsters (a Barroth and a Great Jaggi): a list to choose from
    app.show_quest(1403);
    press(&mut app, "m");
    assert_eq!(app.tab, Tab::Quests, "still on the quest while choosing");
    let choice = app.quests.choosing.as_ref().expect("a list of monsters");
    assert_eq!(choice.monsters, [8, 12]);
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert!(app.quests.choosing.is_none());
    assert_eq!((app.tab, app.monsters.selected), (Tab::Monsters, Some(12)));

    // Esc closes the list without going anywhere
    app.show_quest(1403);
    press(&mut app, "m");
    key(&mut app, KeyCode::Esc);
    assert!(app.quests.choosing.is_none());
    assert_eq!(app.tab, Tab::Quests);

    // a quest with no large monster says so
    app.show_quest(1308);
    press(&mut app, "m");
    assert_eq!(app.tab, Tab::Quests);
    assert!(app.status.contains("no large monster"), "{}", app.status);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn live_mode_follows_the_hunter_the_game_loads_to_that_slots_lists() {
    let snapshots = concat!(env!("CARGO_MANIFEST_DIR"), "/../../snapshots/");
    let (Ok(first), Ok(second)) = (
        std::fs::read(format!("{snapshots}03-latest/user1")),
        std::fs::read(format!("{snapshots}08-after-quest2/user2")),
    ) else {
        eprintln!("skipped: the snapshots are not available");
        return;
    };
    let Some(game_dir) = crate::guess_game_dir() else { return };
    let dir = temp_dir("follow");
    let saves = dir.join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("user1"), &first).unwrap();
    std::fs::write(saves.join("user2"), &second).unwrap();
    // slot 2 has its own wishlist and skills
    let files = |slot| Files::in_dirs(&dir.join("config"), &dir.join("data"), slot);
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::fs::write(files(2).wishlist, "5 2\n").unwrap();
    std::fs::write(files(2).builds, "skill 37 10\npool all\n").unwrap();
    std::fs::write(files(1).wishlist, "4 11\n4 12\n").unwrap();

    let mut app = App::new(GameData::load(&game_dir).unwrap(), saves.join("user1"), Some(files(1))).unwrap();
    let (name1, name2) = (app.save.hunter_name.clone(), Save::parse(&second).unwrap().hunter_name);
    assert_ne!(name1, name2);
    assert_eq!(app.wish.items, [(4, 11), (4, 12)], "slot 1's wishlist");
    assert!(app.builds.settings.targets.is_empty());

    // the game loads the second hunter
    let live = Save::parse(&second).unwrap();
    let message = app.follow_hunter(&live).expect("a switch");
    assert!(message.contains(&name2) && message.contains("slot 2"), "{message}");
    assert_eq!(app.wish.items, [(5, 2)], "slot 2's wishlist");
    assert_eq!(app.builds.settings.targets.len(), 1);
    assert_eq!(app.builds.settings.pool, builds::Pool::All);
    assert_eq!(app.save_path, saves.join("user2"), "the fallback save file follows");
    assert_eq!(app.slot, Some(2));

    // a change now goes into slot 2's files, and the same hunter again changes nothing
    app.toggle_wish(5, 3);
    assert!(std::fs::read_to_string(files(2).wishlist).unwrap().contains("5 3"));
    assert_eq!(std::fs::read_to_string(files(1).wishlist).unwrap(), "4 11\n4 12\n");
    app.save = Save::parse(&second).unwrap();
    assert!(app.follow_hunter(&live).is_none());
    // and back to the first hunter
    let back = Save::parse(&first).unwrap();
    assert!(app.follow_hunter(&back).is_some());
    assert_eq!(app.wish.items, [(4, 11), (4, 12)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn weapons_are_put_in_the_comparison_with_v_and_taken_out_again() {
    let dir = temp_dir("compare");
    let Some(mut app) = app_in(&dir) else { return };
    // Crafting tab, a great sword
    for _ in 0..3 {
        key(&mut app, KeyCode::Right);
    }
    press(&mut app, "/great sword");
    key(&mut app, KeyCode::Enter);
    press(&mut app, "v");
    assert_eq!(app.compare.weapons.len(), 1);
    key(&mut app, KeyCode::Down);
    press(&mut app, "v");
    assert_eq!(app.compare.weapons.len(), 2);
    press(&mut app, "v"); // the same one again takes it out
    assert_eq!(app.compare.weapons.len(), 1);

    // armor cannot be compared
    press(&mut app, "x");
    press(&mut app, "/jaggi helm");
    key(&mut app, KeyCode::Enter);
    press(&mut app, "v");
    assert_eq!(app.compare.weapons.len(), 1);
    assert!(app.status.contains("only weapons"), "{}", app.status);

    // the tab holds at most four, x removes one, c clears
    press(&mut app, "x");
    press(&mut app, "/sword");
    key(&mut app, KeyCode::Enter);
    for _ in 0..8 {
        press(&mut app, "v");
        key(&mut app, KeyCode::Down);
    }
    assert_eq!(app.compare.weapons.len(), crate::app::MAX_COMPARED);
    assert!(app.status.contains("holds 4"), "{}", app.status);
    for _ in 0..7 {
        key(&mut app, KeyCode::Right);
    }
    assert_eq!(app.tab, Tab::Compare);
    press(&mut app, "x");
    assert_eq!(app.compare.weapons.len(), crate::app::MAX_COMPARED - 1);
    press(&mut app, "c");
    assert!(app.compare.weapons.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn affordable_and_cheapest_first_use_the_forging_fee_against_the_zenny_you_have() {
    let dir = temp_dir("afford");
    let Some(mut app) = app_in(&dir) else { return };
    for _ in 0..3 {
        key(&mut app, KeyCode::Right);
    }
    // the second hunter has plenty of zenny: lower it so that the filter has something to cut
    app.save.zenny = 1000;
    app.refresh_pieces();
    let all = app.craft.pieces.len();
    press(&mut app, "z");
    assert!(app.craft.affordable_only);
    assert!(app.craft.pieces.len() < all);
    assert!(
        app.craft.pieces.iter().all(|p| !p.owned && p.fee.is_some_and(|f| f <= 1000)),
        "only unowned pieces whose fee fits in 1,000 z"
    );
    press(&mut app, "z");
    // cheapest first: fees climb, and pieces with no known fee come last
    while app.craft.sort != PieceSort::Cost {
        press(&mut app, "s");
    }
    let fees: Vec<u32> = app.craft.pieces.iter().map(|p| p.fee.unwrap_or(u32::MAX)).collect();
    assert!(fees.windows(2).all(|w| w[0] <= w[1]), "sorted by fee");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_skills_tab_lists_the_armor_with_a_skill_and_hands_it_to_builds() {
    let dir = temp_dir("skilltab");
    let Some(mut app) = app_in(&dir) else { return };
    for _ in 0..9 {
        key(&mut app, KeyCode::Right);
    }
    assert_eq!(app.tab, Tab::Skills);
    press(&mut app, "/attack");
    key(&mut app, KeyCode::Enter);
    let skill = app.skills.selected().expect("a skill");
    assert_eq!(app.game.skill_name(skill), Some("Attack"));
    let pieces = app.skills.pieces_with(skill);
    assert!(!pieces.is_empty());
    assert!(pieces.windows(2).all(|w| w[0].points >= w[1].points), "most points first");
    assert!(pieces.iter().all(|p| {
        app.game
            .armor_stats(p.kind, p.id)
            .is_some_and(|a| a.skills.contains(&(skill, p.points)))
    }));
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.tab, Tab::Builds);
    assert!(app.builds.settings.targets.iter().any(|t| t.skill == skill && t.points == 10));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn h_lists_the_hunters_in_the_save_slots_and_shows_the_one_picked() {
    let snapshots = concat!(env!("CARGO_MANIFEST_DIR"), "/../../snapshots/");
    let (Ok(first), Ok(second)) = (
        std::fs::read(format!("{snapshots}03-latest/user1")),
        std::fs::read(format!("{snapshots}08-after-quest2/user2")),
    ) else {
        eprintln!("skipped: the snapshots are not available");
        return;
    };
    let Some(game_dir) = crate::guess_game_dir() else { return };
    let dir = temp_dir("picker");
    let saves = dir.join("saves");
    std::fs::create_dir_all(&saves).unwrap();
    std::fs::write(saves.join("user1"), &first).unwrap();
    std::fs::write(saves.join("user2"), &second).unwrap();
    let files = |slot| Files::in_dirs(&dir.join("config"), &dir.join("data"), slot);
    std::fs::create_dir_all(dir.join("config")).unwrap();
    std::fs::write(files(2).wishlist, "5 2\n").unwrap();
    let mut app = App::new(GameData::load(&game_dir).unwrap(), saves.join("user1"), Some(files(1))).unwrap();
    let (name1, name2) = (app.save.hunter_name.clone(), Save::parse(&second).unwrap().hunter_name);

    press(&mut app, "H");
    let choice = app.hunter_choice.as_ref().expect("the list of hunters");
    assert_eq!(
        choice.slots,
        [(1, name1.clone()), (2, name2.clone())],
        "an empty slot is not listed"
    );
    key(&mut app, KeyCode::Esc);
    assert!(app.hunter_choice.is_none());

    press(&mut app, "H");
    key(&mut app, KeyCode::Down);
    key(&mut app, KeyCode::Enter);
    assert_eq!(app.save.hunter_name, name2);
    assert_eq!(app.slot_shown(), Some(2));
    assert_eq!(app.wish.items, [(5, 2)], "slot 2's wishlist");
    assert!(app.status.contains("slot 2"), "{}", app.status);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn live_pickups_are_logged_starred_when_wanted_and_saved_per_hunter() {
    let dir = temp_dir("pickups");
    let Some(mut app) = app_in(&dir) else { return };
    let mut live = app.save.clone();
    // the first update after connecting is only a starting point
    assert!(app.record_gains(&live).is_none());
    assert!(app.gains.log.entries.is_empty());
    // a moved item adds nothing; a new one does
    let moved = live.item_box.pop();
    if let Some(stack) = moved {
        live.pouch.push(stack);
    }
    assert!(app.record_gains(&live).is_none(), "moving between box and pouch is not a pickup");
    live.item_box.push(ItemStack { id: 1, count: 3 });
    let notice = app.record_gains(&live).expect("a pickup");
    assert!(notice.starts_with("picked up ") && notice.contains("x3"), "{notice}");
    assert_eq!(app.gains.log.entries.len(), 1);
    let file = std::fs::read_to_string(app.files.as_ref().unwrap().gains.clone()).unwrap();
    assert!(file.contains("\t1:3"), "{file:?}");
    // the game loading another hunter starts over
    app.forget_gain_baseline();
    assert!(app.record_gains(&live).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_wishlisted_piece_is_announced_when_its_materials_arrive() {
    let dir = temp_dir("craftable");
    let Some(mut app) = app_in(&dir) else { return };
    // the first piece (by id) of head armor that is not craftable now and that has a recipe of at most two materials
    let found = app.game.piece_ids(5).find_map(|id| {
        let r = app.game.recipe(5, id)?;
        (!app.can_make_now(5, id) && r.materials.len() <= 2 && !r.materials.is_empty()).then_some((id, r.clone()))
    });
    let Some((id, recipe)) = found else { return };
    app.wish.items = vec![(5, id)];
    let before = app.craftable_wishes();
    assert!(!before.contains(&(5, id)));
    for m in &recipe.materials {
        app.save.item_box.retain(|s| s.id != m.id);
        app.save.item_box.push(ItemStack { id: m.id, count: m.count });
    }
    if !app.can_make_now(5, id) {
        return; // a recipe with another condition (money, a parent): nothing to announce
    }
    let notice = app.newly_craftable(&before).expect("a notice");
    assert!(notice.starts_with("now craftable: "), "{notice}");
    assert!(app.newly_craftable(&app.craftable_wishes()).is_none(), "only what is new");
    let _ = std::fs::remove_dir_all(&dir);
}

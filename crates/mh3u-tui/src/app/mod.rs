use crate::builds::{self, Candidate, Found, Settings, Target};
use crate::files::Files;
use crate::templates::{self, Template};
use crate::unlocked::Unlocked;
use crate::{commands, search};
use anyhow::{Context, Result};
use mh3u_core::{
    armor::{ArmorClass, Gender},
    edit,
    gamedata::GameData,
    live::{LiveEvent, LiveReader},
    prices::{Ledger, Outcome, PriceEntry, PriceTracker, RecipeBook, Recorded, Route, Skip, SkipReason, Source},
    recipes::{Recipe, Upgrade},
    save::{ItemStack, Save},
};
use ratatui::{
    DefaultTerminal,
    crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    widgets::ListState,
};
use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

mod blacksmith;
mod build_manager;
mod crafting;
mod families;
#[cfg(test)]
mod flow_tests;
mod hunts;
mod inventory;
mod keys;
mod live;
mod money;
mod monsters;
mod price_watch;
mod quests;
mod sorting;
mod wishlist;

pub use blacksmith::Offer;
pub use build_manager::{Availability, BuildFocus, BuildManager, NameAction};
pub use crafting::Crafting;
pub use crafting::Via;
pub use families::FamiliesTab;
pub use hunts::HuntTab;
pub use inventory::Inventory;
pub use inventory::TreeView;
pub use live::EditConsole;
pub use money::{group_digits, signed_zenny};
pub use monsters::MonsterTab;
pub use price_watch::PriceBook;
pub use quests::QuestTab;
pub use sorting::{BoxSort, EquipSort, MonsterSort, PieceSort};
pub use wishlist::WishList;

// the helpers the submodules share (their `use super::*` picks these up)
use crafting::kind_rank;
use money::next_zenny_change;
use wishlist::parse_wishlist;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Items,
    Equipment,
    Worn,
    Crafting,
    Wishlist,
    Monsters,
    Hunts,
    Quests,
    Families,
    Builds,
}

impl Tab {
    pub const ALL: [Tab; 10] = [
        Tab::Items,
        Tab::Equipment,
        Tab::Worn,
        Tab::Crafting,
        Tab::Wishlist,
        Tab::Monsters,
        Tab::Hunts,
        Tab::Quests,
        Tab::Families,
        Tab::Builds,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Tab::Items => "Items",
            Tab::Equipment => "Equipment",
            Tab::Worn => "Worn",
            Tab::Crafting => "Crafting",
            Tab::Wishlist => "Wishlist",
            Tab::Monsters => "Monsters",
            Tab::Hunts => "Hunt plan",
            Tab::Quests => "Quests",
            Tab::Families => "Families",
            Tab::Builds => "Builds",
        }
    }
}

/// A Cemu we started, and the background reader of its memory.
pub struct Live {
    pub reader: LiveReader,
    pub child: std::process::Child,
    /// True while the game's live save data is being read.
    pub connected: bool,
}

/// The game's recipes, for the price tracker.
struct GameBook<'a> {
    game: &'a GameData,
    /// Recipes learned from play, for pieces the game data lacks.
    create: &'a HashMap<(u8, u16), Recipe>,
    upgrade: &'a HashMap<(u8, u16), Upgrade>,
}

pub struct App {
    pub game: GameData,
    pub save: Save,
    pub save_path: PathBuf,
    modified: Option<SystemTime>,
    pub status: String,
    pub tab: Tab,
    /// The Items and Equipment tabs.
    pub inv: Inventory,
    /// The Crafting tab.
    pub craft: Crafting,
    pub wish: WishList,
    pub monsters: MonsterTab,
    /// The Hunt plan tab.
    pub hunts: HuntTab,
    /// The Families tab.
    pub families: FamiliesTab,
    /// The Quests tab.
    pub quests: QuestTab,
    /// The Builds tab.
    pub builds: BuildManager,
    /// Forging costs seen in the game and the watcher that finds them.
    pub costs: PriceBook,
    /// The debug command line (`--debug-edit`).
    pub console: EditConsole,
    /// Pieces seen on offer at the blacksmith, per hunter, saved next to the ledger.
    unlocked: Unlocked,
    /// Where the files live; `None` when the system has no home folder, and nothing is kept between sessions.
    files: Option<Files>,
    /// Set when the TUI started Cemu and is reading its memory (`--live`).
    pub live: Option<Live>,
    /// Typing a search (the text belongs to the tab: `inv.item_search` or `craft.search`).
    pub searching: bool,
    /// Show what each skill does under it in the details panels.
    pub skill_info: bool,
    /// The upgrade tree popup, when open.
    pub tree: Option<TreeView>,
    pub show_help: bool,
    /// First visible line of the help screen (the drawing code keeps it inside the text).
    pub help_scroll: u16,
    /// The recent change in zenny and when it happened (see `zenny_change`).
    zenny_change: Option<(i64, Instant)>,
    /// Waiting for y/n because quitting would end live updates from a Cemu that is still running.
    pub confirm_quit: bool,
    quit: bool,
}

impl App {
    pub fn new(game: GameData, save_path: PathBuf, files: Option<Files>) -> Result<App> {
        let bytes = std::fs::read(&save_path).with_context(|| format!("reading {}", save_path.display()))?;
        let save = Save::parse(&bytes)?;
        let read = |path: Option<&PathBuf>| path.and_then(|p| std::fs::read_to_string(p).ok());
        let entries = read(files.as_ref().map(|f| &f.wishlist))
            .map(|t| parse_wishlist(&t))
            .unwrap_or_default();
        let wishlist: Vec<(u8, u16)> = entries.iter().map(|&(k, i, _)| (k, i)).collect();
        let auto_parents = entries.iter().filter(|e| e.2).map(|&(k, i, _)| (k, i)).collect();
        let builds = BuildManager::load(read(files.as_ref().map(|f| &f.builds)), read(files.as_ref().map(|f| &f.templates)));
        let prices = read(files.as_ref().map(|f| &f.prices))
            .map(|t| Ledger::parse(&t))
            .unwrap_or_default();
        let unlocked = read(files.as_ref().map(|f| &f.unlocked))
            .map(|t| Unlocked::parse(&t))
            .unwrap_or_default();
        let families = App::group_families(&game);
        let quest_tab = QuestTab::new(&game);
        let mut app = App {
            game,
            save,
            modified: modified(&save_path),
            save_path,
            status: String::new(),
            tab: Tab::Items,
            inv: Inventory::default(),
            craft: Crafting::default(),
            wish: WishList::new(wishlist, auto_parents),
            monsters: MonsterTab::default(),
            hunts: HuntTab::default(),
            families: FamiliesTab::new(families),
            quests: quest_tab,
            builds,
            costs: PriceBook::new(prices),
            console: EditConsole::default(),
            unlocked,
            files,
            live: None,
            searching: false,
            skill_info: false,
            tree: None,
            show_help: false,
            help_scroll: 0,
            zenny_change: None,
            confirm_quit: false,
            quit: false,
        };
        app.refresh_families();
        app.refresh_quests();
        app.rebuild_learned();
        app.craft.catalog = app.build_catalog();
        app.learn_unlocked();
        app.inv.uses = app.game.material_uses();
        app.refresh_box();
        app.refresh_families();
        app.refresh_equipment();
        app.refresh_pieces();
        Ok(app)
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.quit {
            terminal.draw(|f| crate::ui::draw(f, self))?;
            if event::poll(Duration::from_millis(250))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.on_key(key.code, key.modifiers);
            }
            self.poll_live();
            self.tick_prices();
            self.reload_if_changed();
        }
        Ok(())
    }

    fn switch_tab(&mut self, step: isize) {
        let i = Tab::ALL.iter().position(|&t| t == self.tab).unwrap_or(0) as isize;
        self.tab = Tab::ALL[(i + step).rem_euclid(Tab::ALL.len() as isize) as usize];
        if self.tab == Tab::Builds && self.builds.stale {
            self.refresh_builds();
        }
        if self.tab == Tab::Hunts && self.hunts.stale {
            self.refresh_hunts();
        }
    }

    fn move_selection(&mut self, step: isize) {
        let (state, len) = match self.tab {
            Tab::Worn => return,
            Tab::Monsters => {
                let view = self.monster_view();
                let at = self.highlighted_monster().and_then(|m| view.iter().position(|&(v, _)| v == m));
                if let Some(&(m, _)) = view.get(stepped(at, step, view.len())) {
                    if self.monsters.selected != Some(m) {
                        self.monsters.scroll = 0;
                    }
                    self.monsters.selected = Some(m);
                }
                return;
            }
            Tab::Items if self.items_on_pouch() => (&mut self.inv.pouch_state, self.inv.pouch_view.len()),
            Tab::Items => (&mut self.inv.box_state, self.inv.box_view.len()),
            Tab::Equipment => (&mut self.inv.equip_state, self.inv.equip_view.len()),
            Tab::Crafting => (&mut self.craft.state, self.craft.pieces.len()),
            Tab::Wishlist => (&mut self.wish.state, self.wish.items.len()),
            Tab::Hunts => (&mut self.hunts.state, self.hunts.plan.steps.len()),
            Tab::Families => (&mut self.families.state, self.families.rows.len()),
            Tab::Quests => {
                self.quests.scroll = 0;
                (&mut self.quests.state, self.quests.rows.len())
            }
            Tab::Builds => match self.builds.focus {
                BuildFocus::Sets => (&mut self.builds.result_state, self.builds.results.len()),
                BuildFocus::Skills => (&mut self.builds.target_state, self.builds.settings.targets.len()),
                BuildFocus::Templates => (&mut self.builds.template_state, self.builds.templates.len()),
            },
        };
        state.select(Some(stepped(state.selected(), step, len)));
    }
}

fn stepped(current: Option<usize>, step: isize, len: usize) -> usize {
    let next = (current.unwrap_or(0) as isize).saturating_add(step);
    next.clamp(0, len.saturating_sub(1) as isize) as usize
}

fn modified(path: &PathBuf) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_stays_inside_the_list_and_home_end_reach_the_edges() {
        assert_eq!(stepped(Some(5), 1, 10), 6);
        assert_eq!(stepped(Some(5), -10, 10), 0);
        assert_eq!(stepped(Some(5), 10, 10), 9);
        assert_eq!(stepped(Some(5), isize::MIN, 10), 0, "Home");
        assert_eq!(stepped(Some(5), isize::MAX, 10), 9, "End");
        assert_eq!(stepped(None, isize::MAX, 10), 9);
        assert_eq!(stepped(None, isize::MAX, 0), 0, "an empty list has nothing to reach");
    }
}

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
mod inventory;
mod keys;
mod live;
mod money;
mod monsters;
mod price_watch;
mod sorting;
mod wishlist;

pub use blacksmith::Offer;
pub use build_manager::{Availability, BuildFocus, BuildManager, NameAction};
pub use crafting::{Piece, Via};
pub use inventory::TreeView;
pub use money::{group_digits, signed_zenny};
pub use sorting::{BoxSort, EquipSort, MonsterSort, PieceSort};

// the helpers the submodules share (their `use super::*` picks these up)
use crafting::{Searchable, kind_rank};
use money::next_zenny_change;
use wishlist::parse_wishlist;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Items,
    Equipment,
    Worn,
    Crafting,
    Wishlist,
    Monsters,
    Builds,
}

impl Tab {
    pub const ALL: [Tab; 7] = [
        Tab::Items,
        Tab::Equipment,
        Tab::Worn,
        Tab::Crafting,
        Tab::Wishlist,
        Tab::Monsters,
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
    pub box_state: ListState,
    pub pouch_state: ListState,
    /// The Items tab's highlight (and details) follow the pouch rather than the box.
    pub pouch_focus: bool,
    pub equip_state: ListState,
    pub craft_state: ListState,
    pub search: String,
    /// Search text for the Items tab (the crafting search is `search`).
    pub item_search: String,
    pub searching: bool,
    pub craftable_only: bool,
    pub hide_owned: bool,
    /// Show only pieces you own that have no forging cost in the ledger yet.
    pub unpriced_only: bool,
    /// Crafting list shows only pieces the blacksmith is offering (see `mh3u_core::blacksmith`).
    pub blacksmith_only: bool,
    pub piece_sort: PieceSort,
    pub box_sort: BoxSort,
    pub equip_sort: EquipSort,
    pub monster_sort: MonsterSort,
    /// The highlighted monster (by id, so the highlight stays on it when the order changes).
    pub monster_selected: Option<u16>,
    /// First visible line of the highlighted monster's drops; the drawing code keeps it inside the text.
    pub monster_scroll: u16,
    /// Show what each skill does under it in the details panels.
    pub skill_info: bool,
    /// The equipment box in display order: indexes into `save.equipment_box` (see `equip_sort`).
    pub equip_view: Vec<usize>,
    /// The upgrade tree popup, when open.
    pub tree: Option<TreeView>,
    /// The item box in display order (see `box_sort`).
    pub box_view: Vec<ItemStack>,
    /// The item pouch, filtered by the item search.
    pub pouch_view: Vec<ItemStack>,
    pub pieces: Vec<Piece>,
    catalog: Vec<Searchable>,
    /// Wishlisted pieces as (equipment kind, piece id), in the order they were added.
    pub wishlist: Vec<(u8, u16)>,
    /// Wishlisted pieces that were added automatically as a parent of another piece (see `remove_wish`).
    auto_parents: HashSet<(u8, u16)>,
    pub wish_state: ListState,
    /// The Builds tab (see `build_manager`).
    pub builds: BuildManager,
    /// Where the files live; `None` when the system has no home folder, and nothing is kept between sessions.
    files: Option<Files>,
    pub show_help: bool,
    /// First visible line of the help screen (the drawing code keeps it inside the text).
    pub help_scroll: u16,
    /// Set when the TUI started Cemu and is reading its memory (`--live`).
    pub live: Option<Live>,
    /// Debug editing is on (`--debug-edit`): `:` opens the command line.
    pub edit_mode: bool,
    commanding: bool,
    pub command: String,
    /// The newest live save block, the base for edit commands.
    live_bytes: Option<Vec<u8>>,
    /// Forging costs seen in the game (see `mh3u_core::prices`) and the watcher that finds them.
    prices: Ledger,
    /// Pieces seen on offer at the blacksmith, per hunter, saved next to the ledger.
    unlocked: Unlocked,
    /// Recipes learned from play (see `prices::Outcome::Learned`), for pieces the game data has no recipe for.
    learned_create: HashMap<(u8, u16), Recipe>,
    learned_upgrade: HashMap<(u8, u16), Upgrade>,
    tracker: PriceTracker,
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
        let mut app = App {
            game,
            save,
            modified: modified(&save_path),
            save_path,
            status: String::new(),
            tab: Tab::Items,
            box_state: ListState::default().with_selected(Some(0)),
            pouch_state: ListState::default().with_selected(Some(0)),
            pouch_focus: false,
            equip_state: ListState::default().with_selected(Some(0)),
            craft_state: ListState::default().with_selected(Some(0)),
            search: String::new(),
            item_search: String::new(),
            searching: false,
            craftable_only: false,
            hide_owned: false,
            unpriced_only: false,
            blacksmith_only: false,
            piece_sort: PieceSort::GameOrder,
            box_sort: BoxSort::BoxOrder,
            equip_sort: EquipSort::BoxOrder,
            monster_sort: MonsterSort::GameOrder,
            monster_selected: None,
            monster_scroll: 0,
            skill_info: false,
            equip_view: Vec::new(),
            tree: None,
            box_view: Vec::new(),
            pouch_view: Vec::new(),
            pieces: Vec::new(),
            catalog: Vec::new(),
            wishlist,
            auto_parents,
            wish_state: ListState::default().with_selected(Some(0)),
            builds: BuildManager::load(read(files.as_ref().map(|f| &f.builds)), read(files.as_ref().map(|f| &f.templates))),
            files: files.clone(),
            show_help: false,
            help_scroll: 0,
            live: None,
            prices: read(files.as_ref().map(|f| &f.prices))
                .map(|t| Ledger::parse(&t))
                .unwrap_or_default(),
            unlocked: read(files.as_ref().map(|f| &f.unlocked))
                .map(|t| Unlocked::parse(&t))
                .unwrap_or_default(),
            learned_create: HashMap::new(),
            learned_upgrade: HashMap::new(),
            tracker: PriceTracker::default(),
            zenny_change: None,
            edit_mode: false,
            commanding: false,
            command: String::new(),
            live_bytes: None,
            confirm_quit: false,
            quit: false,
        };
        app.rebuild_learned();
        app.catalog = app.build_catalog();
        app.learn_unlocked();
        app.refresh_box();
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
    }

    fn move_selection(&mut self, step: isize) {
        let (state, len) = match self.tab {
            Tab::Worn => return,
            Tab::Monsters => {
                let view = self.monster_view();
                let at = self.highlighted_monster().and_then(|m| view.iter().position(|&(v, _)| v == m));
                if let Some(&(m, _)) = view.get(stepped(at, step, view.len())) {
                    if self.monster_selected != Some(m) {
                        self.monster_scroll = 0;
                    }
                    self.monster_selected = Some(m);
                }
                return;
            }
            Tab::Items if self.items_on_pouch() => (&mut self.pouch_state, self.pouch_view.len()),
            Tab::Items => (&mut self.box_state, self.box_view.len()),
            Tab::Equipment => (&mut self.equip_state, self.equip_view.len()),
            Tab::Crafting => (&mut self.craft_state, self.pieces.len()),
            Tab::Wishlist => (&mut self.wish_state, self.wishlist.len()),
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

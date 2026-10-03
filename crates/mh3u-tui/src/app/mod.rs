use crate::builds::{self, Candidate, Found, Settings, Target};
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
mod live;
mod money;
mod monsters;
mod price_watch;
mod sorting;
mod wishlist;

pub use blacksmith::Offer;
pub use build_manager::{BuildFocus, NameAction, NamePrompt, PiecePicker, SkillPicker};
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
    wishlist_path: Option<PathBuf>,
    /// The build manager: wanted skills and options (saved per hunter), the pieces it may use and the sets it found.
    pub build: Settings,
    pub build_pool: Vec<Candidate>,
    pub build_results: Vec<Found>,
    pub build_target_state: ListState,
    pub build_result_state: ListState,
    pub build_focus: BuildFocus,
    /// Saved sets (build templates), per hunter, and the highlighted slot of the highlighted one (an index into `templates::SLOTS`).
    pub templates: Vec<Template>,
    pub template_state: ListState,
    pub template_slot: usize,
    templates_path: Option<PathBuf>,
    pub name_prompt: Option<NamePrompt>,
    pub piece_picker: Option<PiecePicker>,
    pub skill_picker: Option<SkillPicker>,
    /// The save changed since the sets were searched; they are searched again when the Builds tab is next shown.
    build_stale: bool,
    builds_path: Option<PathBuf>,
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
    prices_path: Option<PathBuf>,
    /// Pieces seen on offer at the blacksmith, per hunter, saved next to the ledger.
    unlocked: Unlocked,
    unlocked_path: Option<PathBuf>,
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
    pub fn new(
        game: GameData,
        save_path: PathBuf,
        wishlist_path: Option<PathBuf>,
        builds_path: Option<PathBuf>,
        prices_path: Option<PathBuf>,
    ) -> Result<App> {
        let bytes = std::fs::read(&save_path).with_context(|| format!("reading {}", save_path.display()))?;
        let save = Save::parse(&bytes)?;
        // `builds.txt` goes with `templates.txt`, `builds-2.txt` with `templates-2.txt`
        let templates_path = builds_path.as_ref().and_then(|p| {
            let name = p.file_name()?.to_str()?.strip_prefix("builds")?;
            Some(p.with_file_name(format!("templates{name}")))
        });
        let entries = wishlist_path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
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
            wishlist_path,
            build: builds_path
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|t| Settings::parse(&t))
                .unwrap_or_default(),
            build_pool: Vec::new(),
            build_results: Vec::new(),
            build_target_state: ListState::default().with_selected(Some(0)),
            build_result_state: ListState::default().with_selected(Some(0)),
            build_focus: BuildFocus::Skills,
            templates: templates_path
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|t| templates::parse(&t))
                .unwrap_or_default(),
            template_state: ListState::default().with_selected(Some(0)),
            template_slot: 0,
            templates_path,
            name_prompt: None,
            piece_picker: None,
            skill_picker: None,
            build_stale: true,
            builds_path,
            show_help: false,
            help_scroll: 0,
            live: None,
            prices: prices_path
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p).ok())
                .map(|t| Ledger::parse(&t))
                .unwrap_or_default(),
            unlocked: prices_path
                .as_ref()
                .and_then(|p| std::fs::read_to_string(p.with_file_name("unlocked.tsv")).ok())
                .map(|t| Unlocked::parse(&t))
                .unwrap_or_default(),
            unlocked_path: prices_path.as_ref().map(|p| p.with_file_name("unlocked.tsv")),
            prices_path,
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

    fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.confirm_quit {
            if matches!(code, KeyCode::Char('y' | 'Y')) {
                self.quit = true;
            }
            self.confirm_quit = false;
            return;
        }
        if let Some(view) = &mut self.tree {
            match code {
                KeyCode::Esc | KeyCode::Char('t' | 'q') => self.tree = None,
                KeyCode::Down | KeyCode::Char('j') => view.scroll = view.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => view.scroll = view.scroll.saturating_sub(1),
                KeyCode::PageDown => view.scroll = view.scroll.saturating_add(10),
                KeyCode::PageUp => view.scroll = view.scroll.saturating_sub(10),
                KeyCode::Home | KeyCode::Char('g') => view.scroll = 0,
                KeyCode::End | KeyCode::Char('G') => view.scroll = u16::MAX, // the drawing code clamps it
                _ => {}
            }
            return;
        }
        if self.skill_picker.is_some() {
            self.picker_key(code);
            return;
        }
        if self.name_prompt.is_some() {
            self.name_key(code);
            return;
        }
        if self.piece_picker.is_some() {
            self.piece_key(code);
            return;
        }
        if self.commanding {
            match code {
                KeyCode::Esc => self.commanding = false,
                KeyCode::Enter => {
                    self.commanding = false;
                    let text = std::mem::take(&mut self.command);
                    self.run_command(&text);
                }
                KeyCode::Backspace => {
                    self.command.pop();
                }
                KeyCode::Char(c) => self.command.push(c),
                _ => {}
            }
            return;
        }
        if self.searching {
            let text = if self.tab == Tab::Items {
                &mut self.item_search
            } else {
                &mut self.search
            };
            match code {
                KeyCode::Esc => {
                    self.searching = false;
                    text.clear();
                }
                KeyCode::Enter => self.searching = false,
                KeyCode::Backspace => {
                    text.pop();
                }
                KeyCode::Char(c) => text.push(c),
                _ => {}
            }
            self.apply_search();
            return;
        }
        if self.show_help {
            match code {
                KeyCode::Down | KeyCode::Char('j') => self.help_scroll = self.help_scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => self.help_scroll = self.help_scroll.saturating_sub(1),
                KeyCode::PageDown => self.help_scroll = self.help_scroll.saturating_add(10),
                KeyCode::PageUp => self.help_scroll = self.help_scroll.saturating_sub(10),
                _ => {
                    self.show_help = false; // any other key closes the help overlay
                    self.help_scroll = 0;
                }
            }
            return;
        }
        if self.tab == Tab::Builds && self.builds_key(code) {
            return;
        }
        match code {
            KeyCode::Char('q') => {
                // Closing the TUI ends live updates from a Cemu that is still running, so ask first.
                let cemu_running = self.live.as_mut().is_some_and(|l| l.child.try_wait().ok().flatten().is_none());
                if cemu_running {
                    self.confirm_quit = true;
                } else {
                    self.quit = true;
                }
            }
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('p') if self.tab == Tab::Items => self.pouch_focus = !self.pouch_focus,
            KeyCode::Char('t') if self.tab != Tab::Items => self.open_tree(),
            KeyCode::Char('i') if self.tab != Tab::Items => self.skill_info = !self.skill_info,
            KeyCode::Char(':') if self.edit_mode => {
                self.commanding = true;
                self.command.clear();
            }
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => self.switch_tab(1),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => self.switch_tab(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            // on the Monsters tab the page keys scroll the drops, since the list is moved with the arrows and Home/End
            KeyCode::PageDown if self.tab == Tab::Monsters => self.monster_scroll = self.monster_scroll.saturating_add(10),
            KeyCode::PageUp if self.tab == Tab::Monsters => self.monster_scroll = self.monster_scroll.saturating_sub(10),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::Home | KeyCode::Char('g') => self.move_selection(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.move_selection(isize::MAX),
            KeyCode::Char('/') if matches!(self.tab, Tab::Items | Tab::Crafting) => self.searching = true,
            KeyCode::Char('c') if self.tab == Tab::Crafting => {
                self.craftable_only = !self.craftable_only;
                self.refresh_pieces();
            }
            KeyCode::Char('o') if self.tab == Tab::Crafting => {
                self.hide_owned = !self.hide_owned;
                self.refresh_pieces();
            }
            KeyCode::Char('b') if self.tab == Tab::Crafting => {
                self.blacksmith_only = !self.blacksmith_only;
                self.refresh_pieces();
            }
            KeyCode::Char('u') if self.tab == Tab::Crafting => {
                self.unpriced_only = !self.unpriced_only;
                self.refresh_pieces();
            }
            KeyCode::Char('s') => match self.tab {
                Tab::Items => {
                    self.box_sort = self.box_sort.next();
                    self.refresh_box();
                }
                Tab::Crafting => {
                    self.piece_sort = self.piece_sort.next();
                    self.refresh_pieces();
                }
                Tab::Equipment => {
                    self.equip_sort = self.equip_sort.next();
                    self.refresh_equipment();
                }
                Tab::Monsters => self.monster_sort = self.monster_sort.next(),
                _ => {}
            },
            KeyCode::Char('w') if self.tab == Tab::Crafting => {
                if let Some(p) = self.craft_state.selected().and_then(|i| self.pieces.get(i)) {
                    let (kind, id) = (p.kind, p.id);
                    self.toggle_wish(kind, id);
                }
            }
            // On the searchable tabs, x clears the search; with nothing searched it does nothing.
            KeyCode::Char('x') if matches!(self.tab, Tab::Items | Tab::Crafting) => {
                if !self.active_search().is_empty() {
                    if self.tab == Tab::Items {
                        self.item_search.clear();
                    } else {
                        self.search.clear();
                    }
                    self.apply_search();
                }
            }
            KeyCode::Char('w' | 'x') | KeyCode::Delete if self.tab == Tab::Wishlist => {
                if let Some(&(kind, id)) = self.wish_state.selected().and_then(|i| self.wishlist.get(i)) {
                    self.toggle_wish(kind, id);
                }
            }
            KeyCode::Esc => {
                if self.tab == Tab::Items {
                    self.item_search.clear();
                } else {
                    self.search.clear();
                }
                self.apply_search();
            }
            _ => {}
        }
    }

    fn switch_tab(&mut self, step: isize) {
        let i = Tab::ALL.iter().position(|&t| t == self.tab).unwrap_or(0) as isize;
        self.tab = Tab::ALL[(i + step).rem_euclid(Tab::ALL.len() as isize) as usize];
        if self.tab == Tab::Builds && self.build_stale {
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
            Tab::Builds => match self.build_focus {
                BuildFocus::Sets => (&mut self.build_result_state, self.build_results.len()),
                BuildFocus::Skills => (&mut self.build_target_state, self.build.targets.len()),
                BuildFocus::Templates => (&mut self.template_state, self.templates.len()),
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

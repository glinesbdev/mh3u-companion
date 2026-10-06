//! Keyboard handling. Popups and prompts get the key first; then the tab's own keys; then the keys every tab shares.

use super::*;

impl App {
    /// A key press from whatever screen is showing the app.
    pub fn on_key(&mut self, code: Key, mods: Mods) {
        if mods.ctrl && code == Key::Char('c') {
            self.quit = true;
            return;
        }
        if self.modal_key(code) || (self.tab == Tab::Builds && self.builds_key(code)) || self.tab_key(code) {
            return;
        }
        self.global_key(code);
    }

    /// Whichever popup or prompt is open takes every key. Returns whether one was open.
    fn modal_key(&mut self, code: Key) -> bool {
        if self.confirm_quit {
            if matches!(code, Key::Char('y' | 'Y')) {
                self.quit = true;
            }
            self.confirm_quit = false;
        } else if self.tree.is_some() {
            self.tree_key(code);
        } else if self.settings.is_some() {
            self.settings_key(code);
        } else if self.hunter_choice.is_some() {
            self.hunter_choice_key(code);
        } else if self.quests.choosing.is_some() {
            self.monster_choice_key(code);
        } else if self.builds.skill_picker.is_some() {
            self.picker_key(code);
        } else if self.builds.name_prompt.is_some() {
            self.name_key(code);
        } else if self.builds.piece_picker.is_some() {
            self.piece_key(code);
        } else if self.give_picker_open() {
            self.give_key(code);
        } else if self.is_commanding() {
            self.command_key(code);
        } else if self.searching {
            self.search_key(code);
        } else if self.show_help {
            self.help_key(code);
        } else {
            return false;
        }
        true
    }

    fn tree_key(&mut self, code: Key) {
        let Some(view) = &mut self.tree else { return };
        match code {
            Key::Esc | Key::Char('t' | 'q') => self.tree = None,
            Key::Down | Key::Char('j') => view.scroll = view.scroll.saturating_add(1),
            Key::Up | Key::Char('k') => view.scroll = view.scroll.saturating_sub(1),
            Key::PageDown => view.scroll = view.scroll.saturating_add(10),
            Key::PageUp => view.scroll = view.scroll.saturating_sub(10),
            Key::Home | Key::Char('g') => view.scroll = 0,
            Key::End | Key::Char('G') => view.scroll = u16::MAX, // the drawing code clamps it
            _ => {}
        }
    }

    fn search_key(&mut self, code: Key) {
        match code {
            Key::Esc => {
                self.search_text_mut().clear();
                self.searching = false;
            }
            Key::Enter => self.searching = false,
            Key::Backspace => {
                self.search_text_mut().pop();
            }
            Key::Char(c) => self.search_text_mut().push(c),
            _ => {}
        }
        self.apply_search();
    }

    fn help_key(&mut self, code: Key) {
        match code {
            Key::Down | Key::Char('j') => self.help_scroll = self.help_scroll.saturating_add(1),
            Key::Up | Key::Char('k') => self.help_scroll = self.help_scroll.saturating_sub(1),
            Key::PageDown => self.help_scroll = self.help_scroll.saturating_add(10),
            Key::PageUp => self.help_scroll = self.help_scroll.saturating_sub(10),
            _ => {
                self.show_help = false; // any other key closes the help overlay
                self.help_scroll = 0;
            }
        }
    }

    /// The keys of one tab. Returns whether the key was used.
    fn tab_key(&mut self, code: Key) -> bool {
        match self.tab {
            Tab::Items => self.items_key(code),
            Tab::Crafting => self.crafting_key(code),
            Tab::Wishlist => self.wishlist_key(code),
            Tab::Equipment => self.equipment_key(code),
            Tab::Monsters => self.monsters_key(code),
            Tab::Hunts => self.hunts_key(code),
            Tab::Families => self.families_key(code),
            Tab::Quests => self.quests_key(code),
            Tab::Skills => self.skills_tab_key(code),
            Tab::Compare => self.compare_key(code),
            Tab::Worn | Tab::Builds | Tab::Gains => false,
        }
    }

    fn items_key(&mut self, code: Key) -> bool {
        match code {
            Key::Char('p') => self.inv.pouch_focus = !self.inv.pouch_focus,
            Key::Char('/') => self.searching = true,
            Key::Char('s') => {
                self.inv.box_sort = self.inv.box_sort.next();
                self.refresh_box();
            }
            Key::Char('u') => {
                self.inv.spare_only = !self.inv.spare_only;
                self.refresh_box();
            }
            Key::Char('x') => self.clear_search(),
            _ => return false,
        }
        true
    }

    fn crafting_key(&mut self, code: Key) -> bool {
        match code {
            Key::Char('/') => self.searching = true,
            Key::Char('c') => self.craft.craftable_only = !self.craft.craftable_only,
            Key::Char('o') => self.craft.hide_owned = !self.craft.hide_owned,
            Key::Char('b') => self.craft.blacksmith_only = !self.craft.blacksmith_only,
            Key::Char('u') => self.craft.unpriced_only = !self.craft.unpriced_only,
            Key::Char('z') => self.craft.affordable_only = !self.craft.affordable_only,
            Key::Char('s') => self.craft.sort = self.craft.sort.next(),
            Key::Char('w') => {
                if let Some(p) = self.craft.state.selected().and_then(|i| self.craft.pieces.get(i)) {
                    let (kind, id) = (p.kind, p.id);
                    self.toggle_wish(kind, id);
                }
                return true; // the wishlist change refreshes the list itself
            }
            Key::Char('x') => {
                self.clear_search();
                return true;
            }
            _ => return false,
        }
        self.refresh_pieces();
        true
    }

    fn wishlist_key(&mut self, code: Key) -> bool {
        match code {
            Key::Char('e') => {
                self.export_shopping_list();
                true
            }
            Key::Char('w' | 'x') | Key::Delete => {
                if let Some((kind, id)) = self.wish.selected() {
                    self.toggle_wish(kind, id);
                }
                true
            }
            Key::Char('d') => {
                self.toggle_done();
                true
            }
            Key::Char('s') => {
                self.cycle_wish_sort();
                true
            }
            _ => false,
        }
    }

    fn equipment_key(&mut self, code: Key) -> bool {
        match code {
            Key::Char('/') => {
                self.searching = true;
                true
            }
            Key::Char('x') => {
                self.clear_search();
                true
            }
            Key::Char('s') => {
                self.inv.equip_sort = self.inv.equip_sort.next();
                self.refresh_equipment();
                true
            }
            _ => false,
        }
    }

    /// On the Monsters tab the page keys scroll the drops, since the list is moved with the arrows and Home/End.
    fn monsters_key(&mut self, code: Key) -> bool {
        match code {
            Key::Char('/') => self.searching = true,
            Key::Char('x') => self.clear_search(),
            Key::Char('s') => self.monsters.sort = self.monsters.sort.next(),
            Key::PageDown => self.monsters.scroll = self.monsters.scroll.saturating_add(10),
            Key::PageUp => self.monsters.scroll = self.monsters.scroll.saturating_sub(10),
            _ => return false,
        }
        true
    }

    /// The keys every tab shares: quitting, help, switching tabs and moving in the list.
    fn global_key(&mut self, code: Key) {
        match code {
            Key::Char('q') => {
                // Closing the TUI ends live updates from a Cemu that is still running, so ask first.
                let cemu_running = self.live.as_mut().is_some_and(|l| l.child.try_wait().ok().flatten().is_none());
                if cemu_running {
                    self.confirm_quit = true;
                } else {
                    self.quit = true;
                }
            }
            Key::Char('?') => self.show_help = true,
            Key::Char('H') => self.open_hunter_picker(),
            Key::Char('S') => self.open_settings(),
            // put the highlighted weapon in the comparison
            Key::Char('v') if matches!(self.tab, Tab::Crafting | Tab::Equipment | Tab::Wishlist) => self.toggle_compare(),
            Key::Char('t') if self.tab != Tab::Items => self.open_tree(),
            Key::Char('i') if self.tab != Tab::Items => self.skill_info = !self.skill_info,
            #[cfg(feature = "edit")]
            Key::Char('E') if self.console.enabled => self.open_give_picker(),
            #[cfg(feature = "edit")]
            Key::Char(':') if self.console.enabled => {
                self.console.active = true;
                self.console.text.clear();
            }
            Key::Tab | Key::Right | Key::Char('l') => self.switch_tab(1),
            Key::BackTab | Key::Left | Key::Char('h') => self.switch_tab(-1),
            Key::Down | Key::Char('j') => self.move_selection(1),
            Key::Up | Key::Char('k') => self.move_selection(-1),
            Key::PageDown => self.move_selection(10),
            Key::PageUp => self.move_selection(-10),
            Key::Home | Key::Char('g') => self.move_selection(isize::MIN),
            Key::End | Key::Char('G') => self.move_selection(isize::MAX),
            Key::Esc => self.clear_search(),
            _ => {}
        }
    }

    /// Forget the search text of the current tab.
    pub(super) fn clear_search(&mut self) {
        self.search_text_mut().clear();
        self.apply_search();
    }

    /// The search text the current tab types into.
    fn search_text_mut(&mut self) -> &mut String {
        match self.tab {
            Tab::Items => &mut self.inv.item_search,
            Tab::Equipment => &mut self.inv.equip_search,
            Tab::Monsters => &mut self.monsters.search,
            Tab::Families => &mut self.families.search,
            Tab::Quests => &mut self.quests.search,
            Tab::Skills => &mut self.skills.search,
            _ => &mut self.craft.search,
        }
    }
}

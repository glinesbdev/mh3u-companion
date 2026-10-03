//! Keyboard handling. Popups and prompts get the key first; then the tab's own keys; then the keys every tab shares.

use super::*;

impl App {
    pub(super) fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        if self.modal_key(code) || (self.tab == Tab::Builds && self.builds_key(code)) || self.tab_key(code) {
            return;
        }
        self.global_key(code);
    }

    /// Whichever popup or prompt is open takes every key. Returns whether one was open.
    fn modal_key(&mut self, code: KeyCode) -> bool {
        if self.confirm_quit {
            if matches!(code, KeyCode::Char('y' | 'Y')) {
                self.quit = true;
            }
            self.confirm_quit = false;
        } else if self.tree.is_some() {
            self.tree_key(code);
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
        } else if self.console.active {
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

    fn tree_key(&mut self, code: KeyCode) {
        let Some(view) = &mut self.tree else { return };
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
    }

    fn command_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.console.active = false,
            KeyCode::Enter => {
                self.console.active = false;
                let text = std::mem::take(&mut self.console.text);
                self.run_command(&text);
            }
            KeyCode::Backspace => {
                self.console.text.pop();
            }
            KeyCode::Char(c) => self.console.text.push(c),
            _ => {}
        }
    }

    fn search_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.search_text_mut().clear();
                self.searching = false;
            }
            KeyCode::Enter => self.searching = false,
            KeyCode::Backspace => {
                self.search_text_mut().pop();
            }
            KeyCode::Char(c) => self.search_text_mut().push(c),
            _ => {}
        }
        self.apply_search();
    }

    fn help_key(&mut self, code: KeyCode) {
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
    }

    /// The keys of one tab. Returns whether the key was used.
    fn tab_key(&mut self, code: KeyCode) -> bool {
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

    fn items_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('p') => self.inv.pouch_focus = !self.inv.pouch_focus,
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('s') => {
                self.inv.box_sort = self.inv.box_sort.next();
                self.refresh_box();
            }
            KeyCode::Char('u') => {
                self.inv.spare_only = !self.inv.spare_only;
                self.refresh_box();
            }
            KeyCode::Char('x') => self.clear_search(),
            _ => return false,
        }
        true
    }

    fn crafting_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('c') => self.craft.craftable_only = !self.craft.craftable_only,
            KeyCode::Char('o') => self.craft.hide_owned = !self.craft.hide_owned,
            KeyCode::Char('b') => self.craft.blacksmith_only = !self.craft.blacksmith_only,
            KeyCode::Char('u') => self.craft.unpriced_only = !self.craft.unpriced_only,
            KeyCode::Char('z') => self.craft.affordable_only = !self.craft.affordable_only,
            KeyCode::Char('s') => self.craft.sort = self.craft.sort.next(),
            KeyCode::Char('w') => {
                if let Some(p) = self.craft.state.selected().and_then(|i| self.craft.pieces.get(i)) {
                    let (kind, id) = (p.kind, p.id);
                    self.toggle_wish(kind, id);
                }
                return true; // the wishlist change refreshes the list itself
            }
            KeyCode::Char('x') => {
                self.clear_search();
                return true;
            }
            _ => return false,
        }
        self.refresh_pieces();
        true
    }

    fn wishlist_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('w' | 'x') | KeyCode::Delete => {
                if let Some(&(kind, id)) = self.wish.state.selected().and_then(|i| self.wish.items.get(i)) {
                    self.toggle_wish(kind, id);
                }
                true
            }
            _ => false,
        }
    }

    fn equipment_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('s') => {
                self.inv.equip_sort = self.inv.equip_sort.next();
                self.refresh_equipment();
                true
            }
            _ => false,
        }
    }

    /// On the Monsters tab the page keys scroll the drops, since the list is moved with the arrows and Home/End.
    fn monsters_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('s') => self.monsters.sort = self.monsters.sort.next(),
            KeyCode::PageDown => self.monsters.scroll = self.monsters.scroll.saturating_add(10),
            KeyCode::PageUp => self.monsters.scroll = self.monsters.scroll.saturating_sub(10),
            _ => return false,
        }
        true
    }

    /// The keys every tab shares: quitting, help, switching tabs and moving in the list.
    fn global_key(&mut self, code: KeyCode) {
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
            KeyCode::Char('H') => self.open_hunter_picker(),
            // put the highlighted weapon in the comparison
            KeyCode::Char('v') if matches!(self.tab, Tab::Crafting | Tab::Equipment | Tab::Wishlist) => self.toggle_compare(),
            KeyCode::Char('t') if self.tab != Tab::Items => self.open_tree(),
            KeyCode::Char('i') if self.tab != Tab::Items => self.skill_info = !self.skill_info,
            KeyCode::Char(':') if self.console.enabled => {
                self.console.active = true;
                self.console.text.clear();
            }
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => self.switch_tab(1),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => self.switch_tab(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_selection(-1),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::Home | KeyCode::Char('g') => self.move_selection(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.move_selection(isize::MAX),
            KeyCode::Esc => self.clear_search(),
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
            Tab::Families => &mut self.families.search,
            Tab::Quests => &mut self.quests.search,
            Tab::Skills => &mut self.skills.search,
            _ => &mut self.craft.search,
        }
    }
}

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
        } else if self.builds.skill_picker.is_some() {
            self.picker_key(code);
        } else if self.builds.name_prompt.is_some() {
            self.name_key(code);
        } else if self.builds.piece_picker.is_some() {
            self.piece_key(code);
        } else if self.commanding {
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
    }

    fn search_key(&mut self, code: KeyCode) {
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
            Tab::Worn | Tab::Builds => false,
        }
    }

    fn items_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('p') => self.pouch_focus = !self.pouch_focus,
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('s') => {
                self.box_sort = self.box_sort.next();
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
            KeyCode::Char('c') => self.craftable_only = !self.craftable_only,
            KeyCode::Char('o') => self.hide_owned = !self.hide_owned,
            KeyCode::Char('b') => self.blacksmith_only = !self.blacksmith_only,
            KeyCode::Char('u') => self.unpriced_only = !self.unpriced_only,
            KeyCode::Char('s') => self.piece_sort = self.piece_sort.next(),
            KeyCode::Char('w') => {
                if let Some(p) = self.craft_state.selected().and_then(|i| self.pieces.get(i)) {
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
                if let Some(&(kind, id)) = self.wish_state.selected().and_then(|i| self.wishlist.get(i)) {
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
                self.equip_sort = self.equip_sort.next();
                self.refresh_equipment();
                true
            }
            _ => false,
        }
    }

    /// On the Monsters tab the page keys scroll the drops, since the list is moved with the arrows and Home/End.
    fn monsters_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Char('s') => self.monster_sort = self.monster_sort.next(),
            KeyCode::PageDown => self.monster_scroll = self.monster_scroll.saturating_add(10),
            KeyCode::PageUp => self.monster_scroll = self.monster_scroll.saturating_sub(10),
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
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::Home | KeyCode::Char('g') => self.move_selection(isize::MIN),
            KeyCode::End | KeyCode::Char('G') => self.move_selection(isize::MAX),
            KeyCode::Esc => self.clear_search(),
            _ => {}
        }
    }

    /// Forget the search text of the current tab (the Items tab has its own).
    fn clear_search(&mut self) {
        if self.tab == Tab::Items {
            self.item_search.clear();
        } else {
            self.search.clear();
        }
        self.apply_search();
    }
}

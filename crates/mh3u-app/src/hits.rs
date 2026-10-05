//! Where things were drawn, so that a click can be matched to what is under it. A screen fills this in while it draws (the app can't know
//! where a terminal put its tabs and lists) and the app reads it when the pointer is used.

use crate::app::{BuildFocus, Tab};
use crate::input::Key;

/// A rectangle of the screen in cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Area {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl Area {
    pub fn new(x: u16, y: u16, width: u16, height: u16) -> Area {
        Area { x, y, width, height }
    }

    pub fn contains(&self, col: u16, row: u16) -> bool {
        col >= self.x && col < self.x.saturating_add(self.width) && row >= self.y && row < self.y.saturating_add(self.height)
    }
}

/// Which of the lists on a tab has the keys. A click in a list that does not have them hands them over first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    ItemsPouch,
    ItemsBox,
    Builds(BuildFocus),
}

/// A list pane as drawn, with a one-cell border all round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListHit {
    pub area: Area,
    /// The first row on screen.
    pub offset: usize,
    pub len: usize,
    /// The highlighted row, when this list has the keys.
    pub selected: Option<usize>,
    pub focus: Option<Focus>,
}

impl ListHit {
    /// The row under the cell, if it is one of the list's rows (not the border).
    pub fn row_at(&self, col: u16, row: u16) -> Option<usize> {
        let inside = Area::new(
            self.area.x.saturating_add(1),
            self.area.y.saturating_add(1),
            self.area.width.saturating_sub(2),
            self.area.height.saturating_sub(2),
        );
        if !inside.contains(col, row) {
            return None;
        }
        let at = self.offset + usize::from(row - inside.y);
        (at < self.len).then_some(at)
    }
}

/// Everything clickable in the last frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Hits {
    pub tabs: Vec<(Area, Tab)>,
    pub lists: Vec<ListHit>,
    /// The piece slots of the template shown on the Builds tab (head ... weapon), numbered as `Slot::ALL`.
    pub slots: Vec<(Area, usize)>,
    /// Text that tells what a key does, drawn where a click should do it: the key to press, after giving the list the keys if it is one of
    /// several.
    pub actions: Vec<(Area, Option<Focus>, Key)>,
    /// How many of `lists` belong to the tab itself: the ones after them are in popups, drawn on top.
    pub tab_lists: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_found_inside_the_border_and_after_the_scroll() {
        let hit = ListHit {
            area: Area::new(10, 5, 20, 6), // four rows
            offset: 3,
            len: 6,
            selected: None,
            focus: None,
        };
        assert_eq!(hit.row_at(12, 5), None, "top border");
        assert_eq!(hit.row_at(12, 6), Some(3));
        assert_eq!(hit.row_at(12, 8), Some(5));
        assert_eq!(hit.row_at(12, 9), None, "past the last row");
        assert_eq!(hit.row_at(10, 6), None, "left border");
        assert_eq!(hit.row_at(29, 6), None, "right border");
    }
}

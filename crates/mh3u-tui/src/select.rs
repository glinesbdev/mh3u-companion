//! The highlighted row of a list and how far it has scrolled, kept by the app so that any screen (a terminal, a window) can show it.

/// Which row of a list is highlighted, and the first row on screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListState {
    selected: Option<usize>,
    offset: usize,
}

impl ListState {
    pub fn with_selected(mut self, selected: Option<usize>) -> ListState {
        self.select(selected);
        self
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Highlight a row, or none (which also scrolls back to the top).
    pub fn select(&mut self, selected: Option<usize>) {
        self.selected = selected;
        if selected.is_none() {
            self.offset = 0;
        }
    }

    /// The first row on screen. A screen that scrolls the list itself stores where it got to with `set_offset`.
    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_nothing_scrolls_back_to_the_top() {
        let mut s = ListState::default().with_selected(Some(7));
        s.set_offset(5);
        assert_eq!((s.selected(), s.offset()), (Some(7), 5));
        s.select(None);
        assert_eq!((s.selected(), s.offset()), (None, 0));
    }
}

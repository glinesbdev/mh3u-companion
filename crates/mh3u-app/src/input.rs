//! Keys as the app understands them, whatever the screen that sends them.

/// A key press. The terminal and any other screen translate their own key events into these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Esc,
    Backspace,
    Delete,
    Tab,
    BackTab,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
}

/// Modifier keys held with a key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mods {
    pub ctrl: bool,
}

/// Something done with the mouse, at a cell of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pointer {
    /// The left button went down.
    Click { col: u16, row: u16 },
    /// The wheel turned one notch.
    Scroll { col: u16, row: u16, down: bool },
}

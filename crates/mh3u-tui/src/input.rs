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

/// The key the terminal sent, if the app has a use for it.
pub fn from_terminal(code: ratatui::crossterm::event::KeyCode) -> Option<Key> {
    use ratatui::crossterm::event::KeyCode as K;
    Some(match code {
        K::Char(c) => Key::Char(c),
        K::Enter => Key::Enter,
        K::Esc => Key::Esc,
        K::Backspace => Key::Backspace,
        K::Delete => Key::Delete,
        K::Tab => Key::Tab,
        K::BackTab => Key::BackTab,
        K::Up => Key::Up,
        K::Down => Key::Down,
        K::Left => Key::Left,
        K::Right => Key::Right,
        K::PageUp => Key::PageUp,
        K::PageDown => Key::PageDown,
        K::Home => Key::Home,
        K::End => Key::End,
        _ => return None,
    })
}

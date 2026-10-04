//! The terminal's keys as the app understands them.

use mh3u_app::input::Key;

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

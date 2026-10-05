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

/// What the mouse did, if the app has a use for it (the left button going down, the wheel).
pub fn pointer_from_terminal(mouse: ratatui::crossterm::event::MouseEvent) -> Option<mh3u_app::input::Pointer> {
    use mh3u_app::input::Pointer;
    use ratatui::crossterm::event::{MouseButton, MouseEventKind as M};
    let (col, row) = (mouse.column, mouse.row);
    match mouse.kind {
        M::Down(MouseButton::Left) => Some(Pointer::Click { col, row }),
        M::ScrollDown => Some(Pointer::Scroll { col, row, down: true }),
        M::ScrollUp => Some(Pointer::Scroll { col, row, down: false }),
        _ => None,
    }
}

//! Colors and small shared widgets, so the whole screen speaks one visual language: green means you have it, yellow means
//! partly, red means missing, cyan marks focus and keys, gray is secondary text. Named terminal colors are used on purpose so
//! the user's own terminal palette applies. With `NO_COLOR` set only bold, dim and the symbols remain.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType},
};
use std::sync::OnceLock;

fn colors_on() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty()))
}

fn fg(color: Color) -> Style {
    if colors_on() { Style::new().fg(color) } else { Style::new() }
}

/// Focus, selection and key names.
pub fn accent() -> Style {
    // the bright variant: the plain cyan slot is dim in many themes
    fg(Color::LightCyan)
}

/// Something you have, can afford or can do.
pub fn good() -> Style {
    fg(Color::Green)
}

/// Partly there.
pub fn warn() -> Style {
    fg(Color::Yellow)
}

/// Something missing or out of reach.
pub fn bad() -> Style {
    fg(Color::Red)
}

/// Secondary text: counts, separators, key descriptions. A fixed mid gray: the terminal's own "dark gray" is nearly
/// invisible in many themes and its "gray" is as bright as normal text, so neither is used for text.
pub fn muted() -> Style {
    if colors_on() {
        Style::new().fg(Color::Indexed(245))
    } else {
        Style::new().add_modifier(Modifier::DIM)
    }
}

/// Decoration that should recede: inactive borders and scroll bars. Darker than `muted`, so never used for text.
pub fn faint() -> Style {
    if colors_on() {
        Style::new().fg(Color::DarkGray)
    } else {
        Style::new().add_modifier(Modifier::DIM)
    }
}

pub fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

/// The selected row: no background, so it reads on dark and light terminals alike.
pub fn selection() -> Style {
    accent().add_modifier(Modifier::BOLD)
}

pub const SELECTION_MARK: &str = "▶ ";

/// A rounded pane. The pane that has focus gets the accent border and a bold title.
pub fn pane<'a>(title: impl Into<String>, active: bool) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(if active { accent() } else { faint() })
        .title(Line::styled(title.into(), if active { bold() } else { Style::new() }))
}

/// Side by side when the terminal is wide enough, stacked when it is not.
pub fn split(area: Rect, first_percent: u16) -> [Rect; 2] {
    let parts = [Constraint::Percentage(first_percent), Constraint::Percentage(100 - first_percent)];
    if area.width >= 100 {
        Layout::horizontal(parts).areas(area)
    } else {
        Layout::vertical(parts).areas(area)
    }
}

/// `have` of `need` as a small bar: `███░░░`. Anything above zero shows at least one cell, and only a full count fills it.
pub fn bar(have: u32, need: u32, width: usize) -> String {
    let filled = if need == 0 || have >= need {
        width
    } else {
        ((have as usize * width) / need as usize).max(usize::from(have > 0))
    };
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// How much of a requirement you have: green when complete, yellow when partly, red when none.
pub fn progress_style(have: u32, need: u32) -> Style {
    if have >= need {
        good()
    } else if have > 0 {
        warn()
    } else {
        bad()
    }
}

/// One material row: name, bar and `have/need`, e.g. `  Iron Ore                 ███░░░  3/5`.
pub fn material_line(name: &str, have: u32, need: u32) -> Line<'static> {
    let style = progress_style(have, need);
    Line::from(vec![
        Span::raw(format!("  {name:<24}")),
        Span::styled(bar(have, need, 6), style),
        Span::styled(format!(" {have:>3}/{need:<3}"), style),
    ])
}

/// Gem slots as filled and empty diamonds: `◆◆◇`.
pub fn gems(slots: u8) -> String {
    let filled = usize::from(slots.min(3));
    format!("{}{}", "◆".repeat(filled), "◇".repeat(3 - filled))
}

pub fn rarity_style(rarity: u8) -> Style {
    let color = match rarity {
        0..=2 => Color::Gray,
        3 => Color::Green,
        4 => Color::LightCyan,
        5 => Color::Blue,
        6 => Color::Magenta,
        7 => Color::Yellow,
        8 => Color::LightRed,
        9 => Color::LightMagenta,
        _ => Color::LightYellow,
    };
    fg(color).add_modifier(Modifier::BOLD)
}

/// A short badge such as `R5 `, padded to three cells so columns line up.
pub fn rarity_badge(rarity: u8) -> Span<'static> {
    Span::styled(format!("R{rarity:<2}"), rarity_style(rarity))
}

pub fn element_style(name: &str) -> Style {
    let color = match name {
        "Fire" => Color::Red,
        "Water" => Color::Blue,
        "Ice" => Color::LightCyan,
        "Thunder" => Color::Yellow,
        _ => Color::Magenta,
    };
    fg(color)
}

/// Green above zero, red below, gray at zero.
pub fn signed_style(points: i32) -> Style {
    match points {
        1.. => good(),
        0 => muted(),
        _ => bad(),
    }
}

/// Key names in the accent color followed by what they do: `s sort   w wish`.
pub fn key_hints(pairs: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for (i, (key, what)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push(Span::raw("  "));
        }
        out.push(Span::styled(key.to_string(), accent().add_modifier(Modifier::BOLD)));
        out.push(Span::styled(format!(" {what}"), muted()));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_fill_in_proportion_and_only_a_full_count_fills_them() {
        assert_eq!(bar(0, 5, 6), "░░░░░░");
        assert_eq!(bar(1, 100, 6), "█░░░░░", "a little shows as at least one cell");
        assert_eq!(bar(3, 6, 6), "███░░░");
        assert_eq!(bar(5, 6, 6), "█████░", "never full until complete");
        assert_eq!(bar(6, 6, 6), "██████");
        assert_eq!(bar(9, 6, 6), "██████");
        assert_eq!(bar(0, 0, 6), "██████", "nothing needed counts as complete");
    }

    #[test]
    fn gem_slots_show_filled_and_empty() {
        assert_eq!(gems(0), "◇◇◇");
        assert_eq!(gems(2), "◆◆◇");
        assert_eq!(gems(3), "◆◆◆");
        assert_eq!(gems(9), "◆◆◆", "out-of-range slot counts stay on the three-gem display");
    }

    #[test]
    fn a_row_of_equal_name_widths_lines_up() {
        let a = material_line("Iron Ore", 3, 5).to_string();
        let b = material_line("Dragonite Ore", 12, 5).to_string();
        assert_eq!(a.chars().count(), b.chars().count());
        assert!(a.contains("███░░░") && a.ends_with("3/5  "), "{a:?}");
    }

    #[test]
    fn narrow_terminals_stack_the_panes() {
        let wide = split(Rect::new(0, 0, 120, 40), 50);
        assert_eq!(wide[0].y, wide[1].y);
        let narrow = split(Rect::new(0, 0, 80, 40), 50);
        assert_eq!(narrow[0].x, narrow[1].x);
        assert!(narrow[1].y > narrow[0].y);
    }

    #[test]
    fn hints_pair_each_key_with_its_action() {
        let line = Line::from(key_hints(&[("s", "sort"), ("w", "wish")])).to_string();
        assert_eq!(line, "s sort  w wish");
    }
}

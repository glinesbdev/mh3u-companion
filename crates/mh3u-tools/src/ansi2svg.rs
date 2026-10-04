//! Turn a terminal capture with colors (`tmux capture-pane -p -e`) into an SVG picture of the terminal, for the README.
//!
//! Only what a TUI draws is handled: 16, 256 and true colors for the text and for a cell's background, bold, dim and underline.
//! The 16 named colors use the Tokyo Night palette.

const CELL_W: f64 = 9.6;
const CELL_H: f64 = 20.0;
const PAD: f64 = 16.0;
const FONT_SIZE: f64 = 16.0;
const BACKGROUND: u32 = 0x1a1b26;
const FOREGROUND: u32 = 0xc0caf5;

const NAMED: [u32; 16] = [
    0x1a1b26, 0xf7768e, 0x9ece6a, 0xe0af68, 0x7aa2f7, 0xad8ee6, 0x449dab, 0xa9b1d6, 0x414868, 0xff7a93, 0xb9f27c, 0xff9e64, 0x7da6ff,
    0xbb9af7, 0x0db9d7, 0xc0caf5,
];

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
struct Style {
    fg: Option<u32>,
    bg: Option<u32>,
    bold: bool,
    dim: bool,
    underline: bool,
}

fn indexed(n: u32) -> u32 {
    match n {
        0..=15 => NAMED[n as usize],
        16..=231 => {
            let n = n - 16;
            let level = |v: u32| if v == 0 { 0 } else { 55 + 40 * v };
            (level(n / 36) << 16) | (level(n / 6 % 6) << 8) | level(n % 6)
        }
        _ => {
            let v = 8 + 10 * (n.min(255) - 232);
            (v << 16) | (v << 8) | v
        }
    }
}

/// Apply one SGR escape (the numbers between `ESC [` and `m`) to a style.
fn apply(style: &mut Style, params: &[u32]) {
    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0 => *style = Style::default(),
            1 => style.bold = true,
            2 => style.dim = true,
            4 => style.underline = true,
            22 => (style.bold, style.dim) = (false, false),
            24 => style.underline = false,
            39 => style.fg = None,
            n @ 30..=37 => style.fg = Some(NAMED[(n - 30) as usize]),
            n @ 90..=97 => style.fg = Some(NAMED[(n - 90 + 8) as usize]),
            38 if params.get(i + 1) == Some(&5) => {
                style.fg = params.get(i + 2).map(|&n| indexed(n));
                i += 2;
            }
            38 if params.get(i + 1) == Some(&2) => {
                if let [r, g, b] = params[i + 2..].iter().take(3).copied().collect::<Vec<_>>()[..] {
                    style.fg = Some((r << 16) | (g << 8) | b);
                }
                i += 4;
            }
            49 => style.bg = None,
            n @ 40..=47 => style.bg = Some(NAMED[(n - 40) as usize]),
            n @ 100..=107 => style.bg = Some(NAMED[(n - 100 + 8) as usize]),
            48 if params.get(i + 1) == Some(&5) => {
                style.bg = params.get(i + 2).map(|&n| indexed(n));
                i += 2;
            }
            48 if params.get(i + 1) == Some(&2) => {
                if let [r, g, b] = params[i + 2..].iter().take(3).copied().collect::<Vec<_>>()[..] {
                    style.bg = Some((r << 16) | (g << 8) | b);
                }
                i += 4;
            }
            _ => {}
        }
        i += 1;
    }
}

/// Cells a character takes on screen.
fn width(c: char) -> usize {
    match c as u32 {
        0x1100..=0x115f | 0x2e80..=0xa4cf | 0xac00..=0xd7a3 | 0xf900..=0xfaff | 0xfe30..=0xfe6f | 0xff00..=0xff60 | 0x1f300..=0x1faff => 2,
        _ => 1,
    }
}

fn parse_line(line: &str) -> Vec<(char, Style)> {
    let mut out = Vec::new();
    let mut style = Style::default();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            let mut params = String::new();
            let mut last = ' ';
            for n in chars.by_ref() {
                if n.is_ascii_digit() || n == ';' {
                    params.push(n);
                } else {
                    last = n;
                    break;
                }
            }
            if last == 'm' {
                let numbers: Vec<u32> = if params.is_empty() {
                    vec![0]
                } else {
                    params.split(';').map(|p| p.parse().unwrap_or(0)).collect()
                };
                apply(&mut style, &numbers);
            }
        } else if !c.is_control() {
            out.push((c, style));
        }
    }
    out
}

/// Escape for XML, and ask viewers to draw symbols that also exist as emoji (check marks, stars, arrows) as plain text symbols
/// by following them with the text-presentation selector, so they keep to one cell like they do in a terminal.
fn escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => {
                out.push(c);
                if matches!(c, '\u{2600}'..='\u{27bf}' | '\u{25b6}' | '\u{25c0}') {
                    out.push('\u{fe0e}');
                }
            }
        }
    }
    out
}

/// Draw `capture` (lines of text with SGR escapes) as an SVG. `replacements` are (from, to) pairs applied to the text first,
/// used to keep names out of the picture.
pub fn render(capture: &str, replacements: &[(String, String)]) -> String {
    let mut text = capture.to_string();
    for (from, to) in replacements {
        text = text.replace(from, to);
    }
    let lines: Vec<Vec<(char, Style)>> = text.lines().map(parse_line).collect();
    let cols = lines
        .iter()
        .map(|l| l.iter().map(|&(c, _)| width(c)).sum::<usize>())
        .max()
        .unwrap_or(0);
    let (w, h) = (cols as f64 * CELL_W + 2.0 * PAD, lines.len() as f64 * CELL_H + 2.0 * PAD);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" role=\"img\">\n\
         <rect width=\"100%\" height=\"100%\" rx=\"10\" fill=\"#{BACKGROUND:06x}\"/>\n\
         <g font-family=\"'JetBrains Mono','JetBrainsMono Nerd Font','DejaVu Sans Mono',Menlo,Consolas,monospace\" font-size=\"{FONT_SIZE}\">\n"
    );
    for (row, line) in lines.iter().enumerate() {
        let y = PAD + row as f64 * CELL_H + CELL_H * 0.72;
        let (mut col, mut i) = (0usize, 0usize);
        while i < line.len() {
            let style = line[i].1;
            let (start, mut run, mut cells) = (col, String::new(), 0usize);
            // a run is consecutive narrow characters of one style; a wide character is drawn on its own
            while i < line.len() && line[i].1 == style {
                let c = line[i].0;
                if width(c) == 2 {
                    if run.is_empty() {
                        run.push(c);
                        cells = 2;
                        i += 1;
                    }
                    break;
                }
                run.push(c);
                cells += 1;
                i += 1;
            }
            col = start + cells;
            if let Some(bg) = style.bg {
                svg.push_str(&format!(
                    "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{CELL_H}\" fill=\"#{bg:06x}\"/>\n",
                    PAD + start as f64 * CELL_W,
                    PAD + row as f64 * CELL_H,
                    cells as f64 * CELL_W
                ));
            }
            if run.trim().is_empty() && !style.underline {
                continue;
            }
            let fill = style.fg.unwrap_or(FOREGROUND);
            // Wide characters are emoji: name an emoji font so the picture does not depend on the viewer's fallback.
            let emoji = if cells == 2 && run.chars().next().is_some_and(|c| width(c) == 2) {
                " font-family=\"'Noto Color Emoji','Apple Color Emoji','Segoe UI Emoji',sans-serif\""
            } else {
                ""
            };
            let attrs = format!(
                "{emoji}{}{}",
                if style.bold { " font-weight=\"bold\"" } else { "" },
                if style.dim { " fill-opacity=\"0.6\"" } else { "" }
            );
            // Each word is placed at its own exact position and width, so the picture does not depend on how the viewer treats
            // runs of spaces or on the width of its monospace font.
            let (mut offset, mut word, mut word_start, mut word_cells) = (0usize, String::new(), 0usize, 0usize);
            let flush = |svg: &mut String, word: &mut String, word_start: usize, word_cells: usize| {
                if !word.is_empty() {
                    svg.push_str(&format!(
                        "<text x=\"{:.1}\" y=\"{y:.1}\" textLength=\"{:.1}\" lengthAdjust=\"spacing\" fill=\"#{fill:06x}\"{attrs}>{}</text>\n",
                        PAD + (start + word_start) as f64 * CELL_W,
                        word_cells as f64 * CELL_W,
                        escape(word)
                    ));
                    word.clear();
                }
            };
            for ch in run.chars() {
                if ch == ' ' {
                    flush(&mut svg, &mut word, word_start, word_cells);
                    word_cells = 0;
                    offset += 1;
                } else {
                    if word.is_empty() {
                        word_start = offset;
                    }
                    word.push(ch);
                    word_cells += width(ch);
                    offset += width(ch);
                }
            }
            flush(&mut svg, &mut word, word_start, word_cells);
            if style.underline {
                let underline_y = y + 3.0;
                svg.push_str(&format!(
                    "<line x1=\"{:.1}\" y1=\"{underline_y:.1}\" x2=\"{:.1}\" y2=\"{underline_y:.1}\" stroke=\"#{fill:06x}\" stroke-width=\"1\"/>\n",
                    PAD + start as f64 * CELL_W,
                    PAD + (start + cells) as f64 * CELL_W
                ));
            }
        }
    }
    svg.push_str("</g>\n</svg>\n");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_background_is_drawn_behind_its_cells() {
        let svg = render("\x1b[48;5;196m\x1b[30m ♪ \x1b[0m", &[]);
        assert!(svg.contains("<rect x=\"16.0\" y=\"16.0\" width=\"28.8\""), "{svg}");
        assert!(svg.contains("fill=\"#ff0000\""));
        assert!(!render("plain", &[]).contains("<rect x="));
    }

    #[test]
    fn colors_come_from_named_indexed_and_true_color_escapes() {
        let mut s = Style::default();
        apply(&mut s, &[31]);
        assert_eq!(s.fg, Some(0xf7768e));
        apply(&mut s, &[38, 5, 245]);
        assert_eq!(s.fg, Some(0x8a8a8a));
        apply(&mut s, &[38, 5, 14]);
        assert_eq!(s.fg, Some(0x0db9d7));
        apply(&mut s, &[38, 2, 1, 2, 3]);
        assert_eq!(s.fg, Some(0x010203));
        apply(&mut s, &[1, 4]);
        assert!(s.bold && s.underline);
        apply(&mut s, &[0]);
        assert_eq!(s, Style::default());
    }

    #[test]
    fn words_are_placed_at_their_exact_columns_and_widths() {
        let svg = render("\x1b[32mabc\x1b[0m def", &[]);
        assert!(svg.contains("fill=\"#9ece6a\""), "{svg}");
        assert!(svg.contains(">abc</text>") && svg.contains(">def</text>"));
        assert!(
            svg.contains(&format!("x=\"{:.1}\"", PAD + 4.0 * CELL_W)),
            "def starts in column 4: {svg}"
        );
        assert!(svg.contains(&format!("textLength=\"{:.1}\"", 3.0 * CELL_W)));
    }

    #[test]
    fn replacements_and_escaping_apply_to_the_text() {
        let svg = render("Bob <b> & co", &[("Bob".to_string(), "Hunter".to_string())]);
        assert!(
            svg.contains(">Hunter</text>") && svg.contains(">&lt;b&gt;</text>") && svg.contains(">&amp;</text>"),
            "{svg}"
        );
    }

    #[test]
    fn symbols_that_are_also_emoji_are_asked_to_draw_as_text() {
        let svg = render("✔ ok ⚒ x", &[]);
        assert!(svg.contains(">✔\u{fe0e}</text>") && svg.contains(">⚒\u{fe0e}</text>"), "{svg}");
        assert!(svg.contains(">ok</text>"), "letters are left alone");
    }

    #[test]
    fn wide_characters_take_two_cells() {
        assert_eq!(width('🛒'), 2);
        assert_eq!(width('◆'), 1);
        let svg = render("a🛒b", &[]);
        assert!(svg.contains(">🛒</text>") && svg.contains(&format!("textLength=\"{:.1}\"", 2.0 * CELL_W)));
    }
}

//! The Settings popup: the settings of `config.txt`, changed with the keyboard.

use super::*;
use mh3u_app::config::{self, Kind, When};

pub(super) fn draw_settings(f: &mut Frame, app: &mut App) {
    let Some(mut screen) = app.settings.take() else { return };
    let area = f.area();
    let rows_needed = config::DEFS.len() as u16;
    let (w, h) = (area.width.min(104), area.height.min(rows_needed + 12));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    f.render_widget(Clear, popup);
    f.render_widget(theme::pane(" Settings · config.txt ", true), popup);
    let inner = popup.inner(Margin::new(2, 1));
    let [list, details] = Layout::vertical([Constraint::Min(4), Constraint::Length(8)]).areas(inner);

    let selected = screen.state.selected().unwrap_or(0);
    let items: Vec<ListItem> = config::DEFS
        .iter()
        .enumerate()
        .map(|(i, def)| {
            let value = app.config.get(def.key);
            let set = app.config.is_set(def.key);
            let mut spans = vec![Span::raw(format!("{:<18}", def.label))];
            if i == selected && screen.editing.is_some() {
                spans.push(Span::styled(
                    format!("{}_", screen.editing.as_deref().unwrap_or("")),
                    accent().add_modifier(Modifier::UNDERLINED),
                ));
            } else {
                if def.kind == Kind::Color {
                    let color = theme::color_of(app.config.color(def.key));
                    spans.push(Span::styled("██ ", Style::new().fg(color)));
                }
                let shown = if value.is_empty() {
                    "(automatic)".to_string()
                } else {
                    value.to_string()
                };
                spans.push(if set {
                    Span::styled(shown, bold())
                } else {
                    Span::styled(shown, muted())
                });
                if !set {
                    spans.push(Span::styled("  default", muted()));
                }
                if def.when == When::NextStart {
                    spans.push(Span::styled("  ↻ next start", muted()));
                }
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    render_list(
        f,
        List::new(items)
            .highlight_style(theme::selection())
            .highlight_symbol(theme::SELECTION_MARK),
        list,
        &mut screen.state,
    );

    let mut lines: Vec<Line> = Vec::new();
    if let Some(def) = config::DEFS.get(selected) {
        lines.push(Line::styled(def.help.to_string(), muted()));
        lines.push(Line::raw(""));
        let how = match def.kind {
            Kind::Choice(choices) => format!("←/→ or Enter: {}", choices.join(" · ")),
            Kind::Color => "←/→ or Enter: step through the 16 colors (a number or #rrggbb can be written in the file)".to_string(),
            Kind::Number(low, high) => format!("←/→ or Enter: {low} to {high}"),
            Kind::Text => "Enter: type it (an empty text is the default again)".to_string(),
        };
        lines.push(Line::styled(how, accent()));
        lines.push(Line::styled("d: default   w: write the commented file   Esc: close", muted()));
    }
    if let Some(path) = app.config_file() {
        lines.push(Line::styled(format!("File: {}", path.display()), muted()));
    }
    if let Some(problem) = app.config.problems.first() {
        lines.push(Line::styled(format!("Problem in the file: {problem}"), warn()));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), details);
    app.settings = Some(screen);
}

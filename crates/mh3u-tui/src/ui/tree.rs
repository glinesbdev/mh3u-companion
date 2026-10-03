//! The upgrade tree popup.

use super::*;

/// The upgrade tree popup: the line down to the weapon, then everything it upgrades into.
pub(super) fn draw_tree(f: &mut Frame, app: &mut App) {
    let Some(view) = app.tree.take() else { return };
    let kind = view.kind;
    let kind_label = app.game.equipment_kind_label(kind).unwrap_or("Weapon");
    let mut lines: Vec<Line> = Vec::new();
    for row in &view.tree.rows {
        let name = app.game.equipment_name(kind, row.id).unwrap_or("?");
        let owned = app.save.owns_equipment(kind, row.id);
        let mut spans = vec![Span::styled(row.prefix.clone(), theme::faint())];
        let marker = if owned { "● " } else { "○ " };
        let name_style = if row.selected {
            theme::selection()
        } else if owned {
            good()
        } else {
            Style::new()
        };
        spans.push(Span::styled(marker, if owned { good() } else { muted() }));
        spans.push(Span::styled(name.to_string(), name_style));
        if row.selected {
            spans.push(Span::styled(" ◀", accent()));
        }
        if let Some(r) = app.game.equipment_rarity(kind, row.id) {
            spans.push(Span::raw(" "));
            spans.push(theme::rarity_badge(r));
        }
        if let Some(w) = app.game.weapon_stats(kind, row.id) {
            // the rarity badge is three cells wide (`R4 `, `R10`), so this space keeps `R10` off the attack
            spans.push(Span::styled(format!(" atk {}", w.attack), muted()));
        }
        if !owned && app.can_make_now(kind, row.id) {
            spans.push(Span::styled("  ✔ can make", good()));
        }
        if row.repeat {
            spans.push(Span::styled("  (shown above)", muted()));
        }
        if row.other_branches > 0 {
            spans.push(Span::styled(format!("  +{} other upgrade(s)", row.other_branches), muted()));
        }
        if !row.also_from.is_empty() {
            let names: Vec<&str> = row
                .also_from
                .iter()
                .map(|&p| app.game.equipment_name(kind, p).unwrap_or("?"))
                .collect();
            spans.push(Span::styled(format!("  also from {}", names.join(", ")), muted()));
        }
        lines.push(Line::from(spans));
    }
    if view.tree.omitted > 0 {
        lines.push(Line::styled(format!("… and {} more", view.tree.omitted), muted()));
    }
    let area = f.area();
    let wanted = lines.len() as u16 + 2;
    let (w, h) = (area.width.min(96), wanted.clamp(5, area.height.saturating_sub(2).max(5)));
    let popup = Rect::new(area.x + (area.width - w) / 2, area.y + (area.height - h) / 2, w, h);
    let visible = usize::from(h.saturating_sub(2));
    let max_scroll = lines.len().saturating_sub(visible) as u16;
    let scroll = view.scroll.min(max_scroll);
    let title = format!(" {kind_label} upgrade tree ");
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(lines)
            .scroll((scroll, 0))
            .block(theme::pane(title, true).title_bottom(Line::from(key_line(&[("↑/↓", "scroll"), ("t", "close")])).right_aligned())),
        popup,
    );
    scrollbar(
        f,
        popup,
        view.tree.rows.len() + usize::from(view.tree.omitted > 0),
        Some(usize::from(scroll)),
    );
    app.tree = Some(TreeView { scroll, ..view });
}

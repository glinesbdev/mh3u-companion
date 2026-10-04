//! What the Crafting, Equipment and Wishlist tabs say about a piece: stats, blacksmith status and cost.

use super::*;

/// A section heading with the forging cost beside it: `Create from scratch · 300 z (seen)`.
/// The cost is green when the hunter can pay it, and red with the shortfall when not.
pub(super) fn cost_heading(title: &str, cost: Option<(u32, Source)>, zenny: u32) -> Line<'static> {
    let mut spans = vec![Span::styled(title.to_string(), bold())];
    match cost {
        Some((c, source)) => {
            let from = match source {
                Source::Seen => "seen",
                Source::Notes => "from your notes",
                Source::Learned => "recipe and cost learned from your game",
                Source::Game => "game data",
            };
            spans.push(Span::raw(" · "));
            spans.extend(cost_spans(c, zenny));
            spans.push(Span::styled(format!(" ({from})"), muted()));
        }
        None => spans.push(Span::styled(" · cost not seen yet", muted())),
    }
    Line::from(spans)
}

/// `1,150 z` in green if affordable, or `1,150 z` in red followed by `(short 400 z)`.
pub(super) fn cost_spans(cost: u32, zenny: u32) -> Vec<Span<'static>> {
    let amount = format!("{} z", group_digits(u64::from(cost)));
    if cost <= zenny {
        vec![Span::styled(amount, good())]
    } else {
        vec![
            Span::styled(amount, bad()),
            Span::styled(format!(" (short {} z)", group_digits(u64::from(cost - zenny))), bad()),
        ]
    }
}

/// An armor piece's stats, one fact per line with the colors the rest of the screen uses.
pub(super) fn armor_lines(app: &App, a: &mh3u_core::armor::ArmorStats) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        theme::rarity_badge(a.rarity),
        Span::raw(" "),
        Span::styled(theme::gems(a.slots), accent()),
        Span::styled(format!("  Defense {}", a.defense), bold()),
        Span::styled(" (base)", muted()),
        Span::styled(a.max_defense.map_or(String::new(), |m| format!("  {m} fully upgraded")), muted()),
    ])];
    let class = match a.class {
        Some(mh3u_core::armor::ArmorClass::Both) => "Blademaster & Gunner",
        Some(c) => c.label(),
        None => "?",
    };
    let gender = match a.gender {
        Some(mh3u_core::armor::Gender::Both) => "Male & Female",
        Some(g) => g.label(),
        None => "?",
    };
    lines.push(Line::from(vec![
        Span::styled("For ", muted()),
        Span::raw(class),
        Span::styled(" · ", muted()),
        Span::raw(gender),
    ]));
    let [fire, water, thunder, ice, dragon] = a.resist;
    let mut resist = Vec::new();
    for (name, value) in [
        ("Fire", fire),
        ("Water", water),
        ("Ice", ice),
        ("Thunder", thunder),
        ("Dragon", dragon),
    ] {
        if !resist.is_empty() {
            resist.push(Span::raw("  "));
        }
        resist.push(Span::styled(name, theme::element_style(name)));
        resist.push(Span::styled(format!(" {value:+}"), theme::signed_style(i32::from(value))));
    }
    lines.push(Line::from(resist));
    for &(id, pts) in &a.skills {
        lines.push(Line::from(vec![
            Span::raw(format!("  {:<18}", app.game.skill_name(id).unwrap_or("?"))),
            Span::styled(format!("{pts:+}"), theme::signed_style(i32::from(pts))),
        ]));
        if app.skill_info
            && let Some(text) = app.game.skill_description(id)
        {
            lines.push(Line::styled(format!("    {text}"), muted()));
        }
    }
    lines
}

/// A weapon's stats: rarity, gem slots, attack and affinity.
pub(super) fn weapon_lines(app: &App, kind: u8, id: u16, w: &mh3u_core::weapons::Weapon) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        theme::rarity_badge(w.rarity),
        Span::raw(" "),
        Span::styled(theme::gems(w.slots), accent()),
        Span::styled(format!("  Attack {}", w.attack), bold()),
    ])];
    if w.affinity != 0 {
        lines.push(Line::from(vec![
            Span::styled("Affinity ", muted()),
            Span::styled(format!("{:+}%", w.affinity), theme::signed_style(i32::from(w.affinity))),
        ]));
    }
    if let Some(e) = app.game.weapon_extras(kind, id) {
        for s in &e.specials {
            let mut spans = vec![
                Span::styled(format!("{:<10}", if s.hidden { "Hidden" } else { "Element" }), muted()),
                Span::styled(format!("{} {}", s.name, s.value), theme::element_style(&s.name)),
            ];
            if s.hidden {
                spans.push(Span::styled("  (needs Awaken)", muted()));
            }
            lines.push(Line::from(spans));
        }
        if let Some(line) = part_line(&e.part) {
            lines.push(line);
        }
        lines.push(Line::raw(""));
        lines.push(sharpness_line("Sharpness", &e.sharpness));
        if e.plus != e.sharpness {
            // a gap, so the two bars do not read as one
            lines.push(Line::raw(""));
            lines.push(sharpness_line("With +1", &e.plus));
        }
    }
    lines
}

/// A gunlance's shells, a switch axe's phial or a horn's notes, as one line.
pub(super) fn part_line(part: &mh3u_core::weapon_extras::Part) -> Option<Line<'static>> {
    use mh3u_core::weapon_extras::Part;
    let (label, body): (&str, Vec<Span<'static>>) = match part {
        Part::None => return None,
        Part::Shell { kind, level } => ("Shells", vec![Span::raw(format!("{kind} Lv {level}"))]),
        Part::Phial(kind) => ("Phial", vec![Span::raw(kind.clone())]),
        Part::Notes(colors) => ("Notes", note_tiles(colors)),
    };
    let mut spans = vec![Span::styled(format!("{label:<10}"), muted())];
    spans.extend(body);
    Some(Line::from(spans))
}

/// Notes as tiles, with a space between so they read one by one.
fn note_tiles(colors: &[String]) -> Vec<Span<'static>> {
    colors.iter().flat_map(|c| [theme::note_tile(c), Span::raw(" ")]).collect()
}

/// A hunting horn's songs: the notes to play, what they do, for how long. Empty for other weapons.
fn song_lines(app: &App, kind: u8, id: u16) -> Vec<Line<'static>> {
    let songs = app.game.horn_songs(kind, id);
    if songs.is_empty() {
        return Vec::new();
    }
    let seconds = |s: Option<mh3u_core::horn_songs::Seconds>, plus: bool| -> String {
        match s {
            None => "-".to_string(),
            Some(s) => {
                let sign = if plus { "+" } else { "" };
                match s.maestro {
                    Some(m) => format!("{sign}{} ({sign}{m})", s.plain),
                    None => format!("{sign}{}", s.plain),
                }
            }
        }
    };
    let name = |e: &mh3u_core::horn_songs::Effect| {
        if e.self_only {
            format!("{} (you only)", e.name)
        } else {
            e.name.clone()
        }
    };
    let mut lines = vec![
        Line::raw(""),
        Line::styled("Songs   seconds, with Horn Maestro in brackets", muted()),
    ];
    for s in songs {
        let mut spans = note_tiles(&s.notes);
        spans.push(Span::styled(name(&s.effect), bold()));
        lines.push(Line::from(spans));
        let mut more = String::from("  ");
        if s.duration.is_some() {
            more += &format!("{} s", seconds(s.duration, false));
        }
        if s.extension.is_some() {
            more += &format!(", {} s more when played again", seconds(s.extension, true));
        }
        if let Some(e2) = &s.effect2 {
            more += &format!("  then {}", name(e2));
        }
        if !more.trim().is_empty() {
            lines.push(Line::styled(more, muted()));
        }
    }
    lines
}

/// Points of sharpness shown by one cell of the bar.
const POINTS_PER_CELL: u32 = 3;

/// A sharpness bar drawn in the colors of the game's, one cell for every few points. Cells are rounded from the running total so the bar
/// is as long as the whole.
fn sharpness_line(label: &str, bar: &mh3u_core::weapon_extras::Bar) -> Line<'static> {
    let mut spans = vec![Span::styled(format!("{label:<10}"), muted())];
    spans.extend(sharpness_spans(bar, POINTS_PER_CELL));
    Line::from(spans)
}

/// The bar's colored cells, one for every `per_cell` points.
pub(super) fn sharpness_spans(bar: &mh3u_core::weapon_extras::Bar, per_cell: u32) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let (mut total, mut drawn) = (0u32, 0u32);
    for (color, &points) in bar.iter().enumerate() {
        total += u32::from(points);
        let upto = (total + per_cell / 2) / per_cell;
        if upto > drawn {
            spans.push(Span::styled(
                "█".repeat((upto - drawn) as usize),
                Style::new().fg(theme::sharpness_color(color)),
            ));
            drawn = upto;
        }
    }
    spans
}

/// Whether the blacksmith offers a piece, by the rule in `mh3u_core::blacksmith` and from pieces seen on offer before.
pub(super) fn unlock_line(app: &App, kind: u8, id: u16) -> Option<Line<'static>> {
    use mh3u_core::{blacksmith::Unlock, drops::Rank};
    let label = |text: &str, style: Style| -> Line<'static> {
        Line::from(vec![Span::styled("Blacksmith  ", muted()), Span::styled(text.to_string(), style)])
    };
    let rank = |rank: Rank| match rank {
        Rank::Low => "low rank",
        Rank::High => "high rank (6★+ quests when playing alone)",
        Rank::G => "G rank (after Throne of the Abyss)",
    };
    let monsters = |ids: &[u16]| {
        let mut names: Vec<&str> = ids.iter().filter_map(|&m| app.game.monster_name(m)).collect();
        names.dedup();
        let more = names.len().saturating_sub(4);
        names.truncate(4);
        let mut text = names.join(", ");
        if more > 0 {
            text += &format!(" and {more} more");
        }
        text
    };
    Some(match app.offer(kind, id)? {
        Offer::Earlier => label("✔ on offer (seen earlier; the list never shrinks)", good()),
        Offer::Rule(Unlock::Starter) => label("✔ starting gear, always on offer", good()),
        Offer::Rule(Unlock::Hunted(m)) => label(
            &format!(
                "✔ on offer (you have hunted {}, which drops its first material)",
                app.game.monster_name(m).unwrap_or("?")
            ),
            good(),
        ),
        Offer::Rule(Unlock::NeedsHunt(r, ids)) => label(&format!("not on offer yet: hunt {} in {}", monsters(&ids), rank(r)), warn()),
        Offer::Rule(Unlock::NeedsRank(r)) => label(&format!("not on offer yet: its first material drops only in {}", rank(r)), warn()),
        Offer::Rule(Unlock::Special) => label("a special piece, unlocked some other way", muted()),
        Offer::Rule(Unlock::Unknown(_)) => label("unknown how this is unlocked (its first material is not a monster drop)", muted()),
    })
}

/// The details of one piece of equipment: stats, then what it takes to create or upgrade it with the materials you have.
/// `craftable` is `Some` on the Crafting tab, where the piece may not be owned; `None` on the Equipment tab.
pub(super) fn piece_details(app: &App, kind: u8, id: u16, name: &str, craftable: Option<bool>) -> Vec<Line<'static>> {
    let zenny = app.save.zenny;
    let mut lines: Vec<Line> = vec![
        Line::styled(name.to_string(), bold()),
        Line::styled(app.game.equipment_kind_label(kind).unwrap_or("?").to_string(), muted()),
    ];
    if let Some(text) = app.game.equipment_description(kind, id) {
        lines.push(Line::styled(text.to_string(), muted()));
    }
    if let Some(a) = app.game.armor_stats(kind, id) {
        lines.extend(armor_lines(app, a));
    } else if let Some(w) = app.game.weapon_stats(kind, id) {
        lines.extend(weapon_lines(app, kind, id, w));
        lines.extend(song_lines(app, kind, id));
    }
    lines.extend(unlock_line(app, kind, id));
    lines.push(Line::raw(""));
    if let Some(recipe) = app.create_recipe(kind, id) {
        lines.push(cost_heading("Create from scratch", app.cost(kind, id, Route::Create), zenny));
        for m in &recipe.materials {
            lines.push(theme::material_line(
                app.game.item_name(m.id).unwrap_or("?"),
                app.save.item_count(m.id),
                u32::from(m.count),
            ));
        }
        lines.push(Line::raw(""));
    }
    if let Some(up) = app.upgrade_recipe(kind, id) {
        lines.push(cost_heading("Upgrade from", app.cost(kind, id, Route::Upgrade), zenny));
        for &parent in &up.parents {
            let parent_name = app.game.equipment_name(kind, parent).unwrap_or("?");
            let (mark, style) = if app.save.owns_equipment(kind, parent) {
                ("● owned", good())
            } else {
                ("○ not owned", bad())
            };
            lines.push(Line::from(vec![
                Span::raw(format!("  {parent_name:<24}")),
                Span::styled(mark, style),
            ]));
        }
        for m in &up.materials {
            lines.push(theme::material_line(
                app.game.item_name(m.id).unwrap_or("?"),
                app.save.item_count(m.id),
                u32::from(m.count),
            ));
        }
        lines.push(Line::raw(""));
    }
    lines.extend(cheapest_way_lines(app, kind, id));
    let children = app.upgrade_children(kind, id);
    if !children.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("Upgrades into", bold()),
            Span::styled("  (t for the tree)", muted()),
        ]));
        for child in children {
            let child_name = app.game.equipment_name(kind, child).unwrap_or("?");
            let (mark, style) = if app.save.owns_equipment(kind, child) {
                ("● owned", good())
            } else {
                ("", muted())
            };
            lines.push(Line::from(vec![
                Span::raw(format!("  {child_name:<24}")),
                Span::styled(mark, style),
            ]));
        }
        lines.push(Line::raw(""));
    }
    if let Some(craftable) = craftable {
        lines.push(if craftable {
            Line::styled("✔ You can make this now.", good())
        } else {
            Line::styled("✘ Not available yet.", bad())
        });
    }
    let any_cost_missing = (app.create_recipe(kind, id).is_some() && app.cost(kind, id, Route::Create).is_none())
        || (app.upgrade_recipe(kind, id).is_some() && app.cost(kind, id, Route::Upgrade).is_none());
    if any_cost_missing {
        lines.push(Line::raw(""));
        lines.push(Line::styled("Costs are learned by watching you craft in live mode.", muted()));
    }
    lines
}

/// The cheapest way to get a weapon you do not own: each step with its fee, then the materials for all of them. Nothing for armor, a
/// weapon you own, or when the route is just making it (the "Create from scratch" section says that).
fn cheapest_way_lines(app: &App, kind: u8, id: u16) -> Vec<Line<'static>> {
    use mh3u_app::upgrade_path::How;
    if app.save.owns_equipment(kind, id) {
        return Vec::new();
    }
    let Some(path) = app.cheapest_path(kind, id) else {
        return Vec::new();
    };
    if path.steps.len() < 2 {
        return Vec::new();
    }
    let zenny = app.save.zenny;
    let total = path.zenny();
    let mut heading = vec![
        Span::styled("Cheapest way", bold()),
        Span::styled(
            format!(" · {} z", group_digits(u64::from(total))),
            if total <= zenny { good() } else { bad() },
        ),
    ];
    if path.unknown_prices() > 0 {
        heading.push(Span::styled(format!(" + {} price(s) not seen yet", path.unknown_prices()), muted()));
    }
    // the dearer alternative, if the weapon can also be made from scratch
    if let Some((scratch, _)) = app.create_recipe(kind, id).and_then(|_| app.cost(kind, id, Route::Create)) {
        heading.push(Span::styled(
            format!("  (from scratch {} z)", group_digits(u64::from(scratch))),
            muted(),
        ));
    }
    let mut lines = vec![Line::from(heading)];
    for step in &path.steps {
        let name = app.game.equipment_name(kind, step.weapon).unwrap_or("?");
        let (mark, how) = match step.how {
            How::Owned => ("● ", "owned"),
            How::Create => ("▸ ", "make"),
            How::Upgrade => ("▸ ", "upgrade"),
        };
        let mut spans = vec![
            Span::styled(mark, if step.how == How::Owned { good() } else { accent() }),
            Span::raw(format!("{:<24}", fit(name, 24))),
            Span::styled(format!("{how:<8}"), muted()),
        ];
        if step.how != How::Owned {
            spans.push(Span::raw(match step.cost.zenny {
                Some(z) => format!("{} z", group_digits(u64::from(z))),
                None => "price not seen".to_string(),
            }));
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::styled("  materials for every step:", muted()));
    for m in path.materials() {
        lines.push(theme::material_line(
            app.game.item_name(m.id).unwrap_or("?"),
            app.save.item_count(m.id),
            u32::from(m.count),
        ));
    }
    lines.push(Line::raw(""));
    lines
}

//! Watching for forging costs in the running game (see `mh3u_core::prices`).

use super::*;

/// Forging costs seen in the game (see `mh3u_core::prices`) and the watcher that finds them.
pub struct PriceBook {
    pub(super) ledger: Ledger,
    pub(super) tracker: PriceTracker,
}

impl PriceBook {
    pub(super) fn new(ledger: Ledger) -> PriceBook {
        PriceBook {
            ledger,
            tracker: PriceTracker::default(),
        }
    }
}

impl App {
    /// Let the price tracker judge what has settled, and keep any cost it is sure about.
    pub(super) fn tick_prices(&mut self) {
        let book = GameBook {
            game: &self.game,
            create: &self.craft.learned_create,
            upgrade: &self.craft.learned_upgrade,
        };
        let outcome = self.costs.tracker.finish(Instant::now(), &book);
        let when = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let (kind, id, route, cost, source, materials, parent) = match outcome {
            None => return,
            Some(Outcome::Skipped(skip)) => {
                let message = self.skip_message(&skip);
                self.log_tracker(&format!("skipped\t{message}\t{}", self.skip_detail(&skip)));
                return self.status = message;
            }
            Some(Outcome::Seen(o)) => (o.kind, o.id, o.route, o.cost, Source::Seen, None, None),
            Some(Outcome::Learned(l)) => (l.kind, l.id, l.route, l.cost, Source::Learned, Some(l.materials), l.parent),
        };
        let name = self.game.equipment_name(kind, id).unwrap_or("?").to_string();
        let learned = materials.is_some();
        let recipe_text = materials.as_ref().map(|m| {
            let items: Vec<String> = m
                .iter()
                .map(|s| format!("{} x{}", self.game.item_name(s.id).unwrap_or("?"), s.count))
                .collect();
            if items.is_empty() {
                "no materials".to_string()
            } else {
                items.join(", ")
            }
        });
        let entry = PriceEntry {
            kind,
            id,
            route,
            cost,
            source,
            when,
            name: name.clone(),
            materials,
            parent,
        };
        let (z, how) = (group_digits(u64::from(cost)), route.label());
        let message = match (self.costs.ledger.record(entry), recipe_text) {
            (_, Some(recipe)) => format!("learned {name}: {z} z to {how}, recipe {recipe}"),
            (Recorded::New, None) => format!("price noted: {name} costs {z} z to {how}"),
            (Recorded::Unchanged, None) => format!("{name} costs {z} z to {how} (as noted before)"),
            (Recorded::Changed(old), None) => format!("price changed: {name} now {z} z to {how} (was {} z)", group_digits(u64::from(old))),
        };
        self.log_tracker(&format!("recorded\t{message}"));
        self.status = message;
        if learned {
            // the new recipe makes the piece appear in the Crafting list and the wishlist
            self.rebuild_learned();
            self.craft.catalog = self.build_catalog();
            self.refresh_pieces();
        }
        if let Some(path) = self.files.as_ref().map(|f| &f.prices)
            && let Err(message) = crate::files::save(path, &self.costs.ledger.format(), "prices")
        {
            self.status = message;
        }
    }

    /// Append a line to `tracker.log` next to the price ledger: when a crafting was recorded or skipped, and why. It is
    /// what to look at when a price you expected is missing.
    pub(super) fn log_tracker(&self, line: &str) {
        use std::io::Write;
        let Some(path) = self.files.as_ref().map(|f| &f.tracker_log) else {
            return;
        };
        let when = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{when}\t{line}");
        }
    }

    /// What changed in a skipped transaction: the wallet and every item (pouch and box together).
    pub(super) fn skip_detail(&self, skip: &Skip) -> String {
        let items: Vec<String> = skip
            .items
            .iter()
            .map(|(id, d)| format!("{} {d:+}", self.game.item_name(*id).unwrap_or("?")))
            .collect();
        format!(
            "wallet {:+}\titems {}",
            skip.zenny,
            if items.is_empty() { "none".to_string() } else { items.join(", ") }
        )
    }

    /// Explain a piece whose price was not recorded.
    pub(super) fn skip_message(&self, skip: &Skip) -> String {
        let names: Vec<&str> = skip
            .pieces
            .iter()
            .map(|&(k, i)| self.game.equipment_name(k, i).unwrap_or("?"))
            .collect();
        let why = match skip.why {
            SkipReason::NoCost => "the wallet did not go down",
            SkipReason::SeveralPieces => "several pieces appeared at once, craft one at a time",
            SkipReason::UnknownRecipe => "the app has no recipe for it",
            SkipReason::DifferentMaterials => "the items used don't match its recipe (did something else change at the same time?)",
        };
        format!("price not recorded for {}: {why}", names.join(", "))
    }
}

//! The Pickups tab and the live notices: what the hunter gained, and which wishlist pieces just became craftable.

use super::*;
use crate::gains::{self, Counts, Log};

/// The log of pickups and the state needed to keep it: what the hunter held at the last live update.
pub struct GainsTab {
    pub log: Log,
    /// The pouch and box counts and the zenny at the last live update; `None` until live data arrives, so the first update after
    /// connecting (or after the game loads another hunter) is only a starting point and nothing in it is a gain.
    baseline: Option<(Counts, u32)>,
    pub state: ListState,
}

impl GainsTab {
    pub(super) fn new(log: Log) -> GainsTab {
        GainsTab {
            log,
            baseline: None,
            state: ListState::default().with_selected(Some(0)),
        }
    }
}

/// The hunter's log from its file (empty where there is none).
pub(super) fn read_log(files: Option<&Files>) -> Log {
    files
        .and_then(|f| std::fs::read_to_string(&f.gains).ok())
        .map(|t| Log::parse(&t))
        .unwrap_or_default()
}

fn now() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl App {
    /// The next live update is a new starting point (the game connected, dropped the hunter or loaded another).
    pub(super) fn forget_gain_baseline(&mut self) {
        self.gains.baseline = None;
    }

    /// Another hunter's pickups.
    pub(super) fn load_gains(&mut self) {
        self.gains = GainsTab::new(read_log(self.files.as_ref()));
    }

    /// Compare a live update with the one before it and log what was gained. Call before `apply_save`, while `self.save` is still the
    /// previous data: items the wishlist is short of are starred in the notice. Returns the notice.
    pub(super) fn record_gains(&mut self, save: &Save) -> Option<String> {
        let (counts, zenny) = (gains::counts(save), save.zenny);
        let baseline = self.gains.baseline.replace((counts.clone(), zenny));
        let (before, zenny_before) = baseline?;
        let got = self.gains.log.observe(&before, zenny_before, &counts, zenny, now());
        if got.is_empty() {
            return None;
        }
        if let Some(f) = &self.files
            && let Err(e) = crate::files::save(&f.gains, &self.gains.log.format(), "pickups")
        {
            self.status = e;
        }
        self.gains.state.select(Some(0)); // the list shows the newest first
        let short = self.missing_for_wishlist();
        let named: Vec<String> = got
            .iter()
            .take(4)
            .map(|&(id, n)| {
                let star = if short.contains_key(&id) { "★ " } else { "" };
                format!("{star}{} x{n}", self.game.item_name(id).unwrap_or("?"))
            })
            .collect();
        let more = got.len().saturating_sub(named.len());
        Some(format!(
            "picked up {}{}",
            named.join(", "),
            if more > 0 { format!(" and {more} more") } else { String::new() }
        ))
    }

    /// The wishlisted pieces the hunter can make right now.
    pub(super) fn craftable_wishes(&self) -> HashSet<(u8, u16)> {
        self.wish
            .items
            .iter()
            .copied()
            .filter(|&(kind, id)| self.can_make_now(kind, id))
            .collect()
    }

    /// A notice naming the wishlisted pieces that are craftable now but were not in `before`.
    pub(super) fn newly_craftable(&self, before: &HashSet<(u8, u16)>) -> Option<String> {
        let mut names: Vec<&str> = self
            .craftable_wishes()
            .difference(before)
            .filter_map(|&(kind, id)| self.game.equipment_name(kind, id))
            .collect();
        names.sort_unstable();
        (!names.is_empty()).then(|| format!("now craftable: {}", names.join(", ")))
    }
}

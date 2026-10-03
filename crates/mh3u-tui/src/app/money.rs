//! Zenny: formatting and the recent change shown in the header.

use super::*;

/// How long a change in zenny stays highlighted next to the total.
pub(super) const ZENNY_SHOW: Duration = Duration::from_secs(15);

/// `1234567` -> `"1,234,567"`.
pub fn group_digits(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `+1,200` or `-300`.
pub fn signed_zenny(delta: i64) -> String {
    format!("{}{}", if delta < 0 { '-' } else { '+' }, group_digits(delta.unsigned_abs()))
}

/// The running zenny change after the total moved from `old` to `new`: changes close together are added up, and a change
/// that undoes the earlier ones clears it.
pub(super) fn next_zenny_change(prev: Option<(i64, Instant)>, old: u32, new: u32, now: Instant) -> Option<(i64, Instant)> {
    if old == new {
        return prev;
    }
    let carried = prev.filter(|&(_, at)| now.duration_since(at) < ZENNY_SHOW).map_or(0, |(d, _)| d);
    let total = carried + (i64::from(new) - i64::from(old));
    (total != 0).then_some((total, now))
}

impl App {
    /// The zenny change to show next to the total, if one happened in the last few seconds.
    pub fn zenny_change(&self) -> Option<i64> {
        self.zenny_change
            .filter(|(_, at)| at.elapsed() < ZENNY_SHOW)
            .map(|(delta, _)| delta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_are_grouped() {
        assert_eq!(group_digits(0), "0");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(1500), "1,500");
        assert_eq!(group_digits(1_234_567), "1,234,567");
        assert_eq!(signed_zenny(1200), "+1,200");
        assert_eq!(signed_zenny(-300), "-300");
    }

    #[test]
    fn zenny_changes_add_up_while_recent_and_reset_afterwards() {
        let t0 = Instant::now();
        let step = Duration::from_secs;
        let first = next_zenny_change(None, 1500, 2700, t0);
        assert_eq!(first.map(|c| c.0), Some(1200));
        // a second change a few seconds later adds to the first
        let second = next_zenny_change(first, 2700, 2400, t0 + step(5));
        assert_eq!(second.map(|c| c.0), Some(900));
        // no change leaves it alone
        assert_eq!(next_zenny_change(second, 2400, 2400, t0 + step(6)), second);
        // after the window it starts over
        assert_eq!(next_zenny_change(second, 2400, 2500, t0 + step(60)).map(|c| c.0), Some(100));
        // undoing the earlier change clears it
        assert_eq!(next_zenny_change(first, 2700, 1500, t0 + step(2)), None);
    }
}

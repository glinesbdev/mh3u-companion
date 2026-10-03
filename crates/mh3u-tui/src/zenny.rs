//! Planning the zenny for a list of pieces: how much more to earn, and what to make first.

/// One piece to make: its position in the caller's list and the fee for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub index: usize,
    pub fee: u32,
    /// What making it and everything before it in the order costs together.
    pub running: u64,
    /// You can pay for it and everything before it with the zenny you have.
    pub affordable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Plan {
    /// Cheapest first, so as many pieces as possible are made before the money runs out.
    pub order: Vec<Step>,
    /// The fees of every piece whose fee is known.
    pub total: u64,
    /// Pieces whose fee is not known (left out of the total and the order).
    pub unknown: usize,
    /// How much more zenny the known fees take than you have.
    pub shortfall: u64,
}

/// Plan the fees (`None` where a fee is not known) against the zenny you have.
pub fn plan(fees: &[Option<u32>], zenny: u64) -> Plan {
    let mut known: Vec<(usize, u32)> = fees.iter().enumerate().filter_map(|(i, f)| f.map(|f| (i, f))).collect();
    known.sort_by_key(|&(index, fee)| (fee, index));
    let mut running = 0u64;
    let order = known
        .into_iter()
        .map(|(index, fee)| {
            running += u64::from(fee);
            Step {
                index,
                fee,
                running,
                affordable: running <= zenny,
            }
        })
        .collect();
    Plan {
        order,
        total: running,
        unknown: fees.iter().filter(|f| f.is_none()).count(),
        shortfall: running.saturating_sub(zenny),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cheapest_are_made_first_and_the_money_runs_out_in_order() {
        let p = plan(&[Some(750), Some(200), None, Some(450)], 700);
        let order: Vec<(usize, u64, bool)> = p.order.iter().map(|s| (s.index, s.running, s.affordable)).collect();
        assert_eq!(order, [(1, 200, true), (3, 650, true), (0, 1400, false)]);
        assert_eq!((p.total, p.unknown, p.shortfall), (1400, 1, 700));
    }

    #[test]
    fn nothing_is_short_when_the_zenny_covers_it_and_ties_keep_their_order() {
        let p = plan(&[Some(100), Some(100)], 500);
        assert_eq!(p.shortfall, 0);
        assert!(p.order.iter().all(|s| s.affordable));
        assert_eq!(p.order.iter().map(|s| s.index).collect::<Vec<_>>(), [0, 1]);
    }

    #[test]
    fn an_empty_or_unknown_list_has_no_plan() {
        assert_eq!(plan(&[], 100), Plan::default());
        let p = plan(&[None, None], 100);
        assert!(p.order.is_empty());
        assert_eq!((p.total, p.unknown, p.shortfall), (0, 2, 0));
    }
}

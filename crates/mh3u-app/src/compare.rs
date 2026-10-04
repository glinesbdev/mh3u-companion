//! Comparing weapons side by side: which value in a row is the best.

/// For one row of the comparison, which columns hold the best value. Nothing is marked when fewer than two columns have a value or
/// when they are all alike, because then no weapon is better in that row. A column without a value (`None`) is never the best.
pub fn best_marks(values: &[Option<i64>], higher_is_better: bool) -> Vec<bool> {
    let known: Vec<i64> = values.iter().flatten().copied().collect();
    let best = if higher_is_better { known.iter().max() } else { known.iter().min() };
    let all_alike = known.windows(2).all(|w| w[0] == w[1]);
    match best {
        Some(best) if known.len() >= 2 && !all_alike => values.iter().map(|v| *v == Some(*best)).collect(),
        _ => vec![false; values.len()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_highest_or_lowest_value_is_marked() {
        assert_eq!(best_marks(&[Some(5), Some(9), Some(7)], true), [false, true, false]);
        assert_eq!(best_marks(&[Some(5), Some(9), Some(7)], false), [true, false, false]);
    }

    #[test]
    fn ties_are_all_marked_and_a_row_that_is_all_alike_has_no_best() {
        assert_eq!(best_marks(&[Some(9), Some(9), Some(2)], true), [true, true, false]);
        assert_eq!(best_marks(&[Some(4), Some(4)], true), [false, false]);
    }

    #[test]
    fn a_missing_value_is_never_best_and_one_value_alone_is_not_compared() {
        assert_eq!(best_marks(&[None, Some(3), Some(8)], false), [false, true, false]);
        assert_eq!(best_marks(&[None, Some(3)], true), [false, false], "nothing to compare it with");
        assert_eq!(best_marks(&[], true), Vec::<bool>::new());
    }
}

//! Spare items: how many of an item you hold can go (be sold) without making anything you want harder to get.
//!
//! You need to keep what the wishlist asks for and enough for any one piece that still takes the item (you might make that one next).
//! Whatever is above that is spare. An item no piece you do not own takes at all has nothing to keep it for.

/// What asks for an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Demand {
    /// How many the wishlist's shopping list needs in all.
    pub wishlist: u32,
    /// The most any single piece you do not own takes.
    pub biggest_piece: u32,
    /// Recipes of pieces you do not own that take it.
    pub unowned_uses: usize,
    /// Recipes of pieces you own that take it (making another copy).
    pub owned_uses: usize,
}

/// Why some of it is not spare, or why all of it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
    /// No recipe takes it.
    NoRecipe,
    /// Only pieces you already own take it.
    OnlyOwnedPieces,
    /// The wishlist needs more than any one piece does.
    Wishlist,
    /// One piece you do not own takes this many.
    OnePiece,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spare {
    /// How many to hold on to.
    pub keep: u32,
    /// How many can go.
    pub spare: u32,
    pub why: Why,
}

pub fn assess(have: u32, demand: &Demand) -> Spare {
    let (keep, why) = if demand.unowned_uses == 0 {
        // nothing left to make takes it, except what the wishlist plans (which can only be for pieces you do not own)
        let why = if demand.owned_uses == 0 {
            Why::NoRecipe
        } else {
            Why::OnlyOwnedPieces
        };
        (demand.wishlist, why)
    } else if demand.wishlist >= demand.biggest_piece {
        (demand.wishlist, Why::Wishlist)
    } else {
        (demand.biggest_piece, Why::OnePiece)
    };
    Spare {
        keep,
        spare: have.saturating_sub(keep),
        why,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demand(wishlist: u32, biggest: u32, unowned: usize, owned: usize) -> Demand {
        Demand {
            wishlist,
            biggest_piece: biggest,
            unowned_uses: unowned,
            owned_uses: owned,
        }
    }

    #[test]
    fn an_item_no_recipe_takes_is_all_spare() {
        let s = assess(7, &demand(0, 0, 0, 0));
        assert_eq!((s.keep, s.spare, s.why), (0, 7, Why::NoRecipe));
    }

    #[test]
    fn an_item_only_owned_pieces_take_is_spare_too() {
        let s = assess(7, &demand(0, 0, 0, 3));
        assert_eq!((s.keep, s.spare, s.why), (0, 7, Why::OnlyOwnedPieces));
    }

    #[test]
    fn enough_for_the_biggest_single_piece_is_kept() {
        let s = assess(10, &demand(0, 4, 5, 0));
        assert_eq!((s.keep, s.spare, s.why), (4, 6, Why::OnePiece));
        let none = assess(3, &demand(0, 4, 5, 0));
        assert_eq!((none.keep, none.spare), (4, 0), "holding less than that leaves nothing spare");
    }

    #[test]
    fn the_wishlist_counts_when_it_asks_for_more_than_one_piece_does() {
        let s = assess(10, &demand(8, 4, 5, 0));
        assert_eq!((s.keep, s.spare, s.why), (8, 2, Why::Wishlist));
        let small = assess(10, &demand(2, 4, 5, 0));
        assert_eq!((small.keep, small.why), (4, Why::OnePiece));
    }
}

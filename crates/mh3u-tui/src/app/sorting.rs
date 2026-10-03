//! How the lists are ordered: the sort choices of the Items, Crafting, Equipment and Monsters tabs.

/// Ordering of the crafting list.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PieceSort {
    GameOrder,
    Name,
    CraftableFirst,
    OwnedFirst,
    /// Cheapest forging fee first; pieces with no known fee last.
    Cost,
}

impl PieceSort {
    pub(super) fn next(self) -> PieceSort {
        match self {
            PieceSort::GameOrder => PieceSort::Name,
            PieceSort::Name => PieceSort::CraftableFirst,
            PieceSort::CraftableFirst => PieceSort::OwnedFirst,
            PieceSort::OwnedFirst => PieceSort::Cost,
            PieceSort::Cost => PieceSort::GameOrder,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PieceSort::GameOrder => "game order",
            PieceSort::Name => "name",
            PieceSort::CraftableFirst => "craftable first",
            PieceSort::OwnedFirst => "owned first",
            PieceSort::Cost => "cheapest first",
        }
    }
}

/// Ordering of the item box list.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BoxSort {
    BoxOrder,
    Name,
    Quantity,
}

impl BoxSort {
    pub(super) fn next(self) -> BoxSort {
        match self {
            BoxSort::BoxOrder => BoxSort::Name,
            BoxSort::Name => BoxSort::Quantity,
            BoxSort::Quantity => BoxSort::BoxOrder,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            BoxSort::BoxOrder => "box order",
            BoxSort::Name => "name",
            BoxSort::Quantity => "quantity",
        }
    }
}

/// Ordering of the monster list.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum MonsterSort {
    #[default]
    GameOrder,
    Name,
    /// Monsters that drop the most of what the wishlist still needs first.
    Needed,
}

impl MonsterSort {
    pub(super) fn next(self) -> MonsterSort {
        match self {
            MonsterSort::GameOrder => MonsterSort::Name,
            MonsterSort::Name => MonsterSort::Needed,
            MonsterSort::Needed => MonsterSort::GameOrder,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MonsterSort::GameOrder => "game order",
            MonsterSort::Name => "name",
            MonsterSort::Needed => "wishlist needs first",
        }
    }
}

/// Ordering of the equipment box list.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EquipSort {
    BoxOrder,
    Name,
    Rarity,
    Type,
    WornFirst,
}

impl EquipSort {
    pub(super) fn next(self) -> EquipSort {
        match self {
            EquipSort::BoxOrder => EquipSort::Name,
            EquipSort::Name => EquipSort::Rarity,
            EquipSort::Rarity => EquipSort::Type,
            EquipSort::Type => EquipSort::WornFirst,
            EquipSort::WornFirst => EquipSort::BoxOrder,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EquipSort::BoxOrder => "box order",
            EquipSort::Name => "name",
            EquipSort::Rarity => "rarity",
            EquipSort::Type => "type",
            EquipSort::WornFirst => "worn first",
        }
    }
}

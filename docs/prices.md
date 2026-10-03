# Forging costs

The forging price of every weapon and armor piece is in the game files (last section below), and the app reads it from there.
This ledger is what the app has seen: when a piece is crafted, the wallet drops by its price, and live mode watches for that and
keeps what it learns. A price seen in play wins over the game files, and the ledger still holds recipes the game data lacks.

## The ledger

`~/.local/share/mh3u-companion/prices.tsv` (or under `$XDG_DATA_HOME`), plain text, one line per piece and route:

```
# kind  id  route    cost  source  when        name
5       2   create   300   seen    1790996571  Piscine Mask
7       2   upgrade  1500  notes   1790996500  Iron Sword+
```

`kind` and `id` are the same numbers used everywhere else (`docs/formats.md`). The route matters: creating from scratch and
upgrading cost different amounts. `source` is `seen` (watched by the tracker) or `notes` (typed in with `prices-add` from the
game's own forge screen). The name is only for people reading the file.

The recipe pane shows the cost beside each route (`Create from scratch · 300 z (seen)`), and the Wishlist tab totals the known
costs of the planned routes ("Zenny: 8,800 z known + 2 piece(s) not seen yet · you have 4,700").

## Recipes the game data lacks

The recipe tables don't cover everything the blacksmith offers (for example Bone Kris has no recipe in them, and Commander's
Dagger only an upgrade recipe, although it can be created). When a piece appears and the wallet drops, but there is **no known
recipe for the route used** (create, or upgrade with the parent that vanished), the tracker takes what vanished as the recipe:

```
8   52   create   600   learned   1791000486   Bone Kris   175:1,214:2
```

The two extra columns are the recipe (`item id:count`) and, for an upgrade, the weapon that was used up. It is only learned
when nothing else happened to the items at that moment (no item went up) and one piece appeared. Learned recipes make the piece
appear in the Crafting list and the wishlist, `stock` can cover them, and crafting the piece again records its price against the
learned recipe. A known recipe that doesn't match is never overwritten; that is reported as a skip. If a learned recipe turns out
to be wrong (say something else was used at the same moment), delete its line from the file.

## How the tracker decides

It sums every change in the live save until things have been quiet for a second, then records a cost only if all of this holds:

- the wallet went down;
- exactly one piece appeared in the equipment box;
- the items that disappeared (pouch and box together) are **exactly** that piece's recipe, nothing more, nothing less;
- for an upgrade, one of its parent weapons disappeared too (an upgrade replaces the parent).

The game can change things in two steps (the wallet and materials first, the piece after an animation, or the other way round), so
a transaction that looks like half a crafting is held for up to 10 seconds to wait for its other half. Only craft-shaped halves
are held: buying or selling items, or a fee on its own, is judged after a second and never mixed into a later crafting.

Every recorded or skipped crafting is also appended to `tracker.log` next to the ledger, with the wallet change and every item's
change for the skipped ones, so a missing price can be looked into afterwards.

Buying items, selling things, crafting two pieces at once, or using extra items at the same moment do not qualify and are
skipped. When a piece appeared but its price could not be recorded, the status line says why: the wallet did not go down, several
pieces appeared at once (craft one at a time), the app has no recipe for it, or the items used don't match its recipe. Changes
that brought no new piece (buying, selling, moving items) are not reported. Changes made by the `--debug-edit` command line are not counted: right after writing, the tracker is told what the game should now look like, so a crafting a moment later is still recorded. A cost that differs from a stored one
replaces it and the status line says so.

## Finding where the game stores prices

```
mh3u-tools prices-hint <game dir> ~/.local/share/mh3u-companion/prices.tsv
```

For each ledger entry it looks for the cost (as 1, 2 or 4 bytes, optionally divided by 10, 50, 100 or 1000) in the executable's
data sections, and for each possible spacing between pieces works out the table start that would put the cost at
`start + spacing x id`. If several different pieces of one kind agree on the same layout, that is where the table is.
One-byte values match by chance almost anywhere, so they need five pieces to agree and the others three.

With the first eleven prices (mostly one per kind) it found nothing, which is the honest answer: there is too little data. What
makes it conclusive is **many pieces of one kind, ideally with consecutive ids**: for example the first eight or ten head
pieces (ids 2 to 11), which are cheap to make. Add them to the wishlist, run `stock` in `--debug-edit` mode, craft them, and the
tracker records each cost; then run the hint again.

The Crafting tab's `u` key shows only the pieces you own that have no price in the ledger yet, which is the list of what is
still missing (your starting gear will be on it, since it can't be crafted).

## Where the game keeps prices

**Weapons: found** (`weapons.rs`). Each weapon type has a table of stats records in the executable's data section, one per weapon
id (record 0 is a placeholder), and each record holds the price as a big-endian u32. **Upgrading costs exactly that value;
creating from scratch costs 1.5 times it.** Nine types use 28-byte records (price at +4): great sword `0x51dd4`, sword & shield
`0x52cb4`, hammer `0x53c20`, long sword `0x54ae4`, switch axe `0x55778`, gunlance `0x563f0`, dual blades `0x57084`, hunting horn
`0x57e4c`, lance `0x58974`. Heavy bowgun (`0x4bf08`), light bowgun (`0x49924`) and bow (`0x4e2f8`) use 100-byte records with the
price in the first 4 bytes. (Offsets are into the data section at `0x1010a000`; see `weapons.rs` for the exact price-field
addresses.) All 28 weapon prices in the ledger (create and upgrade, every type) match these tables exactly. The app shows these
as "game data" and prefers them over ledger values; the ledger stays as a check. Earlier notes here said the weapon table's
price field was not the forging price: that was wrong, the comparisons used the wrong route and misaligned records.

**Armor: found** (`armor.rs`). Armor has its own set of five tables (body, arms, waist, legs, head), each a run of **32-byte rows
indexed by piece id** and laid end to end from `0x22958` in the data section. A big-endian u16 at byte 14 of the row holds
**half the zenny price**: a Jaggi piece costs 1,150 and stores 575. Armor has no upgrade route, so this is the create cost.

Why the earlier searches missed it: `prices-hint` looked for the shown price divided by 1, 10, 50, 100 or 1000 and never by 2,
and it assumed spacing of up to 64 bytes but only tried a few. Seeing a column of values such as 25, 100, 225, 275, 325, 375 and
575 next to a column of growth numbers in an unrelated dump is what gave it away: they are exactly the prices of Leather Vest
(50), Chainmail Vest (200), Hunter's Mail (450), Loc Lac Shawl (550), Bone Mail (650), Alloy Mail (750) and Jaggi Mail (1,150).

How well it holds: all 38 armor prices in the ledger that were seen in the game agree (the 39th entry is a hand-written note, Yukumo
Dogi at 1,100, where the table says 550). Against a published armor list (966 head, body and arm pieces), 942 agree. The 24
that differ are a few sets (Qurupeco, Yukumo, Rathian X and Rath Heart Z, Silhouette Casque) where the list gives a different
price; the game's own screens have not been checked for those, so the app shows the game's table value for them and a price
you saw in play overrides it.

The same rows hold more: bytes 3 to 8 are upgrade-level data that decides maximum defense (below), byte 2 is 1 for the gunner
version of a piece and 0 for the blademaster version, and byte 1 is some other id. Maximum defense is not decoded yet: it is
determined by the six growth bytes (107 distinct rows, 5 conflicts against the published list), but no closed formula was found.

Other things tried first, and why they failed: a table of the shown price indexed by piece id (the divisor was wrong), the armor stats
record's bytes 1 to 5 (they identify the model, not the price), the recipe materials (one set shares a price across different
recipes), and a scan of live memory with the forge list open (the price is only a half-value in the table and is formatted when
drawn, so the shown number was never in memory as a number).

`prices-add <game dir> <ledger> <create|upgrade> <cost> <exact piece name>` adds a price read off the game's screen by hand.

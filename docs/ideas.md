# Ideas

Things the app does **not** do yet. "Needs" says what is missing before one can be built. Nothing here is promised.

Keep this file honest: when an idea is built, **delete it from here** in the same commit (and put what was learned in the README,
`formats.md` or `architecture.md`). When part of an idea is built, cut the idea down to what is left. New ideas go in the group they
belong to.

## Build manager

The Builds tab searches armor sets for wanted skills and keeps build templates (see the README). Left to do:

- **More filters:** only pieces you can craft now, a rarity cap.
- **More ways to rank** the sets: by resistances, by spare gem slots, "pieces I own first". Today it is by base defense, then by how
  many pieces you own.
- **Swap a template's piece from the search results** (today the swap list is every piece of that slot, owned first).

## Where to get things

- **Smarter hunt plans.** The Hunt plan estimates runs, but the rolls per run (3 body carves, 2 capture rolls, 3 rolls of a quest's main
  box...) are assumptions to check in play, and it does not yet pick between a quick plan and a short one (fewest steps vs fewest runs). It
  also cannot limit itself to the ranks the hunter has reached (**Needs:** the save's hunter rank found, and a quest's rank, which is not
  decoded).
- **More from the quest files.** The Quests tab shows the goal, client, time limit, monsters and rewards, and whether it is a village or a
  hall quest. Not decoded: whether a map is day or night (the game's makers chose it as flavor,
  and no byte tells them apart), the two Elder Dragon places (maps 14 and 15) and what the save's points total at 0x5a46 counts (it also
  rises with village quests, so it is not the hall's hunter rank points alone).
- **Gathering spots and shop stock.** **Needs:** the gather lists (a table of pointers sits just before the capture and break lists in
  the executable's data) and the shop tables decoded.

## Data not shown yet

- **Part-break names from the game.** `breakparts.rs` is a hand-made table for 43 monsters (see `docs/formats.md`); the rest, and a few whose
  high and G rank lists do not line up (Brachydios, Alatreon, Abyssal Lagiacrus, Glacial Agnaktor, Green and Lucent Nargacuga, Silver
  Rathalos), are numbered. The game's own source for the parts was searched for and not found: the monster archive's `em_status00` and
  `em_hitdata00` (the hit areas carry an index, not the kind of part), and the executable's data and read-only sections (no table of the
  kinds in a monster's order). **Needs:** the table that says which parts a monster can break, probably in code.
- Monster **hit points** and the zones of other states (enraged, broken); `em_status00` has no names, and the zone names shown come from Kiranico (33 rows have none).
- Palico (Felyne) equipment, and the rest of the guild card: **hunter rank** (a byte at 0x7569 may be it; to confirm at the next rank-up), weapon usage (the count for the great sword shows 11; not found as a plain count) and the title.
- **Which shop sells what.** The Items tab shows an item's shop price and carry limit (from Kiranico), but not where it is sold or when it appears. **Needs:** the shop tables.
- How the **special pieces** are unlocked (Yukumo armor and a few earrings have recipe tier 0 and do not follow the hunt rule).
- Other languages: the text archive also has French, German, Italian and Spanish names and descriptions; a language option would only
  need the archive path to change.

## Quality of life

- More Equipment sorts (by skill points) and search on the Worn tab (it has no list). Wishlist sorting and a way to mark a piece "done" without owning it.
- Config file for colors and icons (the anvil, the muted gray) beside the wishlist.
- Mouse support for the lists.
- The zenny cost of upgrading armor, if the game has one (not found).

## Live mode

- Find where the game keeps the blacksmith's list. `:scan` (see `docs/live.md`) searched the game's memory for it in four rounds (piece ids
  in and out of order, flags per piece or per recipe row, bytes to dwords and packed bits) with the real menus of two hunters, and found no flag
  array that follows the menu. The list may be rebuilt from other data each time. A clean test would settle what unlocks a piece: a fresh
  hunter, one action at a time (a Kelbi, a quest, a large monster) and a look at the menu after each. The rule is wrong for one case: a
  hunter with Kelbi x6 and Altaroth x3 does not get Bone Helm, while another with similar small hunts plus Ludroth, Epioth and Bnahabra does.
- Tell when a quest ends, so that pickups can be grouped by quest instead of by pauses of 20 seconds (no quest state is known in the live
  block), and log the quest's name and the zenny it paid.
- Search for sets off the UI thread, if a very large pool ever makes the Builds tab slow (it takes a few milliseconds today).

## Checks that need the game

Not features, but they would settle open questions in the notes. Each takes a few minutes in front of the game.

- **Blacksmith rule at high rank and G rank.** The hunt rule was seen on one hunter in low rank. A hunter in high rank (6 stars and
  up, solo) should show whether the S sets need only a hunt in that rank, and G rank (after the Hunter Rank 5 urgent quest "Throne of
  the Abyss") whether it opens the X sets with Jaggi X, Wroggi X, Volvidon X, Rathalos X and Brachydios X. Compare the app's `b` filter
  with the blacksmith's list, for weapons too.
- **Armor prices that differ** between the game's table and a published list: the Qurupeco set, Yukumo, Rathian X, Rath Heart Z and
  Silhouette Casque (about 24 pieces). Read the price off the forge list; a price recorded that way already wins over the table.
- **Weapon attack numbers.** The decoded attack matches a published list for about 97% of weapons but was not compared with the game's
  own screens. A handful of weapons across types would do (the details are in `docs/formats.md`).
- **Torso Up** with a real set: does the body piece's own Torso Up count double? The Worn tab and the Builds search assume not.

## Project

- Screen-drawing tests with ratatui's `TestBackend`, so a layout change that breaks a tab is caught without running the app (today
  `scripts/screenshots.sh` does this by hand).
- Release builds for Linux attached to a GitHub release; a `cargo install` line in the README.

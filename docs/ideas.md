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
- **Skills that suit a weapon.** The Builds tab takes a weapon and filters by its armor class, but does not suggest skills for it (a
  sharpness skill for a sword, reload speed for a bowgun). **Needs:** the skill effect tables decoded, to know what each skill does
  for which weapon type.
- **Swap a template's piece from the search results** (today the swap list is every piece of that slot, owned first).
- **Skill levels above the first** (15, 20 points) and their effect names. **Needs:** the skill effect tables decoded.
- **Decorations (jewels)**: count the gems in the slots. **Needs:** decorations decoded, and a way to know what is socketed.
- **A second talisman skill and talisman slots.** **Needs:** a talisman with two skills and one with slots in a save, to tell the
  record's fields apart (only a one-skill Pawn Talisman has been seen).

## Where to get things

- **Smarter hunt plans.** The Hunt plan picks the monsters and quests that cover the most missing materials, but not yet: how many runs
  a material is likely to take (from the chances and the number of carves and reward rolls), or limiting the plan to the ranks the
  hunter has reached (**Needs:** the save's hunter rank found, and a quest's rank, which is not decoded).
- **More from the quest files.** The Quests tab shows the goal, client, time limit, monsters and rewards. Not decoded: the stage (map), the
  zenny reward and fees, the small monsters and the quest's rank (low, high or G) and where it is given (village or hub).
- **Gathering spots and shop stock.** **Needs:** the gather lists (a table of pointers sits just before the capture and break lists in
  the executable's data) and the shop tables decoded.

## Data not shown yet

- Weapon **sharpness and element** (not found in the executable's data tables).
- **Armor maximum defense** (six growth bytes decide it; no formula yet).
- Which body part each **part-break** list is.
- Skill **effect names and higher tiers**.
- Monster weaknesses, hit points and parts (probably in each monster's archive; nothing decoded).
- Palico (Felyne) equipment and the guild card, quest progress and play time in the save.
- **Item values:** sell and buy price, rarity and carry limit of each item (a published list has them; the game's table is not found).
- **Hunter's Notes** (the monster descriptions in the text archive): show them on the Monsters tab. **Needs:** the mapping from a note to
  a monster, which is not the monster id order.
- How the **special pieces** are unlocked (Yukumo armor and a few earrings have recipe tier 0 and do not follow the hunt rule).
- Other languages: the text archive also has French, German, Italian and Spanish names and descriptions; a language option would only
  need the archive path to change.

## Quality of life

- Search on the Equipment, Monsters and Worn tabs; `/` for monsters by name or by drop.
- More Equipment sorts: by attack, by defense, by skill points.
- Show a weapon's sharpness as the colored bar the game uses, once the data is found (and add sharpness and element to the weapon comparison).
- Wishlist sorting and a way to mark a piece "done" without owning it.
- Export the shopping list as text to paste into a note.
- A "what changed since last time" summary on startup (items gained, zenny change, new equipment), from a saved copy of the last save
  the app saw.
- Config file for colors and icons (the anvil, the muted gray) beside the wishlist.
- Mouse support for the lists.
- Show maximum defense next to base defense once it is decoded, and the zenny cost of upgrading armor if it turns out to have one.

## Live mode

- Notify when a wishlist piece becomes craftable (the materials just arrived).
- Show the quest's drops as they land, matched against the wishlist.
- Record each blacksmith visit's list (the unlock rule could then be checked against what the game shows).
- Notice when a quest ends and log what it gave (items, zenny), to build a personal history of drops that could be compared with the
  published chances.
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

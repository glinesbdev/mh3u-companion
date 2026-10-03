# Ideas

Things the app could do, roughly grouped. "Needs" says what is missing before it can be built. Nothing here is promised; add to the
list as ideas turn up.

## Build manager

Say which skills you want (for example Attack Up (S), Perception and a gem slot) and get the armor pieces to wear.

- Pick skills and the points you want; search head, body, arms, waist and legs (and a talisman) for sets that reach them.
- Filters: only pieces you own, only pieces you can craft now, only pieces the blacksmith is offering (the `b` rule), a rarity cap,
  blademaster or gunner, male or female.
- Rank by total defense, resistances, spare gem slots; show the worn-gear totals (the Worn tab's maths) for each result, and let any
  result be saved as a template (below) or put on the wishlist.
- Torso Up is part of the maths (it doubles the body piece), so the search has to try body pieces last.
- **Build templates, saved between sessions.** A template is a named full set: weapon (optional), head, body, arms, waist, legs and a
  talisman. Templates are kept in a file beside the wishlist, so they survive restarts, and you can keep as many as you like
  ("Attack Up for Rathian", "Fire resist for Lagiacrus", ...).
  - **Mix and match:** every slot of a template can be swapped for another piece (from the search results, the owned gear or the
    full list) and the totals update as you go: defense, resistances, gem slots and skill points, as on the Worn tab.
  - **To the wishlist, piece by piece or whole:** add any single piece of a template to the wishlist, or add the whole template in one
    go. The shopping list then covers what is missing for exactly that set. Pieces you already own are left out of the totals, as
    the wishlist does now, and removing the template's pieces again should not remove pieces that other wishlist entries still need.
  - A template remembers a piece by its kind and id (not its position), so it keeps working after the game's data is reloaded.
  - A template can also start from what you are wearing now ("save my current set as a template").
- **Needs:** talismans (no save has one yet to decode the equipment record from), and decorations or jewels (not decoded) if sets
  should count gems. Templates themselves can be built before that, as armor-only sets. Skill tiers above 10 points (15, 20) and their effect names are not decoded; the search can target the
  first tier only until they are. A reference tool for this exists (an armor set search for MH3U); its data has no prices but it is a
  good model for the search.

## Where to get things

- **Farming plan:** for the wishlist's missing materials, list the monsters that drop them, with the best chance, rank and kind of
  drop, and group them so one hunt covers several items. The Monsters tab already stars what the wishlist needs; this would go
  further and say "hunt X in high rank for 4 of your 7 missing materials".
- **Quest finder:** which quests have a monster (or a reward). **Needs:** the `.quest` files decoded (QTDS: text, then binary data;
  monsters, rewards and the quest rank are in the binary part).
- Quests are spread over several folders (`quest/us`, `quest/btl`, `quest/support` and `DLC/us`); the quest finder should cover them
  all and say which come from downloadable content.
- **Gathering spots and shop stock.** **Needs:** the gather lists (a table of pointers sits just before the capture and break lists in
  the executable's data) and the shop tables decoded.

## Planning

- **Cheapest upgrade path** to a weapon: from what you own, the chain of upgrades and what each costs in zenny and materials, against
  making the weapon from scratch.
- **Set families:** group armor by set (Agnaktor Cap, S, U, X, Z) with a "which variant do I have" view. The game has no upgrade chain
  for armor, so this is by name.
- **Surplus finder:** items in the box that no wishlist piece and no remaining recipe needs, as candidates to sell.
- **Zenny goal:** how much to earn before the wishlist is affordable, and which cheap pieces to make first.
- **Affordable now:** a Crafting filter (like `c` for materials) for pieces whose zenny cost you can pay, and a sort by cost.
- **Weapon comparison:** two or three weapons side by side (attack, affinity, slots, rarity, and sharpness and element once found).
- **Skill browser:** pick a skill and see every armor piece that has it, strongest first, with a mark for the ones you own or the
  blacksmith is offering. (Search already finds a skill by name; this would be a view of its own and the first step of the build
  manager's search.)
- **Hunter picker:** the wishlist is now per save slot (chosen with `--slot`), like the remembered blacksmith list. Build templates should
  follow the same rule, and an in-app way to pick the hunter would replace `--slot`.

## Data not shown yet

- Weapon **sharpness and element** (not found in the executable's data tables).
- **Armor maximum defense** (six growth bytes decide it; no formula yet).
- Which body part each **part-break** list is.
- **Talisman** skills and slots, and **decorations**.
- Skill **effect names and higher tiers**.
- Monster weaknesses, hit points and parts (probably in each monster's archive; nothing decoded).
- Palico (Felyne) equipment and the guild card, quest progress and play time in the save.
- **Item values:** sell and buy price, rarity and carry limit of each item (a published list has them; the game's table is not found).
- **Hunter's Notes** (the monster descriptions in the text archive): show them on the Monsters tab. **Needs:** the mapping from a note to
  a monster, which is not the monster id order.
- How the **special pieces** are unlocked (Yukumo armor and a few earrings have recipe tier 0 and do not follow the first-material rule).
- Other languages: the text archive also has French, German, Italian and Spanish names and descriptions; a language option would only
  need the archive path to change.

## Quality of life

- Search on the Equipment, Monsters and Worn tabs; `/` for monsters by name or by drop.
- More Equipment sorts: by attack, by defense, by skill points.
- Show a weapon's sharpness as the colored bar the game uses, once the data is found.
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
- Record each blacksmith visit's list (the unlock check could then be verified against what the game shows, not inferred).
- Notice when a quest ends and log what it gave (items, zenny), to build a personal history of drops that could be compared with the
  published chances.

## Checks that need the game

Not features, but they would settle open questions in the notes. Each takes a few minutes in front of the game.

- **Blacksmith rule for weapons and late game.** The first-material rule was checked on one early save, for armor only. Compare the
  app's `b` filter with the blacksmith's list on a save with more progress, and for weapons.
- **Armor prices that differ** between the game's table and a published list: the Qurupeco set, Yukumo, Rathian X, Rath Heart Z and
  Silhouette Casque (about 24 pieces). Read the price off the forge list; a price recorded that way already wins over the table.
- **Weapon attack numbers.** The decoded attack matches a published list for about 97% of weapons but was not compared with the game's
  own screens. A handful of weapons across types would do (the details are in `docs/formats.md`).
- **Torso Up** with a real set: does the body piece's own Torso Up count double? The Worn tab assumes not.
- **Talisman.** Get one into the WornTester save so the equipment record's tail can be decoded (skills, slots); then a second with
  different skills to tell the fields apart.

## Project

- A GitHub Actions workflow running `cargo fmt --check`, `cargo clippy` and `cargo test` on every push (the tests that need game files
  skip themselves, so it can run without them).
- Screen-drawing tests with ratatui's `TestBackend`, so a layout change that breaks a tab is caught without running the app.
- Release builds for Linux attached to a GitHub release; a `cargo install` line in the README. The repository is private for now;
  making it public needs a look through the history and docs for anything personal first.
- Keep `scripts/screenshots.sh` in step with the tab order when tabs are added (its key paths depend on it).

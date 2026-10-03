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
- **Gathering spots and shop stock.** **Needs:** the gather lists (a table of pointers sits just before the capture and break lists in
  the executable's data) and the shop tables decoded.

## Planning

- **Cheapest upgrade path** to a weapon: from what you own, the chain of upgrades and what each costs in zenny and materials, against
  making the weapon from scratch.
- **Set families:** group armor by set (Agnaktor Cap, S, U, X, Z) with a "which variant do I have" view. The game has no upgrade chain
  for armor, so this is by name.
- **Surplus finder:** items in the box that no wishlist piece and no remaining recipe needs, as candidates to sell.
- **Zenny goal:** how much to earn before the wishlist is affordable, and which cheap pieces to make first.

## Data not shown yet

- Weapon **sharpness and element** (not found in the executable's data tables).
- **Armor maximum defense** (six growth bytes decide it; no formula yet).
- Which body part each **part-break** list is.
- **Talisman** skills and slots, and **decorations**.
- Skill **effect names and higher tiers**.
- Monster weaknesses, hit points and parts (probably in each monster's archive; nothing decoded).
- Palico (Felyne) equipment and the guild card, quest progress and play time in the save.

## Quality of life

- Search on the Monsters and Worn tabs; `/` for monsters by name or by drop.
- Wishlist sorting and a way to mark a piece "done" without owning it.
- Export the shopping list as text to paste into a note.
- A "what changed since last time" summary on startup (items gained, zenny change, new equipment), from a saved copy of the last save
  the app saw.
- Config file for colors and icons (the anvil, the muted gray) beside the wishlist.
- Mouse support for the lists.

## Live mode

- Notify when a wishlist piece becomes craftable (the materials just arrived).
- Show the quest's drops as they land, matched against the wishlist.
- Record each blacksmith visit's list (the unlock check could then be verified against what the game shows, not inferred).

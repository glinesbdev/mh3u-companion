# mh3u-companion

A terminal app for Monster Hunter 3 Ultimate (Wii U, played in Cemu). It reads your save file and your own game dump to show
what you own and what you need to craft or upgrade gear.

![Crafting: an armor piece with its stats, blacksmith status, cost and the materials you have](docs/screenshots/crafting-armor.svg)

<details>
<summary>More screenshots</summary>

![Wishlist with a shopping list for everything on it](docs/screenshots/wishlist.svg)

![The upgrade tree of a weapon](docs/screenshots/upgrade-tree.svg)

![Weapon details](docs/screenshots/crafting-weapon.svg)

![Equipment box, sorted by name](docs/screenshots/equipment.svg)

![Items](docs/screenshots/items.svg)

</details>

The pictures are drawn from the app's own output by `scripts/screenshots.sh` (the hunter name is replaced), using the colors of a
dark theme; your terminal's own palette applies when you run it.

## What it does

- **Items**: item pouch and item box, with names and quantities, and a details panel for the highlighted item: the game's description, how many you hold, what the wishlist needs of it, and which armor and weapons are made with it. The header shows your hunter name and zenny. `/` is a fuzzy
  search by item name over both lists (typos are fine: `hny` finds Honey).
- **Descriptions**: the details panels show the game's own description of the piece. `i` also shows what each skill does under it.
- **Equipment**: your equipment box, with the same details panel as the Crafting tab (stats, recipes and costs, and what a weapon upgrades into). The weapon and armor you are wearing are marked `●`. `s` cycles the sort: box order, name, rarity, type, worn first.
- **Crafting**: every armor piece and weapon that has a recipe, with have/need counts for each material (pouch and box together).
  - Armor shows rarity, gem slots, base defense, resistances and skills.
  - Weapons show rarity, gem slots, attack and affinity (read from the game files), a "Create from scratch" recipe and an "Upgrade from" recipe, and whether you own the parent weapon.
  - `/` is a fuzzy search over name, type, armor skill, materials, and the labels `male` / `female` / `blademaster` / `gunner`
    (a piece for both genders or both classes matches both). A number from 1 to 10 (or `r3`) filters by rarity, so `attack 3`
    finds armor with an Attack skill and rarity 3 (weapons have a rarity too, so they match as well).
    Several words must all match: `psychic head`, `female gunner 5`. Typos are tolerated (`rthlos mail` finds Rathalos Mail).
    Matches that aren't by name show why (`skill: Poison`, `needs: Iron Ore`). With a search active the list is ordered best
    match first: matches on a name, type or skill come first (grouped by slot), then the weaker ones (loose fuzzy matches, or
    pieces that merely need a material with that name), also grouped. `x` or `Esc` clears the search.
  - Armor details show rarity, slots, base defense, gender (Male / Female / Both) and type (Blademaster / Gunner / Both).
  - `c` shows only what you can make now. `o` hides pieces you already own. `b` shows only what the blacksmith is offering (see below), and an anvil marks those pieces in the list. `s` cycles the sort: game order, name, craftable
    first, owned first. Like pieces always stay together in the order head, body, arms, waist, legs, talisman, then the weapon
    types; the sort orders them within each group. (Craftable first and owned first split the list by that flag, then group.)
- **Blacksmith unlock**: the details panel says whether the blacksmith offers a piece. This is inferred, not read from the game: a piece is on offer once you hold 1 of the first material in its recipe (starting gear always is, and Yukumo-style special pieces follow some other rule). The game never takes a piece off the list, so the app remembers every piece it has seen on offer (per hunter, in `unlocked.tsv` next to the price ledger) and keeps showing it as on offer after you spend the material; a piece unlocked before you first ran the app, whose material you no longer hold, shows as not seen yet. On a save from the start of the game the rule predicted exactly the two pieces the blacksmith offered beyond the starting gear. It has not been checked for weapons or against a late-game save.
- **Wishlist**: press `w` on a piece in the Crafting tab to add it (marked ★). If it can only be obtained by upgrading and you
  don't own a parent weapon, the parents it needs are added with it, back to the first weapon you own or can make from scratch
  (pieces that can be made from scratch never pull in parents). Removing a piece also removes the parents that were added
  automatically for it, unless another wishlisted piece still needs them; parents you added yourself are kept. The Wishlist tab lists your pieces, with an anvil on those the blacksmith is offering that you don't own yet. On the right,
  the highlighted piece shows what it needs by itself (have / need), and below it is one running shopping list for everything
  on the wishlist (have / need / still missing). Pieces you already own are left out of the totals. For each
  piece it plans "create from scratch" whenever that is possible, because upgrading uses up the parent weapon, and plans an
  upgrade only for pieces that have no create recipe. The wishlist is saved in
  `~/.config/mh3u-companion/wishlist.txt` (or under `$XDG_CONFIG_HOME`).
- **Upgrade tree**: `t` on a weapon (in Crafting, Equipment or Wishlist) opens its tree: the line of weapons from the first one down to it, a note where other branches leave that line, and everything it can be upgraded into, each marked owned or not with rarity and attack. `↑`/`↓` scroll, `t` or `Esc` close. Armor has no upgrade line in the game data, so it has no tree.
- **Look**: green means you have it or can afford it, yellow partly, red missing, cyan marks focus and keys. Materials show a
  small bar (`███░░░ 3/5`), armor shows its rarity (`R5`), gem slots (`◆◆◇`), element-colored resistances and one skill per
  line, and a forging cost turns red with the shortfall when you can't pay it. The terminal's own color scheme is used; set
  `NO_COLOR=1` for plain text. The anvil is a Nerd Font glyph (Unicode has no anvil); `MH3U_ICONS=plain` draws a hammer and pick instead for terminals without a Nerd Font. Below 100 columns the panes stack instead of sitting side by side.
- **Sorting**: `s` on the Items tab sorts the item box by box order, name or quantity.
- `Home`/`End` (or `g`/`G`) jump to the top and bottom of a list. `?` shows a key reference.
- The screen reloads on its own whenever the game writes the save.

Not shown yet: sharpness and element for weapons, armor max defense. See `docs/formats.md`.

## Live mode

`mh3u-tui --live` starts Cemu itself and shows the game's data as it changes, before you save: move an item and the screen
follows. Close any running Cemu first, then run the TUI and load your hunter in the game window. The header shows
`● live` once connected. Any change in zenny (shops, NPCs, quest rewards or fees) shows next to the total in the header
as `▲ +1,200` or `▼ -300` for 15 seconds, adding up if several happen close together, and in the status line. See `docs/live.md` for how it works and why the TUI must be the one to start Cemu.

Forging costs for weapons (create and upgrade) and armor are read straight from the game files and always shown, unless you have seen a different price in play, which then wins. In live mode, crafting a piece also teaches the app what it cost (and, for the few pieces the game data has no recipe for, what it needs): the zenny drop is matched against the piece's recipe and kept in
`~/.local/share/mh3u-companion/prices.tsv`, then shown beside the recipe and totalled on the Wishlist tab. See `docs/prices.md`.

`mh3u-tui --live --debug-edit` adds a debug command line (`:`) that changes the running game's zenny and item box (`zenny 50000`,
`give iron ore`, `stock` to cover the wishlist) for testing without hours of play. This is the only part that writes to the game;
it backs up your save slots first, and anything you then save in the game keeps the edits. See `docs/live.md`.

## Running

```
cargo run --release -p mh3u-tui
```

It looks for the game dump under `~/games/wiiu/` (a folder whose name contains `[Game] [0005000010118300]`) and the save at
`~/.local/share/Cemu/mlc01/usr/save/00050000/10118300/user/80000001/user1`. Override either:

```
mh3u-tui --game-dir "/path/to/MONSTER HUNTER 3 ULTIMATE [Game] [0005000010118300]" --save /path/to/user1
```

or set `MH3U_GAME_DIR` and `MH3U_SAVE`. The game has three save slots (`user1`, `user2`, `user3`); `--slot 2` picks the second
one in the default Cemu folder. Press `?` in the app for the keys.

The game data is read from **your own dump** at runtime. No game data is stored in this repository.

Tested only with the US version (update v32) of the game. The recipe and stats tables are located by fixed offsets in the game
executable, which are checked on load; a different version will fail with an "unsupported executable" error instead of showing
wrong data.

## Layout

- `crates/mh3u-core`: parsers (save, `.arc` archives, `.gmd` text, `.rpx` executable, recipes, armor stats) and `GameData`.
- `crates/mh3u-tui`: the terminal app.
- `crates/mh3u-tools`: developer commands used to reverse-engineer the formats: `savediff`, `items`, `recipe`, `arcls`, `arcx`,
  `gmd`, `arcsearch`, `prices-add`, `prices-hint`, `armor-todo`, `weapon-names`, `unlock-guess`, `ansi2svg`, `cemu-host` (starts Cemu and answers memory queries from a file; used to find where the data lives).
- `scripts/screenshots.sh`: regenerates `docs/screenshots/*.svg` by running the app in tmux in a sandbox (needs a save and the release build; `mh3u-tools ansi2svg` draws the pictures).
- `docs/formats.md`: what is known about each file format, and how confident that knowledge is.
- `docs/live.md`: how live mode finds and reads the game's data.
- `docs/prices.md`: the forging-cost ledger and the search for where the game stores prices.
- `snapshots/`: copies of save files taken during development. These are personal save data and are git-ignored; some tests read them and are skipped when they're missing.

## Development

```
cargo test
cargo clippy --all-targets
cargo fmt
```

## License

Public domain under the [Unlicense](LICENSE): use it however you like, no attribution needed. This covers the code and docs only.

Monster Hunter 3 Ultimate and all of its content (items, recipes, text, artwork) are the property of Capcom, who hold all rights to it. This is an unofficial fan tool, not affiliated with or endorsed by Capcom, Nintendo or Cemu. It reads data from your own copy of the game and includes none of it.

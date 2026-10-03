# How the code is organised

Three crates in one workspace. The rule that keeps it tidy: **`mh3u-core` knows nothing about the screen, and the screen knows
nothing about byte offsets.**

## `mh3u-core`: the game's data

Everything that reads a file or the running game. No terminal code.

| Module | What it does |
|---|---|
| `save` | The save file (and the live copy of it in memory): pouch, item box, equipment box, worn gear, hunt counts. |
| `arc`, `gmd`, `rpx` | The game's archive, text-table and executable formats. |
| `gamedata` | Loads a game dump once and answers questions about it: names, descriptions, recipes, stats, drops. Everything else asks `GameData`. |
| `recipes`, `armor`, `weapons`, `drops` | The tables found in the executable's data section (offsets are for the US v32 build and are checked on load). `drops` also indexes the lists by item and by monster. |
| `blacksmith` | The rule for which pieces the blacksmith offers (a monster that drops the piece's first material has been hunted). |
| `prices` | The forging-cost ledger and the tracker that learns costs from play. |
| `live`, `livesave`, `procmem`, `edit` | Live mode: find the save in Cemu's memory, follow it, and (debug only) write to it. |

## `mh3u-tui`: the app

`main.rs` parses arguments (clap), opens the files and starts the loop. `App` (in `app/`) holds all state; `ui/` draws it. Neither
knows how the other works beyond `App`'s public fields and methods.

```
app/
  mod.rs            the App struct (a handful of fields, one struct per tab: Inventory, Crafting, WishList, MonsterTab,
                    BuildManager, PriceBook, EditConsole), App::new, the main loop, moving in lists
  keys.rs           all key handling: popups first, then the tab's keys, then the shared ones
  crafting.rs       Crafting tab: pieces, recipes, plans, search and sort, costs
  inventory.rs      Items and Equipment tabs, worn gear, upgrade-tree popup
  monsters.rs       Monsters tab
  wishlist.rs       the wishlist, parent weapons, the shopping list
  blacksmith.rs     what is on offer, and the pieces remembered as seen on offer
  build_manager.rs  Builds tab: BuildManager (its state), templates, the popups that edit them
  live.rs           following the running game, reloading the save, the debug command line
  price_watch.rs    watching for forging costs
  money.rs, sorting.rs   small shared types
ui/
  mod.rs            the frame (tabs, footer) and helpers every tab shares
  one file per tab, plus pieces.rs (what is said about a piece), help.rs, tree.rs
```

Pure logic with no `App` in sight lives beside them so it can be tested alone: `builds` (the skill search), `templates`
(saved sets and their file format), `worn` (totals for a set), `tree` (the weapon upgrade tree), `search` (fuzzy matching),
`unlocked` and `files` (what is kept on disk, and where).

### Adding a tab

1. Add a variant to `Tab` (`app/mod.rs`) and to `Tab::ALL` and `title`.
2. Put its state in a struct and its methods in `app/<name>.rs` (`impl App { ... }`); add `mod <name>;` in `app/mod.rs` and a field of the struct on `App`.
3. Its keys go in a `<name>_key` method that `tab_key` in `app/keys.rs` calls.
4. Its drawing goes in `ui/<name>.rs`; add the arm in `ui::draw`, key hints in `draw_footer`, and a line in `ui/help.rs`.

### Adding something that is kept between sessions

Add the path to `files::Files` (so every file is named in one place, per hunter if it should be), and save with `files::save`,
which creates the folder and words the error for the status line.

## `mh3u-tools`: developer commands

`mh3u-tools --help` lists them. They exist to explore the formats and to check findings (`unlock-monsters`, `drops`, `recipe`...);
the app does not depend on them. Findings are written up in `formats.md` with how sure they are.

## Tests

- Unit tests sit next to the code. Tests that need the game dump or a personal save skip themselves when it is missing
  (`fixture!` for saves under the git-ignored `snapshots/`).
- A change that should not alter what is drawn can be checked by re-running `scripts/screenshots.sh` and seeing that
  `docs/screenshots/*.svg` did not change.

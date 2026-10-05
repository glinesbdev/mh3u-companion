# Keeping the code clean

How to add to this app without letting it get tangled. `architecture.md` says where things are; this says how to behave while
changing them. When a rule here stops being true, change the rule in the same commit.

## The few rules that matter most

1. **`mh3u-core` never draws; `mh3u-app` never knows a screen; a screen never knows a byte offset.** Anything that reads a file or the
   game's memory belongs in core and comes out as a typed value (`ArmorStats`, `Source`, `Unlock`). The app asks `GameData`; it does
   not parse. `mh3u-app` must not depend on ratatui or crossterm (check with `cargo tree -p mh3u-app`): it has its own `Key` and
   `ListState`, and what the screen needs it reads from `App`'s public fields.
2. **Pure logic first, screen second.** A rule (the blacksmith unlock, the build search, a sort, a file format) is a function that
   takes plain values and returns plain values, with tests. `App` calls it and `ui` shows the result. If you are writing an `if` in
   a draw function that decides something about the game, it belongs in a function that is not a draw function.
3. **One struct of state per tab.** `App` holds a handful of fields; each tab's state lives in its own struct next to its methods
   (`Inventory`, `Crafting`, `WishList`, `BuildManager`, ...). A new tab brings a new struct, not ten new fields on `App`.
4. **Nothing heavy per frame.** The screen redraws several times a second. Work that scans the game's tables is done when the thing it
   depends on changes (`refresh_*`), and the result is stored. Look things up through the indexes (`Drops::sources`,
   `GameData::piece_name`), not by scanning.
5. **Say what is known and what is guessed.** A rule worked out from play is written down with how it was checked (`formats.md`),
   and the screen words it the same way ("not seen on offer yet", not "locked"). Never present an inference as the game's data.

## Where code goes

| Doing this | Put it in |
|---|---|
| Reading a new table from the executable | `mh3u-core/src/<table>.rs`, loaded in `GameData::load`, offsets checked there |
| A game rule | `mh3u-core` (see `blacksmith.rs`), with a test on a real save if one exists |
| A new tab | see "Adding a tab" in `architecture.md` |
| A key | the tab's `*_key` method in `mh3u-app/src/app/keys.rs`; popups get keys before tabs do |
| Something kept between sessions | a path in `files::Files`, saved with `files::save`; a small text format with `parse` and `format` and a round-trip test |
| A colour, symbol or shared widget | `theme.rs`, so the screen keeps one visual language |
| A command-line option | the `Cli` struct (clap); an explorer command goes in `mh3u-tools` |

## Words

Use the same word for the same thing, in the code, the docs and on screen.

- **Not "farm" or "farming".** The game has a farm (the village's), so a feature about getting materials is a **hunt plan**, a monster
  is **hunted**, a drop comes from a **carve**, a **capture** or a **part break**.
- **"On offer"** is what the blacksmith lists; **"owned"** is in the equipment box; **"seen on offer"** is what the app remembers
  (the blacksmith never takes a piece off). Do not use "unlocked" for a piece, the app only infers it.
- A **slot** is a save slot (hunter) on the command line and a **gear slot** (head, body...) in a set. Say which when it could be
  either; in code `slot` is the save slot and `SLOTS` in `templates` are the gear slots.
- **Template** is a saved set; **set** is a found or worn set of armor; **build** is the feature. A **family** is the armor named
  alike (Agnaktor...) with its variants; do not call that a "set", which would clash with the Builds tab.

## Size and shape

These are limits to notice, not to game. If you hit one, split by meaning, not by line count.

- A function that does not fit on a screen (about 100 lines) is two or three functions with names. Clippy's `too_many_lines` is a good
  nudge: `cargo clippy --all-targets -- -W clippy::too_many_lines`.
- A file past about 600 lines wants to become two. Moving code is cheap here: it is `impl App` blocks and free functions, so a split
  is a cut and a `mod` line.
- No magic numbers: a table offset, a limit or a stride gets a named constant with a comment saying what it is and where it was found.
- Prefer an enum to a string or a pair of booleans (`Pool`, `Availability`, `BuildFocus`). Prefer a struct to a tuple once it has more
  than two fields or crosses a module (`Source`, `Found`).
- Take `&T`, not `T`, when you do not keep it; return iterators or slices rather than fresh `Vec`s when the caller only loops.
- Do not repeat yourself a third time. The second copy is a warning; the third is a function (`files::save`, `ArmorStats::talisman`,
  `GameData::piece_ids`).

## Errors and panics

- Anything the user can cause (a missing file, a save from another version, a typo) gives an error worded for the status line or
  the terminal, never a panic. Use `anyhow` with context (`with_context`) at the edges.
- `unwrap` and `expect` are for tests and for facts the code has just established. A byte-offset parser checks the length once, up
  front, and says "unsupported executable" rather than slicing blindly.
- A failed save of a list is shown (`could not save wishlist: ...`) and the app carries on.

## Tests

- Put a test beside the code it covers. A bug fix starts with a test that fails.
- Test the rule, not the drawing: build the input by hand, assert on the output. Real-data tests are welcome for decoded tables, and
  they must **skip themselves** when the game dump or a personal save is missing (`fixture!`, or return early). CI has neither. A test finds
  the dump through `MH3U_GAME_DIR` (`mh3u_core::gamedata::dump_from_env`); `scripts/check.sh` sets it from your settings' `game_dir`.
- Files the program reads can be damaged or made by somebody else (a dump, a save, a settings profile): read numbers with `get` and
  checked arithmetic, never trust a size in a file to set aside memory (see `inflate`), and give an error, never a panic. A new
  parser gets a case in `mh3u-core/src/hostile_tests.rs`. The workspace forbids `unsafe`. `scripts/check.sh` runs `cargo audit` (before pushing, if `cargo-audit` is installed); CI always does. See `SECURITY.md`.
- A new user setting is one entry in `config::DEFS` (key, label, kind, default, help): the config file's commented template, the parser, the checks and the Settings screen (`S`) all come from that table. Say in its `when` whether it applies now or at the next start.
- Whole-app behaviour (keys in, state out) goes in `app/flow_tests.rs`, with its files in a temporary folder. What is drawn is checked in
  `mh3u-tui/src/ui/screen_tests.rs`, which draws every tab into ratatui's test backend at several terminal sizes (a panic or a missing pane fails it).
- A change that must not alter what is drawn is checked by running `scripts/screenshots.sh` and seeing that `docs/screenshots/*.svg`
  did not change (build release first).
- No real hunter names, no personal save data and no copied third-party tables in the repository, in tests or in docs. `snapshots/`
  is git-ignored for a reason. Published data may be used to check a decode in a test, never copied in.

## Dependencies

Add a crate when it replaces code we would otherwise have to keep correct (argument parsing, platform folders), not for convenience.
Say why in the commit. Currently: `anyhow`, `clap`, `dirs`, `flate2`, `memchr`, `ratatui` (only in `mh3u-tui`).

## Docs are part of the change

A feature is not done until:

- the README says what it does in a line or two, in the user's words, and `docs/features.md` has the details (and the screenshot is refreshed if a tab changed);
- `docs/ideas.md` no longer lists it. **Ideas that are built are deleted from ideas.md**; ones half built are cut down to what is
  left; new ideas go in;
- `docs/formats.md` records anything newly decoded, with how sure we are and how it was checked;
- the in-app help (`ui/help.rs`) lists its keys, and the footer hints are right;
- `docs/architecture.md` still describes the folders.

## Before every commit

```
scripts/check.sh                                   # fmt --check, clippy -D warnings, tests: what CI runs, stopping at the first failure
cargo build --release && scripts/screenshots.sh   # when anything drawn could have changed
```

Do not pipe the checks through `tail` or `grep` and read the last line: that hides a failing exit status. CI runs `scripts/check.sh`'s
three commands on every push. One purpose per commit, with a message that says why; refactors and behaviour changes go in
separate commits so a "pure move" can be trusted to be one. Pushing is the owner's call.

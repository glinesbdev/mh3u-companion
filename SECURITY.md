# Security

MH3U Companion is a local, single-user terminal program. It reads your own game dump and save files, and with `--live` it reads (and,
in a build with the `edit` feature and `--debug-edit`, writes) the memory of a Cemu it started itself. It has no network code, runs no shell, and contains no `unsafe` code
(`unsafe_code = "forbid"` in the workspace).

## What it is built to handle

- **Files that are damaged or made by somebody else.** A game dump, a save, a quest file, an archive or a settings file can be cut short or
  scrambled; the parsers check every number they read and give an error, never a panic or an unbounded allocation. Compressed data is
  not unpacked past the size the file declares (and never past 512 MiB). `crates/mh3u-core/src/hostile_tests.rs` feeds them made-up,
  truncated and scrambled files. Text from a save (names, greetings, quest titles) cannot send control codes to your terminal: they are
  stripped when drawn.
- **Writes to the game.** Only in a build with the `edit` feature and with `--debug-edit` (the default build has no code that writes to the game's memory), only to the Cemu process the program started, and only inside the save block (a write
  past it is refused). It will not start in edit mode while Cemu has online play turned on.
- **Dependencies.** Few, pinned by `Cargo.lock`, checked against the RustSec advisory database (`cargo audit`) in CI and in `scripts/check.sh`, and kept
  current by Dependabot. CI runs with read-only permissions and its actions are pinned to commits.

## What it does not protect against

- Somebody who changes this open source program, or edits a save without it. The online guard for debug edits is a safeguard for
  honest users, not a way to stop cheating.
- A settings profile from a stranger: the `cemu` setting is a program the app starts (without a shell), so read a profile before using it,
  as you would a script.

## Reporting a problem

Please use GitHub's private vulnerability reporting (the repository's Security tab) if it is available, otherwise open an issue that
does not include a working exploit. There is no bounty; I will fix what is reported as soon as I can.

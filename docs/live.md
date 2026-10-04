# Live mode

`mh3u-tui --live` starts Cemu on the game itself and reads the game's data straight out of Cemu's memory, so the screen
follows what happens in the game before anything is saved: moving items, picking things up, changing gear.

```
mh3u-tui --live [--cemu /path/to/Cemu]
```

Close any Cemu that is already running first. The TUI shows `◌ waiting for the game` until a hunter is loaded, then
`● live`. If the game drops the hunter (back to the title screen, or Cemu closing) the TUI reloads the save file right away, so
anything that was never saved disappears from the screen, the same as in the game, and the status line says so.

## Why the TUI has to start Cemu

The data is read through `/proc/<pid>/mem`. The kernel only lets a process do that for its own descendants, unless the
reader is privileged. This is the Yama `ptrace_scope` setting; on Arch it is `1`:

```
cat /proc/sys/kernel/yama/ptrace_scope
```

Starting Cemu from the TUI needs no extra privileges and changes no system setting. Reading is strictly read-only; nothing is
written to the game unless you ask for it with `--debug-edit` (below). The alternatives are `sudo sysctl -w kernel.yama.ptrace_scope=0` (weakens a system-wide default)
or `setcap cap_sys_ptrace=ep` on the binary (lost on every rebuild); neither is needed.

Consequence: if you quit the TUI, Cemu keeps running but can no longer be read, and a new TUI cannot attach to it. Quitting
asks for confirmation while Cemu is running. Cemu is started in its own process group, so Ctrl-C in the TUI or closing the
terminal does not take the game down.

## How the data is found

While a hunter is loaded the game holds three copies of its save data in memory, each laid out like the `user1` file (Cemu
keeps guest memory in the guest's byte order, so `Save::parse` reads it unchanged):

| Copy | Header | Behaviour |
|------|--------|-----------|
| as loaded | blank | never changes while playing |
| UI state | blank | has the same layout but is partly unrelated runtime data |
| **live** | self-pointers | updated immediately, e.g. moving 3 Honey from the box to the pouch changed exactly three bytes |

The live copy's header (the first `0x28` bytes of the file are the checksummed file header and are not present in memory)
has a guest pointer `P` at offset `0x08` and `P + 0x34` at `0x24`, and the guest memory base derived from it
(`block - 0x0c - P`) is 64 KiB aligned. That is how it is told apart from the other two copies.

To find it, a background thread searches Cemu's writable memory for a hunter name from the save slots + `00`
(the three bytes before the name differ per hunter, so they are not part of the search; about a second per search, repeated every few seconds until found), then reads the 35,364-byte block four times a second and
sends it to the app whenever it changes. While live data is flowing, changes to the save file are ignored.

Only the hunter currently loaded in the game is live. The `userN` files beside the save you opened with `--slot` / `--save` give the
names to search for. When the live hunter is not the one on screen, the app works out which slot it is by name and switches to that
slot's data: the wishlist, the Builds skills and templates (`wishlist-2.txt` and so on) and the save file used as the fallback when
the hunter is unloaded. The price tracker starts afresh so that one hunter's inventory is never compared with another's. So `--slot`
only decides what is shown until the game loads a hunter. If the hunter's name is in no slot file, nothing switches.

## Pickups and notices

Each live update is compared with the one before. Items the pouch and box hold more of than before are a pickup, and are written to the
Pickups tab and to `gains.tsv` (one file per hunter, in the data folder; see `src/gains.rs`). The first update after connecting, after the
hunter is unloaded or after the game loads another hunter is only a starting point, so nothing already in the save counts. Moving items
between pouch and box nets to zero. The status line announces a pickup (★ for what the wishlist is short of) and a wishlisted piece that
has become craftable. The game's data holds no quest state, so pickups are grouped by time (20 seconds), not by quest.

## Debug editing (`--debug-edit`)

```
mh3u-tui --live --debug-edit
```

For testing: a game takes a long time to play, so this lets you set up a situation instead. It writes into the game's live
copy of the save data, and nothing else; it is off unless you pass the flag, and the header shows a red `✎ EDIT`. Press `:` to
type a command:

| Command | Effect |
|---------|--------|
| `zenny 50000` | set the wallet (also `zenny +500`, `zenny -200`); limited to 999,999 |
| `give iron ore` | fill that item's stack in the item box to 99. The name is matched as a whole: an exact name wins (`monster bone s` is Monster Bone S), then a name that starts with or contains what you typed (shortest first), and only then word by word with typo tolerance. If several names tie, the status line lists the others it could have meant |
| `give honey 5` | add 5 (a stack holds 99) |
| `set honey 5` | set exactly 5; `set honey 0` removes it |
| `stock` | add whatever the Wishlist tab's shopping list is still missing |
| `scan head` | read-only: look in the game's memory for the blacksmith's list of head pieces (also `body`, `arms`, `waist`, `legs`); see below |
| `equip 12` | show the 16 bytes of equipment box slot 12 (slots count from 0; the Equipment tab's order is the box order unless sorted) |
| `equip 12 = 06 00 00 01 25 0a ...` | write a whole record (32 hex digits); `equip 12 @4 25 0a` writes bytes from offset 4 of the record |
| `talisman auto-guard 10, psychic 5` | add a talisman with those skills (names matched like items); its first four bytes are copied from a talisman you already have |
| `stock all` | the same, but also for wishlisted pieces you already own (for crafting another copy, e.g. to learn a starter weapon's price) |

Edited values are what the game sees from then on (a shop or the item box shows them), and they come back to the screen like
any other live change. **If you save in the game, the edits are saved with them.** So when edit mode starts, the save slots
(`user1`-`user3`, `system`) are copied to `~/.local/share/mh3u-companion/backups/<unix time>/`, and the status line says
where. It is a good idea to use a spare hunter.

### Finding what an equipment record's bytes mean (`equip`, `talisman`)

An equipment box record is 16 bytes: kind, a state byte, the piece id (u16) and 12 bytes that hold a talisman's skills and, probably, a
piece's socketed decorations. To learn them, write a record and look at it in the game, a change at a time:

1. `equip 12` on a talisman shows its bytes (the Equipment tab lists the box in box order with `s` set to box order).
2. `talisman auto-guard 10` adds one with a skill you know; check it in the game's item box (it should read Auto-Guard +10).
3. `equip N @6 30 05` writes a second pair (skill id 0x30, 5 points); does the game show a second skill? Then try the other bytes
   (`@10`, `@12`, `@14`...) one at a time, and note what the screen says.

Edits go into the live game only, and the saves are backed up first like every other debug edit; use a spare hunter.

### Finding the blacksmith's list (`scan`)

Where the game keeps the list of pieces the blacksmith offers is not known (the app works it out from the monsters you have hunted). To
look for it, open the blacksmith's head-armor menu in the game and type `:scan head`. This searches all of Cemu's writable memory for
runs of the piece ids the app expects on offer: big-endian u16 values, 2 to 24 bytes apart, either increasing or each once in any order (the menu may group pieces by set, not by id). It writes a report to
`scan-<time>.txt` in the data folder with the best twelve runs: where each is (in Cemu's memory and as a guest address), which expected
ids it lacks, and the bytes around it. It also searches for one flag per piece (by row of the game's recipe table, or by piece id) stored as
bytes, words, dwords or packed bits, with up to two flags different from the app's idea, and lists those first (as `F1`, `F2`...). The
blacksmith's menu lists pieces in the order of the recipe table (checked on a head menu: Alloy comes before Bone because the table has
them in that order), so a list of ids in id order is not what to expect. If the app's idea of the list is wrong, give the menu's real contents after the kind, as game names separated by commas (`:scan head Leather Headgear, Piscine Mask, ...`): the search then wants every flag to match exactly. It only reads, but the screen stands still for a few seconds. Runs of consecutive ids are counting
tables and score low. Compare a good run with what the menu really shows; a piece the run has and the app does not expect (or the
other way round) is a place where the unlock rule is wrong.

Edits only touch the item box (new items go in the first empty slot) and the wallet, and are made by overwriting the same
bytes the game uses, found as described above.

## Verified on the real game

Checked by playing (US v32, Cemu started by the TUI): moving items between the pouch and box, selling and buying (zenny and
items follow immediately), crafting at the blacksmith (the new piece appears in the equipment box and the zenny cost shows as a
change, e.g. -300 for a Piscine Mask), equipping gear (the new `worn` pointer), and unloading a hunter without saving (back to the
save file). Edits made with `--debug-edit` are seen by the game itself: `zenny 5000` showed in the blacksmith's shop, and
`give` / `set` items appeared and disappeared in the in-game item box.

## Troubleshooting

Start with `MH3U_LIVE_LOG=<file>` to get a log of every search: each hunter name's hits in memory and the header found there.

## Limits

- Linux, and a Cemu you start through the TUI.
- Only the save data is live. Everything else (names, recipes, stats) comes from the game files, as before.
- The header check was derived from one capture (US v32 on this machine's Cemu build). If a future Cemu lays the object out
  differently, the TUI stays on `◌ waiting for the game` and keeps using the save file.
- Hunters are matched by name, so two save slots with the same name would be ambiguous (the slot on screen wins, then the lowest).

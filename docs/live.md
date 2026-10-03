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

Only the hunter currently loaded in the game is live; the save slot you opened with `--slot` / `--save` is used for the
names to search for and as the fallback.

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
| `stock all` | the same, but also for wishlisted pieces you already own (for crafting another copy, e.g. to learn a starter weapon's price) |

Edited values are what the game sees from then on (a shop or the item box shows them), and they come back to the screen like
any other live change. **If you save in the game, the edits are saved with them.** So when edit mode starts, the save slots
(`user1`-`user3`, `system`) are copied to `~/.local/share/mh3u-companion/backups/<unix time>/`, and the status line says
where. It is a good idea to use a spare hunter.

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
- Hunters are matched by name, so two save slots with the same name would be ambiguous.

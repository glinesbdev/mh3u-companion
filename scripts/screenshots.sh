#!/usr/bin/env bash
# Regenerate the README screenshots (docs/screenshots/*.svg) by driving the real app in tmux.
#
# Needs tmux, a release build of both tools (cargo build --release), the game dump (the game_dir of your settings, or MH3U_GAME_DIR)
# and a save to show. The app runs in a sandbox: its wishlist, price ledger and unlocked-pieces file live in a temporary
# directory, so your own are never read or changed. The hunter name is replaced with "Hunter" in the pictures, and the plain
# icons are used (the anvil is a Nerd Font glyph that most viewers of the README would not have).
#
#   SAVE=snapshots/03-latest/user1 scripts/screenshots.sh
set -euo pipefail
cd "$(dirname "$0")/.."

SAVE=${SAVE:-snapshots/03-latest/user1}
# the hunter's name as the save has it (21 bytes at 0x2b), replaced with "Hunter" in the pictures
NAME=${NAME:-$(dd if="$SAVE" bs=1 skip=43 count=21 2>/dev/null | tr -d '\0')}
COLS=${COLS:-118}
ROWS=${ROWS:-34}
SESSION=mh3u-shots
SANDBOX=$(mktemp -d "${TMPDIR:-/tmp}/mh3u-shots.XXXXXX")
trap 'tmux kill-session -t "$SESSION" 2>/dev/null || true; rm -rf "$SANDBOX"' EXIT

mkdir -p "$SANDBOX/config/mh3u-companion" "$SANDBOX/data/mh3u-companion" docs/screenshots
# A small wishlist: three Jaggi pieces, and a great sword with the weapons it is upgraded through (auto = added as a parent).
printf '%s\n' '4 11' '4 12' '1 10' '7 6' '7 5 auto' '7 4 auto' '7 3 auto' '7 2 auto' >"$SANDBOX/config/mh3u-companion/wishlist.txt"

tmux kill-session -t "$SESSION" 2>/dev/null || true
tmux new-session -d -s "$SESSION" -x "$COLS" -y "$ROWS" \
  "MH3U_ICONS=plain MH3U_GAME_DIR='$(scripts/dump-dir.sh)' XDG_CONFIG_HOME=$SANDBOX/config XDG_DATA_HOME=$SANDBOX/data target/release/mh3u-tui --save $SAVE; sleep 600"
sleep 2

keys() { tmux send-keys -t "$SESSION" "$@"; sleep 0.6; }
shot() { # shot <name>
  tmux capture-pane -t "$SESSION" -p -e >"$SANDBOX/$1.ansi"
  target/release/mh3u-tools ansi2svg "$SANDBOX/$1.ansi" "docs/screenshots/$1.svg" "$NAME=Hunter" "$SANDBOX/config=~/.config"
  echo "wrote docs/screenshots/$1.svg"
}

keys p Down Down Down Down Down Down Down Down   # the pouch, on Jaggi Hide
shot items
keys p                    # back to the box

keys S                    # the Settings screen
keys Right                # icons: plain
keys Down Right           # the accent color: the next in the list
shot settings
keys d Escape


keys Right Right Right    # Crafting (Items, Equipment, Worn, Crafting)
keys '/' 'jaggi greaves' Enter
shot crafting-armor
keys Escape               # clear the search

keys '/' 'ravager blade' Enter
keys Home
shot crafting-weapon
keys v Down v Up          # two weapons for the comparison, highlight back on the first

keys t                    # upgrade tree
shot upgrade-tree
keys t
keys Escape

keys Right                # Wishlist
shot wishlist

keys Left Left Left       # Equipment (back past Crafting and Worn)
keys s                    # sort by name
shot equipment

keys Right                # Worn
keys i                    # with what each skill does
shot worn
keys i

keys Right Right Right    # Monsters (Crafting, Wishlist, Monsters)
keys s s                  # the ones the wishlist needs first
shot monsters

keys Right                # Hunt plan
shot hunts

keys Right                # Quests
keys '/' 'arzuros capture' Enter
shot quests
keys Escape

keys Right                # Families
keys '/' 'jaggi' Enter
shot families
keys Escape

keys Right                # Skills
keys '/' 'attack' Enter
shot skills
keys Escape

keys Right                # Compare
shot compare

keys Right                # Builds
keys a
keys 'psychic'
keys Enter
shot builds
keys f s Enter            # save the best set as a template
keys f                    # the templates list, with the saved set in full
shot templates

#!/usr/bin/env bash
# Regenerate the README screenshots (docs/screenshots/*.svg) by driving the real app in tmux.
#
# Needs tmux, a release build of both tools (cargo build --release), the game dump (found under ~/games/wiiu, as the app does)
# and a save to show. The app runs in a sandbox: its wishlist, price ledger and unlocked-pieces file live in a temporary
# directory, so your own are never read or changed. The hunter name is replaced with "Hunter" in the pictures, and the plain
# icons are used (the anvil is a Nerd Font glyph that most viewers of the README would not have).
#
#   SAVE=snapshots/03-latest/user1 scripts/screenshots.sh
set -euo pipefail
cd "$(dirname "$0")/.."

SAVE=${SAVE:-snapshots/03-latest/user1}
NAME=${NAME:-Shamus}
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
  "MH3U_ICONS=plain XDG_CONFIG_HOME=$SANDBOX/config XDG_DATA_HOME=$SANDBOX/data target/release/mh3u-tui --save $SAVE; sleep 600"
sleep 2

keys() { tmux send-keys -t "$SESSION" "$@"; sleep 0.6; }
shot() { # shot <name>
  tmux capture-pane -t "$SESSION" -p -e >"$SANDBOX/$1.ansi"
  target/release/mh3u-tools ansi2svg "$SANDBOX/$1.ansi" "docs/screenshots/$1.svg" "$NAME=Hunter"
  echo "wrote docs/screenshots/$1.svg"
}

shot items

keys Right Right          # Crafting
keys '/' 'jaggi greaves' Enter
shot crafting-armor
keys Escape               # clear the search

keys '/' 'ravager blade' Enter
keys Home
shot crafting-weapon

keys t                    # upgrade tree
shot upgrade-tree
keys t
keys Escape

keys Right                # Wishlist
shot wishlist

keys Left Left             # Equipment
keys s                    # sort by name
shot equipment

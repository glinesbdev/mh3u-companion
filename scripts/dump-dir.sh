#!/usr/bin/env bash
# Print the game folder the program is set up to use, for the other scripts and for running the tests that need the game's files:
# MH3U_GAME_DIR if it is set, else `game_dir` from the settings file of the profile in use (config.txt, or config-NAME.txt for the
# profile named in the `profile` file of the config folder). Prints nothing when none is set.
set -euo pipefail
if [ -n "${MH3U_GAME_DIR:-}" ]; then
    echo "$MH3U_GAME_DIR"
    exit 0
fi
dir=${XDG_CONFIG_HOME:-$HOME/.config}/mh3u-companion
profile=default
if [ -f "$dir/profile" ]; then
    profile=$(tr -d '[:space:]' <"$dir/profile")
fi
file="$dir/config.txt"
if [ "$profile" != default ]; then
    file="$dir/config-$profile.txt"
fi
[ -f "$file" ] || exit 0
sed -n 's/^[[:space:]]*game_dir[[:space:]]*=[[:space:]]*//p' "$file" | head -n 1 | sed 's/^"\(.*\)"[[:space:]]*$/\1/; s/[[:space:]]*$//'

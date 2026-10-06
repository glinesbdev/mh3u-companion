#!/usr/bin/env bash
# The checks CI runs, in the same order, stopping at the first failure. Run it before every commit.
set -euo pipefail
cd "$(dirname "$0")/.."
# the tests that need the game's files find them through MH3U_GAME_DIR (set here from the settings file); without it they skip
MH3U_GAME_DIR=$(scripts/dump-dir.sh)
export MH3U_GAME_DIR
cargo fmt --all --check
# twice: the default build only reads the game; --all-features adds the debug editing tools (the `edit` feature)
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo test --all --all-features
# known vulnerabilities in the dependencies (https://rustsec.org). CI always runs this; here it runs when cargo-audit is installed
# (cargo install cargo-audit --locked) and says so when it is not
if command -v cargo-audit >/dev/null 2>&1; then
    cargo audit
else
    echo "note: cargo-audit is not installed, so the dependency audit was skipped here (CI runs it)" >&2
fi
# the app must not know about any one screen
if cargo tree -p mh3u-app | grep -E "ratatui|crossterm"; then
    echo "mh3u-app depends on a terminal crate; keep screens out of it" >&2
    exit 1
fi
# the default build has no way to change the game, so it does not even have the flag; the `edit` build does
if cargo run -q -p mh3u-tui -- --help | grep -q -e "--debug-edit"; then
    echo "the default build of mh3u-tui has --debug-edit; it belongs behind the edit feature" >&2
    exit 1
fi
cargo run -q -p mh3u-tui --features edit -- --help | grep -q -e "--debug-edit"

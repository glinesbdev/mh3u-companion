#!/usr/bin/env bash
# The checks CI runs, in the same order, stopping at the first failure. Run it before every commit.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all
# the app must not know about any one screen
if cargo tree -p mh3u-app | grep -E "ratatui|crossterm"; then
    echo "mh3u-app depends on a terminal crate; keep screens out of it" >&2
    exit 1
fi

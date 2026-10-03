#!/usr/bin/env bash
# The checks CI runs, in the same order, stopping at the first failure. Run it before every commit.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all

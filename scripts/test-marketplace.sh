#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test
cargo build --release
./target/release/snackctl validate examples/criteric.insert-timestamp-0.1.0.yummy
./target/release/snackctl publish examples/criteric.insert-timestamp-0.1.0.yummy --dry-run
SNACK_MARKETPLACE_INDEX="$(pwd)/registry-starter/index.json" ./target/release/snackctl search timestamp
printf 'Marketplace smoke test completed (no GitHub writes).\n'

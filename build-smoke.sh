#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
command -v cargo >/dev/null || { echo 'Rust Cargo missing; install Rust via rustup.' >&2; exit 1; }
printf '\n[1/5] Rust tests\n'
cargo test
printf '\n[2/5] Release build\n'
cargo build --release
printf '\n[3/5] Executable version\n'
./target/release/snack --version
./target/release/snackctl --version
printf '\n[4/5] Sample .yummy integrity validation\n'
./target/release/snackctl validate examples/local.greeter-0.1.0.yummy
./target/release/snackctl validate first-plugin/criteric.quick-todo-0.1.0.yummy
printf '\n[5/5] Help / command registration\n'
./target/release/snack --help
./target/release/snackctl --help
printf '\nSnack 0.6.3 smoke test passed!\n'

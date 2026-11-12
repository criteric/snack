# Snack 0.4.2 — Keyboard shortcuts and version flags

- `Ctrl+O` now opens the **folder** selector.
- `Ctrl+Shift+O` opens the **file** selector when the terminal reports Shift and Control separately.
- `F4` is a fallback to open a file in terminals without extended keyboard reporting. The File menu also remains available.
- Snack requests enhanced keyboard reporting from compatible terminals and restores terminal state on exit, shell launch and GUI file selection.
- `snack --version` and `snack -version` (also `-V`) print the program version instead of opening the editor.
- `snackctl --version` and `snackctl -version` (also `-V`) print the CLI version.
- Previous 0.4.1 compile hotfix is included.

Build: `cargo build --release`.

This code was checked for consistency and ZIP integrity; Rust/Cargo compilation could not be run in the current environment.

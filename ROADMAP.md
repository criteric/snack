# Snack — incremental development plan

The goal is an extensible editor with real LSP, an embedded PTY terminal, Git workflows, `.yummy` Lua/Native/WASM runtimes and a DAP debugger.

1. **0.6.0 functional preview:** preserve 0.5.0, add usable Emmet subset, nested Explorer, git status, non-PTY command runner, on-demand LSP completion, Problems list, editing utilities, on-demand Lua package runner. No fake placeholders presented as working runtimes.
2. **0.7.x editor services:** full PTY with resize and process lifecycle, LSP synchronization/hover/code actions, file tree move/copy and drag where supported, SDK event bus.
3. **0.8.x extensions and Git:** WASM host and native process/in-process adapters with explicit risk acknowledgment; Git stage/commit/diff; structured plugin hooks to provide completions.
4. **0.9.x debugger and stability:** DAP debugger with breakpoints, stepping/variables, signature optional, error prompts, regression tests on Linux and Windows.
5. **1.0 stable:** document SDK, safe update/recovery, marketplace publishing and release binaries validated in CI.

Always run `cargo test && cargo build --release` for each milestone and record results. Do not deploy native in-process plugins from unknown publishers.

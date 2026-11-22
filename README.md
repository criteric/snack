# Snack 0.6.3 — Extensions Marketplace (Developer Preview)

**New:** press `F9` in Snack or select **Extensions → Marketplace...** to open the built-in plugin browser. It reads the existing official registry at `criteric/snack-plugins` and downloads verified `.yummy` packages from GitHub Releases. It uses Tokyo Night colors and the editor's existing diff renderer.

Snack is developed on **Fedora Linux** and can be built and installed on **Fedora Linux, Debian, and Arch Linux**.

## Install on Linux

From the Snack source directory, install the build dependencies and optional file picker and Lua runtime using your distribution's package manager:

| Distribution | Commands |
| --- | --- |
| Fedora Linux | `sudo dnf install gcc git rust cargo zenity lua` |
| Debian | `sudo apt update` followed by `sudo apt install build-essential git rustc cargo zenity lua5.4` |
| Arch Linux | `sudo pacman -Syu --needed base-devel git rust zenity lua` |

These commands install the dependencies. Build, test, and install Snack and Snackctl from source:

```bash
cargo test
cargo build --release
install -Dm755 target/release/snack "$HOME/.local/bin/snack"
install -Dm755 target/release/snackctl "$HOME/.local/bin/snackctl"
```

Run `snack` if `~/.local/bin` is on your PATH, or run `~/.local/bin/snack` directly.

### Test without installing

```bash
cargo test
cargo build --release
./target/release/snack
```

In Snack, `F9` opens/closes the Marketplace. **Type text** to search; `Backspace` deletes search characters, `/` clears the search, `Tab` switches between Discover and Installed, arrows select, `Enter` shows details, `F5` refreshes, `I` asks to install, `U` asks to update, `X` asks to uninstall, and `P` runs a locally installed plugin (through Snack's existing first-run consent). Click the tabs, list, or footer actions with the mouse. Every installation or removal requires an additional `Y` confirmation; `N` or `Esc` cancels. The network operations execute on worker threads to avoid freezing the editor.

**Example:** first install `criteric.insert-timestamp` from Marketplace (already published in the official repository), then press `P` or close the Marketplace and `Ctrl+P` to run it.

## First new plugin: Quick TODO

The archive `first-plugin/criteric.quick-todo-0.1.0.yummy` is a *locally created*, **not-yet-published** sample. It inserts the current date and `TODO` at the caret.

```bash
./target/release/snackctl validate first-plugin/criteric.quick-todo-0.1.0.yummy
./target/release/snackctl install first-plugin/criteric.quick-todo-0.1.0.yummy
./target/release/snack
```

Then press `F9`, switch to **Installed**, select Quick TODO and press `P`. Confirm `YES` when Snack first asks to run the Lua package. Alternatively, `Ctrl+P` followed by `criteric.quick-todo` runs it. You need `lua` or `lua5.4` installed. You can inspect its original Lua source in `first-plugin/criteric.quick-todo/src/main.lua`.

To publish after reviewing and testing:

```bash
./target/release/snackctl publish first-plugin/criteric.quick-todo-0.1.0.yummy --dry-run
./target/release/snackctl publish first-plugin/criteric.quick-todo-0.1.0.yummy
```

Publishing requires `gh auth login` and write permissions to `criteric/snack-plugins`. This project ZIP does **not** publish automatically. Once published and indexed, the plugin becomes discoverable in Marketplace after `F5`.

## Safety and limitations

- Marketplace uses the official repository's `index.json`, with an optional `SNACK_MARKETPLACE_INDEX` override for local test fixtures.
- Downloads are checked against the index's SHA-256, the `.yummy` internal checksums, and manifest ID/version. This is **not** a digital signature or malware scanner.
- Installing a plugin does not execute it. **Lua execution has the same operating-system permissions as Snack**, and the first run asks for consent.
- Only Lua runtime plugins work at present. Native/WASM, plugin ratings, screenshots, advanced lazy loading, and a fully featured LSP plugin SDK remain future work.
- The Included Quick TODO package is not yet listed in the official index until you publish it.
- The Marketplace's shell-only CLI remains available via `snackctl search`, `snackctl install`, `snackctl list`, and `snackctl update`.
- **No Rust compiler was available in the artifact preparation environment.** Source-level checks and `.yummy` binary integrity checks were performed, but `cargo test` and `cargo build` should be run on your target distribution. Please share any compiler errors for a focused fix.

# Snack 0.6.2 — Context-Aware Mouse Menus (Rust)

Snack is a lightweight, Tokyo Night terminal code editor for **Linux and Windows**. This release extends the last working 0.5.0 tree; the existing `snackctl` tool and custom `.yummy` archive format are preserved.

> **Build status:** This source package was checked for module completeness, Rust delimiter balance, archive integrity and expected command wiring. `cargo build` and `cargo test` **could not be run in the packaging environment** because Rust was not installed. Run the included smoke test on your Linux system. These are development sources, not verified release binaries.

## 1. Installation and build on Linux

Install the dependencies for your distribution using the table above, then run:

```bash
cd snack-0.6.2
cargo test
cargo build --release
./target/release/snack --version
./target/release/snackctl --version
```

If you want **Rust semantic completions** (as opposed to built-in snippets), install the external language server:

```bash
rustup component add rust-analyzer
rust-analyzer --version
```

Launch from your project root:

```bash
./target/release/snack examples/demo.html
```

## 2. Changes and keybindings

| Feature | Test it | Scope in 0.6.0 |
|---|---|---|
| HTML Emmet | Type `!`, `ul>li*5` or `section>h1+p` then Tab in `.html` | Common tags, IDs, classes, children, siblings, repetition (up to 32), `{text}`; not full Emmet grammar |
| Existing completion | Type `main` then Tab in `.rs` | Snippets + current-file words retained |
| Language server completion | **Ctrl+Space** in Rust code | Uses the installed `rust-analyzer` LSP asynchronously; several other servers detected by executable name |
| Explorer | Click folders on the left | Expand/collapse nested folders; right-click for create, rename and delete |
| File picker | Ctrl+O for folder, Ctrl+Shift+O / F4 for file | Linux GTK/Zenity or Windows native picker retained |
| Git changes | **Ctrl+G** | Read-only status list; click a changed file to open it; View > Refresh Git |
| Command panel | **Ctrl+`** or View > Terminal Panel | Run shell commands in editor and view output; **not a PTY** — interactive TUI commands aren't supported here |
| Existing interactive shell | Ctrl+T | Temporarily exits editor UI to run your interactive shell, then returns |
| Problems panel | **F8** or Ctrl+D | Displays diagnostics received from LSP; **not a DAP debugger** |
| Find and replace | Ctrl+F / **Ctrl+H** | Find; replace all occurrences with one undo checkpoint |
| Auto-indent and pairs | Press Enter after indented line, type `{`/`[`/`(` | Whitespace indentation and bracket insertion; users can undo |
| Lua `.yummy` runner | **Ctrl+P**, enter plugin ID, type **YES** once | Runs installed Lua plugins only on command, not at startup; first-run consent tied to file hash |

`Ctrl+Shift+O` is limited by some terminals' keyboard protocols; use F4 if they cannot report Shift. Normal editing, tabs, mouse, recovery and settings from v0.5 remain in place.

## 3. Real LSP IntelliSense — supported server commands

Snack's initial client uses the standard LSP `initialize`, `didOpen`, `didChange`, `textDocument/completion` and `publishDiagnostics` messages. An installed language server must be accessible on PATH. It uses Ctrl+Space on demand; regular snippet suggestions still show automatically as you type.

| Language | Expected executable |
|---|---|
| Rust | `rust-analyzer` |
| Python | `pylsp` |
| JS / TypeScript | `typescript-language-server --stdio` |
| C/C++ | `clangd` |
| Go | `gopls` |
| HTML | `vscode-html-language-server --stdio` |
| CSS | `vscode-css-language-server --stdio` |

Some LSP features are intentionally not implemented yet: snippet text edit placeholders, code actions/auto imports, hover, go-to-definition and incremental text updates. Other languages retain local snippets until adapters are added. If a language server is missing, built-in snippets continue working.

## 4. Working `.yummy` Lua proof of concept

The included `examples/local.greeter-0.1.0.yummy` is a **custom binary .yummy archive**, not a renamed ZIP. It has `manifest.toml` and `src/main.lua`, with per-entry SHA-256 integrity verification.

```bash
./target/release/snackctl validate examples/local.greeter-0.1.0.yummy
./target/release/snackctl install examples/local.greeter-0.1.0.yummy
./target/release/snack
```

Inside Snack press **Ctrl+P**, type `local.greeter`, press Enter, then type `YES` and Enter to grant access to that exact archive version. It prints a greeting in Snack's status bar. To edit an example plugin, start from `examples/lua-greeter/` and use `snackctl compiler examples/lua-greeter` (it backs up source before packaging).

**Security:** Lua is launched as an external process with the same user permissions as Snack. Its first-run confirmation is a trust prompt, **not a sandbox**. Only install code you trust. There is no arbitrary native/WASM execution yet and `.yummy` digest checks are **not digital signatures**.

`snackctl unzip -ex examples/local.greeter-0.1.0.yummy` prompts for an extraction folder; `--output DIR` works without a UI.

## 5. Windows

```powershell
cargo test
cargo build --release
.\target\release\snack.exe --version
.\target\release\snack.exe examples\demo.html
```

Windows pickers use PowerShell/WinForms. Lua plugin execution requires a working `lua.exe` in PATH. Git and language servers are optional. Windows has **not been compiled or tested** in this development environment.

## 6. Roadmap — features delivered in stages

**0.6.0**: Editor-preserving integration: Emmet subset, nested Explorer, Git read-only status, inline command runner, first LSP completion client, LSP Problems list, Lua `.yummy` on-demand runner, auto-indent and replace.

**0.7.x**: Full PTY embedded terminal, event-driven LSP sync, auto-import/hover/definition, diagnostic navigation and structured SDK events for Lua plugins.

**0.8.x**: Git staging/unstaging and diff panel; multi-cursor and code folding; real WASM and native plugin runtime adapters; plugin event API.

**0.9–1.0**: Real Debug Adapter Protocol (DAP) debugger with breakpoints, stepping, variables; cross-platform regression testing, safe plugin runtime lifecycle, marketplace integration and stable SDK.

**A Problems panel is not the same as a debugger**; this preview does not claim to implement DAP debugging. Likewise, the command panel is not yet an interactive PTY.

## 7. Diagnostic steps

```bash
./build-smoke.sh
# If it fails, paste the first Rust error and the corresponding file:line.
```

You can always rebuild from a separate new directory without deleting Snack 0.5.0. The CLI package and editor are separate binaries (`snack` and `snackctl`).

## 8. Snack 0.6.2 — contextual right-click menus

Mouse right-click is **region-aware** rather than always opening the Explorer menu:

- **Editor:** Cut/Copy/Paste, Select All/Line, Duplicate/Delete Line, comment toggle, Undo/Redo, Find/Replace, Save, Copy Path, Reveal in Files.
- **Explorer:** different menus for files, folders and blank space; create, open, rename, delete (with `DELETE` confirmation), expand, copy name/path, reveal, refresh.
- **Tabs:** Save/Save As, copy file path, reveal, close tab, close others/right (refuses to close dirty tabs), new/open.
- **Git status:** Open changed file, copy path, reveal, refresh status, return to Explorer.
- **Command panel:** Copy output/command, paste command, clear output, run, focus/hide, open interactive shell.
- **Problems / status bar:** copy/clear diagnostics, LSP completion request, show/hide panels, settings.

**Text selection:** drag the left mouse button across code, or hold Shift and use arrow keys. Right-click within an existing selection to keep it; right-click elsewhere moves the caret. `Ctrl+A`, `Ctrl+C`, `Ctrl+X`, and `Ctrl+V` are supported. With no selection, Copy/Cut operates on the whole current line. Desktop clipboard on Linux uses `wl-clipboard`, `xclip` or `xsel`; internal Snack clipboard works without them.

For desktop clipboard support, install `xclip` for X11 or `wl-clipboard` for Wayland using `dnf install` on Fedora, `apt install` on Debian, or `pacman -S` on Arch.

```bash
cargo test
cargo build --release
./target/release/snack examples/demo.html
```

Long context menus remain inside the terminal window and can be scrolled with the mouse wheel. Right-click the desired UI region to test it. If your terminal intercepts right-click for its own menu, disable the terminal's right-click context menu or use the **Edit** top-level menu for common editor operations.

**Scope:** This update does not implement Git staging, LSP go-to-definition, a full PTY or native/WASM extension runtimes. Git status and terminal command runner retain their earlier capabilities. Cross-platform builds and UI behaviour require testing in the target terminal. The original `.yummy` and `snackctl` implementation are included unchanged except for the shared package version.

# Snack 0.6.3 graphical TUI Marketplace

Press `F9` or open **Extensions > Marketplace...** from the menu bar.

The on-screen Marketplace is a separate editor screen, with tabs **Discover / Installed**, live text search, plugin detail pane, status messages, and clickable actions. It uses async worker threads for index retrieval and install/update/uninstall while keeping the terminal responsive. Refresh with `F5`.

Only plugins listed in the official index can be installed or updated from the remote browsing tab. Existing local `.yummy` packages appear under Installed even if the registry is offline. The first execution of a Lua plugin still needs consent via the editor's trusted-package fingerprint system.

Quick TODO sample: `first-plugin/criteric.quick-todo-0.1.0.yummy` (not published yet).

# Snack 0.6.1 — GitHub Marketplace Integration

Official registry: https://github.com/criteric/snack-plugins
Index: https://raw.githubusercontent.com/criteric/snack-plugins/main/index.json
Packages: GitHub Release assets (`.yummy`, custom binary format)

**Publishing:** `snackctl publish FILE.yummy` defaults to the official repository, requires `gh auth login` and repository write rights, creates a uniquely tagged Release, then updates `index.json` using the GitHub Contents API. `--dry-run` previews a publish without network writes. `--repo owner/repo --github` publishes an external release only and does not register it in the official index.

**Search:** `snackctl search keyword` reads the official index and separately searches community GitHub repositories, marked as unverified. If the official repository isn't public or the index is absent, the registry returns a warning.

**Install:** `snackctl install criteric.insert-timestamp` finds that ID in the official index, downloads its GitHub Release `.yummy` asset, verifies whole-file SHA-256 and internal checksums, checks manifest ID/version, then installs it as a local archive without executing it. `snackctl install path/to/file.yummy` still works offline.

**Update:** `snackctl update` checks versions and only prints available updates (no automatic downloads).

**Privacy and security:** `snackctl publish` uses the authenticated `gh` tool. Don't put PATs in scripts or source. Package SHA-256 doesn't guarantee a harmless plugin; Lua code executes as the user after Snack requests consent. This release only supports Lua plugin execution, not WASM/native execution or a complete SDK.

### Local offline test

```bash
bash scripts/test-marketplace.sh
```

It uses a local empty registry index. To test online publishing, first make the official GitHub repository public and upload the scaffold from `snack-plugins-registry/`.

### Installing development requirements on Zorin OS

```bash
sudo apt install git curl lua5.4 zenity gh
cargo build --release
./target/release/snackctl --help
```

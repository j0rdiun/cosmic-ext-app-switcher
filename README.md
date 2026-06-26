# cosmic-app-switcher

A macOS-style horizontal app switcher for the [COSMIC desktop](https://system76.com/cosmic) on Pop!_OS — replacing the default vertical Super+Tab list with a compact icon strip centered on screen.

![Switcher strip showing open app icons with the selected app highlighted]()

---

## What it looks like

```
┌────────────────────────────────────────────────────┐
│  ╔══════╗                                          │
│  ║  󰈹  ║   󰻞     󰆍     󰙯     󰎙               │
│  ╚══════╝                                          │
└────────────────────────────────────────────────────┘
```

- Always-dark frosted pill, centered on screen
- All icons the same size — selected app gets a soft white highlight box
- Works with both Super+Tab and Alt+Tab
- Activates the selected window on modifier release; Escape cancels

---

## Install

**One-line (no Rust required):**

```bash
curl -fsSL https://raw.githubusercontent.com/j0rdiun/cosmic-app-switcher/main/install.sh | bash
```

**Or build from source:**

```bash
# Install dependencies (one-time)
sudo apt install libxkbcommon-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && source ~/.cargo/env

git clone https://github.com/j0rdiun/cosmic-app-switcher
cd cosmic-app-switcher
make install
```

Changes take effect immediately — no logout required.

---

## Uninstall

```bash
curl -fsSL https://raw.githubusercontent.com/j0rdiun/cosmic-app-switcher/main/uninstall.sh | bash
```

Or from the project directory:

```bash
make uninstall
```

---

## Usage

| Shortcut | Action |
|---|---|
| Super+Tab / Alt+Tab | Open switcher / cycle forward |
| Super+Shift+Tab / Alt+Shift+Tab | Cycle backward |
| Modifier release | Activate selected window |
| Escape | Cancel — return to original window |
| Click icon | Select |
| Release click | Activate |

---

## Enable / Disable

```bash
make enable         # register as COSMIC window switcher (live reload)
make disable        # remove — restores COSMIC's default switcher
make status         # show current state
make check-compat   # verify COSMIC environment is compatible
```

---

## How it works

COSMIC's compositor (`cosmic-comp`) lets you override the `WindowSwitcher` action by pointing a config file at any binary. The scripts detect the config path automatically — if COSMIC updates and moves to a new config version, it still works.

Each key press launches the binary fresh. The first invocation creates a layer-shell overlay and binds a Unix socket. Subsequent presses (while the modifier is held) connect to that socket and send a `next`/`prev` signal — the running instance advances the selection and re-renders. Releasing the modifier triggers window activation via `zcosmic_toplevel_manager_v1`.

**Wayland protocols used:**
- `zcosmic_toplevel_info_v1` — enumerate open windows
- `zcosmic_toplevel_manager_v1` — activate a window
- `zwlr_layer_shell_v1` — overlay surface centered on screen

---

## Project structure

```
src/
  main.rs      # entry point: IPC check, then launch app
  app.rs       # libcosmic Application: state machine, socket subscription
  ui.rs        # horizontal icon strip view
  wayland.rs   # background thread: enumerate toplevels, activate window
  icons.rs     # app_id → icon name via .desktop file lookup
scripts/
  find-config.sh   # detects COSMIC shortcuts config path dynamically
  enable.sh        # writes WindowSwitcher entries to COSMIC config
  disable.sh       # removes them
  status.sh        # prints current state
install.sh     # standalone installer (downloads binary, no Rust needed)
uninstall.sh   # standalone uninstaller
.github/
  workflows/
    release.yml    # builds x86_64 + aarch64 binaries on tag push
```

---

## Releasing a new version

```bash
git tag v0.2.0
git push origin v0.2.0
```

GitHub Actions will build binaries for x86_64 and aarch64 and publish a release automatically.

---

## Requirements

- Pop!_OS with COSMIC desktop
- x86_64 or aarch64 architecture

Building from source additionally requires `libxkbcommon-dev` and Rust.

---

## License

MIT

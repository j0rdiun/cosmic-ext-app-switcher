# cosmic-app-switcher

A macOS-style horizontal app switcher for the [COSMIC desktop](https://system76.com/cosmic) on Pop!_OS — replacing the default vertical Super+Tab list with a compact icon strip centered on screen.

![Switcher strip showing open app icons with the selected app highlighted]()

---

## What it looks like

```
┌────────────────────────────────────────────────────┐
│  ╔══════╗                                          │
│  ║  󰈹  ║   󰻞     󰆍     󰙯     󰎙               │
│  ║Firefox║                                          │
│  ╚══════╝                                          │
└────────────────────────────────────────────────────┘
```

- Dark frosted pill, always — regardless of system theme
- Icons only for unfocused apps; selected app gets a white highlight ring and a label
- Cycles forward with Super+Tab, backward with Super+Shift+Tab
- Activates the selected window on Super release; Escape cancels

---

## Requirements

- Pop!_OS with COSMIC desktop (cosmic-comp)
- Rust toolchain (`rustup`)
- `libxkbcommon-dev`

```bash
sudo apt install libxkbcommon-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
```

---

## Install

```bash
git clone https://github.com/yourusername/cosmic-app-switcher
cd cosmic-app-switcher
make install
```

This builds the binary, installs it to `~/.local/bin/`, and registers it as the COSMIC window switcher. Changes take effect immediately — no logout required.

---

## Usage

| Shortcut | Action |
|---|---|
| Super+Tab | Open switcher / cycle forward |
| Super+Shift+Tab | Cycle backward |
| Super (release) | Activate selected window |
| Escape | Cancel — return to original window |
| Click icon | Select |
| Release click | Activate |

---

## Enable / Disable

```bash
make enable    # register as COSMIC window switcher (live reload)
make disable   # remove — restores COSMIC's default switcher
make status    # show current state
```

Disabling is instant and reversible. The binary stays installed.

---

## Uninstall

```bash
make uninstall
```

Removes the binary and restores the default COSMIC switcher.

---

## How it works

COSMIC's compositor (`cosmic-comp`) lets you override the `WindowSwitcher` action by pointing a config file at any binary:

```
~/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/system_actions
```

Each Super+Tab press launches the binary fresh. The first invocation creates a layer-shell overlay and binds a Unix socket. Subsequent presses (while Super is held) connect to that socket and send a `next`/`prev` signal — the running instance advances the selection and re-renders. Super release triggers window activation via `zcosmic_toplevel_manager_v1`.

**Wayland protocols used:**
- `zcosmic_toplevel_info_v1` — enumerate open windows
- `zcosmic_toplevel_manager_v1` — activate a window
- `zwlr_layer_shell_v1` — overlay surface centered on screen

---

## Build from source

```bash
cargo build --release
# binary at target/release/cosmic-app-switcher
```

First build takes a few minutes — libcosmic compiles from source. Subsequent builds are incremental.

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
  enable.sh    # writes WindowSwitcher entries to COSMIC config
  disable.sh   # removes them
  status.sh    # prints current state
```

---

## License

MIT

# cosmic-ext-app-switcher

A macOS-style horizontal app switcher for COSMIC desktop on Pop!_OS. Replaces the default vertical Super+Tab switcher with a compact, centered strip of app icons.

---

## How It Works

COSMIC reads `/home/jordan/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/system_actions` and launches the registered binary when Super+Tab is pressed. This project provides a standalone Rust binary that:

1. Opens a Wayland connection and queries all open windows via `zcosmic_toplevel_info_v1`
2. Renders a centered horizontal overlay (layer-shell surface) with app icons
3. Cycles selection on Tab; activates on Super release or Enter; cancels on Escape
4. Calls `zcosmic_toplevel_manager_v1::activate()` and exits

The override mechanism is fully reversible — `make disable` removes the shortcut registration, `make uninstall` removes the binary entirely, and the original COSMIC switcher resumes automatically.

---

## Package Lifecycle

```
make build      # compile the Rust binary
make install    # build + deploy binary + enable shortcuts
make uninstall  # disable shortcuts + remove binary
make enable     # register with COSMIC (binary must already be installed)
make disable    # deregister from COSMIC (binary stays on disk)
make status     # show whether enabled/disabled and binary present
make reinstall  # uninstall then install (useful for updates)
```

`enable`/`disable` only touch the COSMIC shortcuts config — no binary is moved. This lets you temporarily switch back to the default switcher without losing the build.

---

## Project Structure

```
cosmic-ext-app-switcher/
├── docs/
│   └── plan.md           # this file
├── scripts/
│   ├── enable.sh         # add WindowSwitcher entries to COSMIC config
│   ├── disable.sh        # remove WindowSwitcher entries from COSMIC config
│   └── status.sh         # report current install/enable state
├── Makefile              # install / uninstall / enable / disable / build / status
├── Cargo.toml
└── src/
    ├── main.rs           # CLI args (--reverse flag), two-phase init
    ├── wayland.rs        # background thread: query toplevels, activate window
    ├── app.rs            # libcosmic Application: keyboard events, state machine
    ├── ui.rs             # view(): horizontal icon strip
    └── icons.rs          # app_id → icon name via .desktop file lookup
```

---

## Prerequisites

Rust is not yet installed on this system. The Makefile handles this check.

```bash
# Install Rust (one-time)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# Missing build dep
sudo apt install libxkbcommon-dev
```

`libwayland-dev` and `build-essential` are already installed.

---

## Cargo.toml Dependencies

```toml
[package]
name = "cosmic-ext-app-switcher"
version = "0.1.0"
edition = "2021"

[dependencies]
libcosmic = { git = "https://github.com/pop-os/libcosmic", default-features = false, features = ["wayland", "tokio", "multi-thread"] }
cosmic-client-toolkit = { git = "https://github.com/pop-os/cosmic-client-toolkit" }
cosmic-protocols      = { git = "https://github.com/pop-os/cosmic-protocols" }
freedesktop-desktop-entry = "0.5"

wayland-client        = "0.31"
wayland-protocols     = { version = "0.32", features = ["client"] }
wayland-protocols-wlr = { version = "0.3",  features = ["client"] }

clap       = { version = "4", features = ["derive"] }
anyhow     = "1"
tokio      = { version = "1", features = ["rt-multi-thread", "sync"] }
log        = "0.4"
env_logger = "0.11"
```

> If build fails with conflicting `wayland-client` versions between git deps, check `cosmic-workspaces-epoch`'s `Cargo.lock` for pinned commit SHAs to align.

---

## Architecture: Two Wayland Connections

`libcosmic`'s iced backend manages its own `wl_display` connection for rendering. A **second background thread** owns the protocol objects (`zcosmic_toplevel_info_v1`, `zcosmic_toplevel_manager_v1`, `wl_seat`). The two sides communicate via `std::sync::mpsc` channels:

```
main()
 ├─ spawn background Wayland thread
 │    └─ connect → query toplevels → send list via channel → wait for ActivateCommand
 ├─ receive ToplevelEntry list
 └─ run libcosmic app (blocks until exit)
      └─ on activate: send ActivateCommand(handle_key) → background thread activates → exit
```

### wayland.rs key types

```rust
pub struct ToplevelEntry {
    pub app_id:     String,
    pub title:      String,
    pub is_active:  bool,
    pub handle_key: usize,
}

pub enum ActivateCommand { Activate(usize), Cancel }
```

Globals to bind:
- `zcosmic_toplevel_info_v1` (v2) — emits `toplevel` + `done` events
- `zcosmic_toplevel_handle_v1` — `app_id`, `title`, `state` (contains `Activated` bit)
- `zcosmic_toplevel_manager_v1` (v1) — `activate(handle, seat)`
- `wl_seat` (v7) — required parameter for `activate()`

Two `roundtrip()` calls after initial dispatch ensure all handles emit their metadata.

---

## app.rs: libcosmic Application

Layer-shell settings:

```rust
Settings::default()
    .layer_shell(true)
    .layer(Layer::Overlay)
    .keyboard_interactivity(KeyboardInteractivity::Exclusive)
    .anchor(Anchor::empty())   // compositor centers the surface
    .transparent(true)
```

**Super key release detection:**

```rust
// subscription(): global keyboard event listener
event::listen_with(|event, _| {
    if let Event::Keyboard(ke) = event { Some(Message::KeyEvent(ke)) } else { None }
})

// update():
Message::KeyEvent(keyboard::Event::ModifiersChanged(mods)) => {
    if self.super_held && !mods.logo() {
        return self.update(Message::Activate);
    }
    self.super_held = mods.logo();
}
```

`KeyboardInteractivity::Exclusive` routes modifier release events (including Super) to our surface. Initial selection: index 1 (next app) for forward, last index for `--reverse`.

---

## ui.rs: Horizontal Strip

```
┌──────────────────────────────────────────────────────┐
│  ┌──────┐  ┌──────┐  ┌─────────────────┐  ┌──────┐ │
│  │  🦊  │  │  📁  │  │  ▓▓▓▓▓▓▓▓▓▓▓▓  │  │  📺  │ │
│  │ Fire │  │Files │  │   CosmicTerm    │  │Player│ │
│  └──────┘  └──────┘  └─────────────────┘  └──────┘ │
└──────────────────────────────────────────────────────┘
```

- Strip: `background.component.base` at 92% opacity, `border_radius: 16px`, centered
- Unselected: 44px icon, transparent cell
- Selected: 52px icon, `accent_color` background, `border_radius: 12px`
- Label: 11px, last component of reverse-DNS app_id, truncated at 10 chars
- Mouse: hover = SelectIndex, release = Activate

---

## icons.rs: App Icon Resolution

1. Look for `{app_id}.desktop` in `/usr/share/applications` and `~/.local/share/applications`
2. Parse `Icon=` field via `freedesktop-desktop-entry` crate
3. Fallback: scan all `.desktop` files for `StartupWMClass` match (handles Chrome, Ghostty, etc.)
4. Final fallback: return `app_id` as-is (works for `firefox`, `code`, etc.)

---

## enable/disable: Shortcut Registration

`scripts/enable.sh` adds two entries to the COSMIC system_actions config:

```ron
{
    Terminal: "/usr/bin/ghostty --gtk-single-instance=true",
    WindowSwitcher: "/home/jordan/.local/bin/cosmic-ext-app-switcher",
    WindowSwitcherPrevious: "/home/jordan/.local/bin/cosmic-ext-app-switcher --reverse",
}
```

`scripts/disable.sh` removes those two lines (sed-based, keyed on `cosmic-ext-app-switcher`).

cosmic-comp watches this file and reloads live — no restart needed in either direction.

---

## Build & Install

```bash
# Full install (builds from source, deploys, enables)
make install

# Just build
make build

# Deploy and enable without rebuilding
make enable

# Temporarily disable (keep binary)
make disable

# Full removal
make uninstall
```

---

## Verification

1. **Smoke test** (before full build): `make enable` with placeholder binary — press Super+Tab, check `notify-send` fires
2. **After full build**: Super+Tab shows horizontal strip, Tab cycles, releasing Super activates
3. **Reversibility**: `make disable` → Super+Tab reverts to COSMIC default vertical list

---

## Key Reference Repos

- `https://github.com/pop-os/cosmic-workspaces-epoch` — layer-shell + toplevel protocol reference
- `https://github.com/pop-os/cosmic-applets` — libcosmic app structure

# cosmic-ext-app-switcher

macOS-style horizontal Super+Tab app switcher for COSMIC desktop on Pop!_OS.

## Build & install

```bash
source ~/.cargo/env          # Rust not on PATH by default in this environment
cargo build --release
make install                 # builds + installs to ~/.local/bin/ + enables shortcut
```

## Makefile targets

| Target | Effect |
|---|---|
| `make install` | build, install binary, enable shortcut |
| `make uninstall` | disable shortcut, remove binary |
| `make enable` | register in COSMIC shortcuts (live reload) |
| `make disable` | remove from COSMIC shortcuts, restore default |
| `make status` | show binary + shortcut state |

## Architecture

**Two-process design**: cosmic-comp launches a fresh binary on every Super+Tab keypress.

- First invocation: binds Unix socket at `/tmp/cosmic-ext-app-switcher.sock`, shows layer-shell overlay
- Subsequent Tab presses (Super still held): new binary connects to socket, sends `"next"`/`"prev"`, exits
- Running instance receives message via iced Subscription, updates selection, re-renders
- Super release → `ModifiersChanged(logo=false)` → activate selected window

**Two Wayland connections**: libcosmic manages its own `wl_display` for rendering. A background thread holds a separate connection for `zcosmic_toplevel_info_v1` and `zcosmic_toplevel_manager_v1`.

## Critical gotchas

- Bind `zcosmic_toplevel_info_v1` at **version 1** — v2 never emits `Toplevel` events
- Layer surface requires explicit pixel size: `size: Some((Some(w), Some(h)))` — `None` produces a 1×1 surface
- `super_held` must initialize to `false` — `true` causes immediate activation on the first modifier event
- cosmic-comp intercepts all Super+key combos before our exclusive surface sees them — Tab keypresses never arrive via keyboard events; IPC socket is the only cycling mechanism
- Use `cosmic::iced::Subscription`, `cosmic::iced::futures`, `cosmic::iced::stream` — not `iced_futures` directly
- Socket subscription sender type: `cosmic::iced::futures::channel::mpsc::Sender<Message>`

## Shortcut config

`~/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/system_actions`

cosmic-comp watches this file and reloads live on change.

## Key dependencies

- `libcosmic` (git, pop-os/libcosmic, rev `417923f`) — features: `wayland`, `tokio`, `winit`, `multi-window`
- `cosmic-protocols` (git, pop-os/cosmic-protocols, rev `c253ec1`) — `zcosmic_toplevel_*` protocol bindings
- `tokio` — async socket listener in subscription
- `freedesktop-desktop-entry` — `.desktop` file parsing for icon names

Keep the revs above in sync with `Cargo.toml`/`Cargo.lock` — check both when bumping.

### Finding the right rev after a COSMIC/cosmic-comp upgrade

`zcosmic_*` protocols are unstable (`z`-prefixed) and their wire format can and does
shift between cosmic-comp releases (e.g. a `_v1` global disappearing in favor of `_v2`,
or a bumped max version like `zcosmic_toplevel_info_v1` going from v1 to v3). A client
built against a mismatched `cosmic-protocols` rev can misinterpret event opcodes —
this tends to surface as a segfault, not a clean error, since a wrong `event_created_child!`
opcode mapping corrupts the object's type at the wayland-client level rather than failing loudly.

There's no published table mapping cosmic-comp releases to compatible
`libcosmic`/`cosmic-protocols` revs. To find one after an upgrade:

1. Check the installed version: `dpkg -l cosmic-comp` (format: `0.1~<unix-ts>~24.04~<short-sha>`
   — the trailing hex is cosmic-comp's own commit).
2. Look up that commit on `github.com/pop-os/cosmic-comp` and open its `Cargo.lock`.
3. Find the `libcosmic` and `cosmic-protocols` entries in that lock file — their pinned
   `rev`s are what cosmic-comp itself was built and tested against for that release.
4. Update this project's `Cargo.toml` to match, run `cargo build`, and re-verify Super+Tab
   end-to-end (a build succeeding is not sufficient — the crash described above happens
   at runtime, not compile time).

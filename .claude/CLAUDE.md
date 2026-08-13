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

## Release checklist

The version lives in `[workspace.package]` in the root `Cargo.toml`; both crates inherit it
with `version.workspace = true`. Three things must agree or the release workflow fails:

1. Bump `[workspace.package] version`.
2. Add a matching `<release version="…" date="…">` at the top of
   `applet/data/io.github.cosmic-ext-applet-app-switcher.metainfo.xml`. **This entry is what
   the COSMIC Store displays** — a stale one is why the store advertised 0.1.3 long after
   v0.1.4 shipped.
3. Tag `vX.Y.Z` and push. `.github/workflows/release.yml` checks tag/Cargo/metainfo before
   building and refuses to publish a mismatch.
4. The store does **not** follow tags or `main`. It rebuilds only when a PR to
   `pop-os/cosmic-flatpak` changes the pinned `"commit"` in
   `app/io.github.cosmic-ext-applet-app-switcher/io.github.cosmic-ext-applet-app-switcher.json`.
   Open that PR, regenerating the sibling `cargo-sources.json`
   (`flatpak/generate-cargo-sources.sh`) whenever `Cargo.lock` moved.

The in-repo `io.github.cosmic-ext-applet-app-switcher.json` is a local-build variant, and
differs from the upstream one in exactly two ways — diff them before opening a PR:

- `sources` uses a local `dir` instead of a pinned `git` commit, so `flatpak-builder`
  builds the working tree.
- it omits `base: com.system76.Cosmic.BaseApp` / `base-version: stable`. That base is not
  published on the user-facing `cosmic` remote (only cosmic-flatpak's own build
  environment has it), so keeping it here would break local builds. Re-add it in the PR.

## Flatpak: the switcher cannot work inside the sandbox

cosmic-comp gates its privileged globals on `client_not_sandboxed`
(`src/state.rs`), which is false for any client carrying a Wayland security context —
which Flatpak ≥1.15 always attaches. Verified empirically on 2026-08-13 by dumping
`wl_registry` inside and outside the sandbox: the host sees 58 globals, the sandbox 36,
and the 22 missing ones include **`zcosmic_toplevel_info_v1`, `zcosmic_toplevel_manager_v1`
and `zwlr_layer_shell_v1`** — i.e. no window list and no overlay surface. The store build
therefore exits silently on Super+Tab.

Two further sandbox facts: `--filesystem=/usr/share/applications:ro` (and the `icons`/
`pixmaps` equivalents) in the manifest are **silently ignored** — Flatpak reserves `/usr`,
so the sandbox sees 0 host `.desktop` files and only the `hicolor` icon theme, breaking
icon lookup. And nothing of ours runs at `flatpak uninstall`, so a registered shortcut
outlives the app.

The only configuration found that restores the globals is opting out of the Wayland
sandbox entirely: `--nosocket=wayland --filesystem=xdg-run/wayland-N` plus
`WAYLAND_DISPLAY` pointed at the host socket (verified: 58 globals inside the sandbox).
That is a deliberate sandbox bypass and needs cosmic-flatpak maintainer buy-in.

#!/usr/bin/env bash
set -euo pipefail

# cosmic-ext-app-switcher — standalone installer
# Downloads pre-built binaries from GitHub Releases (no Rust required).
# Usage: curl -fsSL https://raw.githubusercontent.com/j0rdiun/cosmic-ext-app-switcher/main/install.sh | bash

REPO="j0rdiun/cosmic-ext-app-switcher"
INSTALL_DIR="$HOME/.local/bin"
APPS_DIR="$HOME/.local/share/applications"
ICONS_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
BINARY="cosmic-ext-app-switcher"
APPLET="cosmic-ext-applet-app-switcher"
APPLET_DESKTOP_ID="io.github.cosmic-ext-applet-app-switcher"
SVG_NAME="io.github.cosmic-ext-applet-app-switcher-symbolic.svg"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-}")" && pwd)" || SCRIPT_DIR=""

# ── Detect architecture ───────────────────────────────────────────────────────
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  ARCH_TAG="x86_64-unknown-linux-gnu" ;;
    aarch64) ARCH_TAG="aarch64-unknown-linux-gnu" ;;
    *)
        echo "Error: unsupported architecture '$ARCH'." >&2
        echo "Build from source: https://github.com/$REPO" >&2
        exit 1
        ;;
esac

# ── Check for COSMIC ─────────────────────────────────────────────────────────
SHORTCUTS_DIR="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts"
if [ ! -d "$SHORTCUTS_DIR" ]; then
    echo "Error: COSMIC desktop shortcuts directory not found." >&2
    echo "Make sure COSMIC desktop is installed and has been launched at least once." >&2
    exit 1
fi

# ── Migrate from old binary name ─────────────────────────────────────────────
OLD_BINARY="cosmic-app-switcher"
if [ -f "$INSTALL_DIR/$OLD_BINARY" ]; then
    echo "Migrating from $OLD_BINARY to $BINARY..."
    rm -f "$INSTALL_DIR/$OLD_BINARY"
    MIGCONF=$(find "$SHORTCUTS_DIR" -name "system_actions" 2>/dev/null | sort -V | tail -1 || true)
    if [ -n "$MIGCONF" ] && grep -q "$OLD_BINARY" "$MIGCONF" 2>/dev/null; then
        MFTMP=$(mktemp)
        grep -v "$OLD_BINARY" "$MIGCONF" > "$MFTMP"
        mv "$MFTMP" "$MIGCONF"
    fi
fi

# ── Fetch latest release asset URLs ──────────────────────────────────────────
echo "Fetching latest release..."
if command -v curl &>/dev/null; then
    RELEASE_JSON=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest")
elif command -v wget &>/dev/null; then
    RELEASE_JSON=$(wget -qO- "https://api.github.com/repos/$REPO/releases/latest")
else
    echo "Error: curl or wget is required." >&2
    exit 1
fi

get_url() {
    echo "$RELEASE_JSON" | grep "browser_download_url" | grep "$1" | grep "$ARCH_TAG" | cut -d '"' -f 4
}

get_asset_url() {
    echo "$RELEASE_JSON" | grep "browser_download_url" | grep "$1" | cut -d '"' -f 4
}

SWITCHER_URL=$(get_url "$BINARY")
APPLET_URL=$(get_url "$APPLET")
SVG_URL=$(get_asset_url "$SVG_NAME")

if [ -z "$SWITCHER_URL" ]; then
    echo "Error: could not find switcher binary for $ARCH_TAG." >&2
    echo "Build from source: https://github.com/$REPO" >&2
    exit 1
fi

# ── Download and install switcher ─────────────────────────────────────────────
mkdir -p "$INSTALL_DIR" "$APPS_DIR"
TMPFILE=$(mktemp)
trap 'rm -f "$TMPFILE"' EXIT

echo "Downloading $BINARY ($ARCH_TAG)..."
if command -v curl &>/dev/null; then
    curl -fsSL "$SWITCHER_URL" -o "$TMPFILE"
else
    wget -qO "$TMPFILE" "$SWITCHER_URL"
fi
install -m755 "$TMPFILE" "$INSTALL_DIR/$BINARY"
echo "Installed: $INSTALL_DIR/$BINARY"

# ── Download and install applet ───────────────────────────────────────────────
if [ -n "$APPLET_URL" ]; then
    echo "Downloading $APPLET ($ARCH_TAG)..."
    if command -v curl &>/dev/null; then
        curl -fsSL "$APPLET_URL" -o "$TMPFILE"
    else
        wget -qO "$TMPFILE" "$APPLET_URL"
    fi
    install -m755 "$TMPFILE" "$INSTALL_DIR/$APPLET"
    echo "Installed: $INSTALL_DIR/$APPLET"

    # Install .desktop file so COSMIC panel can discover the applet
    cat > "$APPS_DIR/$APPLET_DESKTOP_ID.desktop" <<'DESKTOP'
[Desktop Entry]
Name=App Switcher Settings
Comment=Set the visual theme for cosmic-ext-app-switcher
Type=Application
Exec=cosmic-ext-applet-app-switcher
Icon=io.github.cosmic-ext-applet-app-switcher-symbolic
Terminal=false
NoDisplay=true
X-CosmicApplet=true
Categories=COSMIC;
Keywords=COSMIC;Applet;AppSwitcher;Theme;
DESKTOP
    echo "Installed: $APPS_DIR/$APPLET_DESKTOP_ID.desktop"
else
    echo "Warning: applet binary not found in release — skipping applet install." >&2
fi

# ── Download and install applet icon ─────────────────────────────────────────
if [ -n "$SVG_URL" ]; then
    mkdir -p "$ICONS_DIR"
    echo "Downloading $SVG_NAME..."
    if command -v curl &>/dev/null; then
        curl -fsSL "$SVG_URL" -o "$ICONS_DIR/$SVG_NAME"
    else
        wget -qO "$ICONS_DIR/$SVG_NAME" "$SVG_URL"
    fi
    echo "Installed: $ICONS_DIR/$SVG_NAME"
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor/" 2>/dev/null || true
else
    echo "Warning: icon SVG not found in release — skipping icon install." >&2
fi

# ── Register shortcut ─────────────────────────────────────────────────────────
if [ -f "$SCRIPT_DIR/scripts/enable.sh" ]; then
    bash "$SCRIPT_DIR/scripts/enable.sh"
else
    CONFIG=$(find "$SHORTCUTS_DIR" -name "system_actions" 2>/dev/null | sort -V | tail -1)
    if [ -z "$CONFIG" ]; then
        echo "Warning: could not find COSMIC system_actions config — shortcut not registered." >&2
        echo "Run 'make enable' from the project directory to register manually." >&2
        exit 0
    fi
    if grep -q "cosmic-ext-app-switcher" "$CONFIG"; then
        echo "Shortcut already registered."
    else
        TMPCONF=$(mktemp)
        grep -vE "^\s*(WindowSwitcher|WindowSwitcherPrevious):" "$CONFIG" | head -n -1 > "$TMPCONF"
        printf '    WindowSwitcher: "%s",\n    WindowSwitcherPrevious: "%s --reverse",\n}\n' \
            "$INSTALL_DIR/$BINARY" "$INSTALL_DIR/$BINARY" >> "$TMPCONF"
        mv "$TMPCONF" "$CONFIG"
        echo "Shortcut registered."
    fi
fi

echo ""
echo "Done! Press Super+Tab or Alt+Tab to try it."
echo "Add the 'App Switcher Settings' applet to your COSMIC panel to change themes."
echo "To uninstall: bash <(curl -fsSL https://raw.githubusercontent.com/j0rdiun/cosmic-ext-app-switcher/main/uninstall.sh)"

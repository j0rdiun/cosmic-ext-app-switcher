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
# The shortcuts directory itself may not exist yet on a fresh install that has never
# customised a keybinding, so presence of COSMIC is checked more loosely — the shortcut
# step below creates the config when it's missing.
SHORTCUTS_DIR="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts"
if ! command -v cosmic-comp &>/dev/null && [ ! -d "$HOME/.config/cosmic" ]; then
    echo "Error: COSMIC desktop not detected." >&2
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

# Match on the asset *filename* only. Matching anywhere in the line would also hit the
# repository name inside every asset URL ("…/cosmic-ext-app-switcher/releases/download/…"),
# which returned two URLs for the switcher and made curl fail with
# "URL rejected: Malformed input to a URL function".
get_url() {                                     # $1 = exact asset filename
    echo "$RELEASE_JSON" \
        | grep -o '"browser_download_url": *"[^"]*"' \
        | cut -d '"' -f 4 \
        | awk -v want="/$1" 'substr($0, length($0) - length(want) + 1) == want' \
        | head -n1
}

# `|| true` keeps a no-match (empty) result from aborting the script under `set -e`,
# so the explicit empty checks below stay reachable.
SWITCHER_URL=$(get_url "$BINARY-$ARCH_TAG" || true)
APPLET_URL=$(get_url "$APPLET-$ARCH_TAG" || true)
SVG_URL=$(get_url "$SVG_NAME" || true)

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
    # Standalone (curl | bash) path — mirrors scripts/find-config.sh and
    # scripts/shortcut-config.sh, which aren't on disk here.
    CONFIG=$(find "$SHORTCUTS_DIR" -name "system_actions" 2>/dev/null | sort -V | tail -1 || true)
    if [ -z "$CONFIG" ]; then
        # Never customised shortcuts before: create the config in the newest schema dir.
        VERSION_DIR=$(find "$SHORTCUTS_DIR" -maxdepth 1 -type d -name 'v[0-9]*' 2>/dev/null | sort -V | tail -1 || true)
        CONFIG="${VERSION_DIR:-$SHORTCUTS_DIR/v1}/system_actions"
    fi

    if [ -f "$CONFIG" ] && grep -q "cosmic-ext-app-switcher" "$CONFIG"; then
        echo "Shortcut already registered."
    else
        # Rebuild the RON map rather than patching it: a file left without its braces
        # fails to parse, and cosmic-settings-daemon then silently falls back to the
        # packaged defaults, dropping the user's other overrides (issue #10).
        BODY=""
        if [ -s "$CONFIG" ]; then
            BODY=$(awk '
                /^[[:space:]]*\{?[[:space:]]*\}?[[:space:]]*$/          { next }
                /^[[:space:]]*(WindowSwitcher|WindowSwitcherPrevious):/ { next }
                { print }
            ' "$CONFIG")
        fi

        TMPCONF=$(mktemp)
        {
            printf '{\n'
            if [ -n "$BODY" ]; then printf '%s\n' "$BODY"; fi
            printf '    WindowSwitcher: "%s",\n' "$INSTALL_DIR/$BINARY"
            printf '    WindowSwitcherPrevious: "%s --reverse",\n' "$INSTALL_DIR/$BINARY"
            printf '}\n'
        } > "$TMPCONF"

        if ! head -n1 "$TMPCONF" | grep -q '^{' || ! tail -n1 "$TMPCONF" | grep -q '^}'; then
            rm -f "$TMPCONF"
            echo "Error: refusing to write malformed shortcut config to $CONFIG" >&2
            exit 1
        fi

        mkdir -p "$(dirname "$CONFIG")"
        mv "$TMPCONF" "$CONFIG"
        chmod 600 "$CONFIG"
        echo "Shortcut registered."
    fi
fi

echo ""
echo "Done! Press Super+Tab or Alt+Tab to try it."
echo "Add the 'App Switcher Settings' applet to your COSMIC panel to change themes."
echo "To uninstall: bash <(curl -fsSL https://raw.githubusercontent.com/j0rdiun/cosmic-ext-app-switcher/main/uninstall.sh)"

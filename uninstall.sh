#!/usr/bin/env bash
set -euo pipefail

# cosmic-ext-app-switcher — standalone uninstaller
# Usage: curl -fsSL https://raw.githubusercontent.com/j0rdiun/cosmic-ext-app-switcher/main/uninstall.sh | bash

INSTALL_DIR="$HOME/.local/bin"
APPS_DIR="$HOME/.local/share/applications"
BINARY="cosmic-ext-app-switcher"
APPLET="cosmic-ext-applet-app-switcher"
OLD_BINARY="cosmic-app-switcher"
SHORTCUTS_DIR="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts"

echo "Uninstalling cosmic-ext-app-switcher..."

# ── Remove shortcut registration ──────────────────────────────────────────────
# Matches on the RON *keys*, not on the binary path: a substring match would also delete
# an unrelated entry that happens to mention the path, and rebuilding the whole map keeps
# the braces intact. Dropping the WindowSwitcher keys is what hands Super+Tab back to
# COSMIC's built-in switcher — a leftover entry pointing at a removed binary just spawns
# nothing, with no fallback (issue #10).
CONFIG=$(find "$SHORTCUTS_DIR" -name "system_actions" 2>/dev/null | sort -V | tail -1 || true)
if [ -n "$CONFIG" ] && grep -qE "^\s*(WindowSwitcher|WindowSwitcherPrevious):" "$CONFIG" 2>/dev/null; then
    BODY=$(awk '
        /^[[:space:]]*\{?[[:space:]]*\}?[[:space:]]*$/          { next }
        /^[[:space:]]*(WindowSwitcher|WindowSwitcherPrevious):/ { next }
        { print }
    ' "$CONFIG")

    TMPFILE=$(mktemp)
    {
        printf '{\n'
        if [ -n "$BODY" ]; then printf '%s\n' "$BODY"; fi
        printf '}\n'
    } > "$TMPFILE"

    if ! head -n1 "$TMPFILE" | grep -q '^{' || ! tail -n1 "$TMPFILE" | grep -q '^}'; then
        rm -f "$TMPFILE"
        echo "Error: refusing to write malformed shortcut config to $CONFIG" >&2
        exit 1
    fi

    mv "$TMPFILE" "$CONFIG"
    chmod 600 "$CONFIG"
    echo "Shortcut removed. COSMIC default switcher restored."
else
    echo "Shortcut not registered — nothing to remove."
fi

# ── Remove binaries ───────────────────────────────────────────────────────────
REMOVED=0
if [ -f "$INSTALL_DIR/$BINARY" ]; then
    rm -f "$INSTALL_DIR/$BINARY"
    echo "Binary removed: $INSTALL_DIR/$BINARY"
    REMOVED=1
fi
if [ -f "$INSTALL_DIR/$APPLET" ]; then
    rm -f "$INSTALL_DIR/$APPLET"
    echo "Binary removed: $INSTALL_DIR/$APPLET"
    REMOVED=1
fi
if [ -f "$INSTALL_DIR/$OLD_BINARY" ]; then
    rm -f "$INSTALL_DIR/$OLD_BINARY"
    echo "Legacy binary removed: $INSTALL_DIR/$OLD_BINARY"
    REMOVED=1
fi
if [ "$REMOVED" -eq 0 ]; then
    echo "Binary not found — nothing to remove."
fi

# ── Remove applet desktop file ────────────────────────────────────────────────
DESKTOP="$APPS_DIR/io.github.cosmic-ext-applet-app-switcher.desktop"
if [ -f "$DESKTOP" ]; then
    rm -f "$DESKTOP"
    echo "Desktop file removed: $DESKTOP"
fi

# ── Remove applet icon ────────────────────────────────────────────────────────
ICON="$HOME/.local/share/icons/hicolor/scalable/apps/io.github.cosmic-ext-applet-app-switcher-symbolic.svg"
if [ -f "$ICON" ]; then
    rm -f "$ICON"
    echo "Icon removed: $ICON"
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor/" 2>/dev/null || true
fi

echo ""
echo "Uninstall complete."

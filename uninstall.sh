#!/usr/bin/env bash
set -euo pipefail

# cosmic-ext-app-switcher — standalone uninstaller
# Usage: curl -fsSL https://raw.githubusercontent.com/j0rdiun/cosmic-ext-app-switcher/main/uninstall.sh | bash

INSTALL_DIR="$HOME/.local/bin"
BINARY="cosmic-ext-app-switcher"
OLD_BINARY="cosmic-app-switcher"
SHORTCUTS_DIR="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts"

echo "Uninstalling cosmic-ext-app-switcher..."

# ── Remove shortcut registration ──────────────────────────────────────────────
CONFIG=$(find "$SHORTCUTS_DIR" -name "system_actions" 2>/dev/null | sort -V | tail -1 || true)
if [ -n "$CONFIG" ]; then
    CHANGED=0
    if grep -q "$BINARY" "$CONFIG" 2>/dev/null; then
        TMPFILE=$(mktemp)
        grep -v "$BINARY" "$CONFIG" > "$TMPFILE"
        mv "$TMPFILE" "$CONFIG"
        CHANGED=1
    fi
    if grep -q "$OLD_BINARY" "$CONFIG" 2>/dev/null; then
        TMPFILE=$(mktemp)
        grep -v "$OLD_BINARY" "$CONFIG" > "$TMPFILE"
        mv "$TMPFILE" "$CONFIG"
        CHANGED=1
    fi
    if [ "$CHANGED" -eq 1 ]; then
        echo "Shortcut removed. COSMIC default switcher restored."
    else
        echo "Shortcut not registered — nothing to remove."
    fi
else
    echo "Shortcut not registered — nothing to remove."
fi

# ── Remove binary ─────────────────────────────────────────────────────────────
REMOVED=0
if [ -f "$INSTALL_DIR/$BINARY" ]; then
    rm -f "$INSTALL_DIR/$BINARY"
    echo "Binary removed: $INSTALL_DIR/$BINARY"
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

echo ""
echo "Uninstall complete."

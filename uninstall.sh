#!/usr/bin/env bash
set -euo pipefail

# cosmic-app-switcher — standalone uninstaller
# Usage: curl -fsSL https://raw.githubusercontent.com/j0rdiun/cosmic-app-switcher/main/uninstall.sh | bash

INSTALL_DIR="$HOME/.local/bin"
BINARY="cosmic-app-switcher"
SHORTCUTS_DIR="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts"

echo "Uninstalling cosmic-app-switcher..."

# ── Remove shortcut registration ──────────────────────────────────────────────
CONFIG=$(find "$SHORTCUTS_DIR" -name "system_actions" 2>/dev/null | sort -V | tail -1 || true)
if [ -n "$CONFIG" ] && grep -q "cosmic-app-switcher" "$CONFIG" 2>/dev/null; then
    TMPFILE=$(mktemp)
    grep -v "cosmic-app-switcher" "$CONFIG" > "$TMPFILE"
    mv "$TMPFILE" "$CONFIG"
    echo "Shortcut removed. COSMIC default switcher restored."
else
    echo "Shortcut not registered — nothing to remove."
fi

# ── Remove binary ─────────────────────────────────────────────────────────────
if [ -f "$INSTALL_DIR/$BINARY" ]; then
    rm -f "$INSTALL_DIR/$BINARY"
    echo "Binary removed: $INSTALL_DIR/$BINARY"
else
    echo "Binary not found — nothing to remove."
fi

echo ""
echo "Uninstall complete."

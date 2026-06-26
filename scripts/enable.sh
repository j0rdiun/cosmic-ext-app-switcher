#!/usr/bin/env bash
set -euo pipefail

BINARY="$HOME/.local/bin/cosmic-app-switcher"
CONFIG="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/system_actions"

if [ ! -f "$CONFIG" ]; then
    echo "Error: COSMIC shortcuts config not found at $CONFIG"
    exit 1
fi

if grep -q "cosmic-app-switcher" "$CONFIG"; then
    echo "Already enabled."
    exit 0
fi

if [ ! -f "$BINARY" ]; then
    echo "Warning: binary not found at $BINARY — enable anyway (run 'make build' + 'make install' to deploy)"
fi

# Insert our two entries before the closing }
# Uses a temp file to avoid in-place sed portability issues
TMPFILE=$(mktemp)
sed "s|}|    WindowSwitcher: \"$BINARY\",\n    WindowSwitcherPrevious: \"$BINARY --reverse\",\n}|" "$CONFIG" > "$TMPFILE"
mv "$TMPFILE" "$CONFIG"

echo "Enabled. cosmic-comp will reload shortcuts automatically."

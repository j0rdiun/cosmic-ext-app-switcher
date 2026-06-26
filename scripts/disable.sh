#!/usr/bin/env bash
set -euo pipefail

CONFIG="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/system_actions"

if [ ! -f "$CONFIG" ]; then
    echo "Error: COSMIC shortcuts config not found at $CONFIG"
    exit 1
fi

if ! grep -q "cosmic-app-switcher" "$CONFIG"; then
    echo "Already disabled."
    exit 0
fi

TMPFILE=$(mktemp)
grep -v "cosmic-app-switcher" "$CONFIG" > "$TMPFILE"
mv "$TMPFILE" "$CONFIG"

echo "Disabled. COSMIC default switcher restored."

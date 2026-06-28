#!/usr/bin/env bash
# Prints the COSMIC system_actions shortcuts config path, centralising version (v1, v2, ...) awareness.
# Exit 0: existing config printed. Exit 2: none yet, canonical creation path printed. Exit 1: COSMIC not installed.

SHORTCUTS_DIR="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts"

if [ ! -d "$SHORTCUTS_DIR" ]; then
    echo "Error: COSMIC shortcuts directory not found: $SHORTCUTS_DIR" >&2
    echo "Is COSMIC desktop installed and has it been opened at least once?" >&2
    exit 1
fi

CONFIG=$(find "$SHORTCUTS_DIR" -name "system_actions" 2>/dev/null | sort -V | tail -1)

if [ -n "$CONFIG" ]; then
    echo "$CONFIG"
    exit 0
fi

VERSION_DIR=$(find "$SHORTCUTS_DIR" -maxdepth 1 -type d -name 'v[0-9]*' 2>/dev/null | sort -V | tail -1)
if [ -z "$VERSION_DIR" ]; then
    VERSION_DIR="$SHORTCUTS_DIR/v1"
fi
echo "$VERSION_DIR/system_actions"
exit 2

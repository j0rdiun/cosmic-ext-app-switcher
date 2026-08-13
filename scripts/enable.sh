#!/usr/bin/env bash
set -euo pipefail

BINARY="$HOME/.local/bin/cosmic-ext-app-switcher"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=scripts/shortcut-config.sh
. "$SCRIPT_DIR/shortcut-config.sh"

set +e
CONFIG=$("$SCRIPT_DIR/find-config.sh")
rc=$?
set -e

case "$rc" in
    0) ;;
    2)
        # No config yet — find-config.sh printed the canonical creation path.
        if [ ! -f "$BINARY" ]; then
            echo "Error: binary not found at $BINARY — run 'make install' first to deploy the binary." >&2
            exit 1
        fi
        shortcut_register "$CONFIG" "$BINARY"
        echo "Created $CONFIG and enabled. cosmic-comp will reload shortcuts automatically."
        exit 0
        ;;
    *)
        exit "$rc"
        ;;
esac

if grep -q "cosmic-ext-app-switcher" "$CONFIG"; then
    echo "Already enabled."
    exit 0
fi

if [ ! -f "$BINARY" ]; then
    echo "Warning: binary not found at $BINARY — run 'make install' first to deploy the binary."
fi

# Rebuilds the map with our entries, keeping every other key (see shortcut-config.sh).
shortcut_register "$CONFIG" "$BINARY"

echo "Enabled. cosmic-comp will reload shortcuts automatically."

#!/usr/bin/env bash
set -euo pipefail

BINARY="$HOME/.local/bin/cosmic-ext-app-switcher"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

set +e
CONFIG=$("$SCRIPT_DIR/find-config.sh")
rc=$?
set -e

case "$rc" in
    0) ;;
    2)
        if [ ! -f "$BINARY" ]; then
            echo "Error: binary not found at $BINARY — run 'make install' first to deploy the binary." >&2
            exit 1
        fi
        mkdir -p "$(dirname "$CONFIG")"
        printf '{\n    WindowSwitcher: "%s",\n    WindowSwitcherPrevious: "%s --reverse",\n}\n' "$BINARY" "$BINARY" > "$CONFIG"
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

TMPFILE=$(mktemp)
# Strip any existing WindowSwitcher bindings (avoids duplicate RON keys if another
# switcher was registered), remove the closing brace, then append our entries.
grep -vE "^\s*(WindowSwitcher|WindowSwitcherPrevious):" "$CONFIG" | head -n -1 > "$TMPFILE"
printf '    WindowSwitcher: "%s",\n    WindowSwitcherPrevious: "%s --reverse",\n}\n' \
    "$BINARY" "$BINARY" >> "$TMPFILE"
mv "$TMPFILE" "$CONFIG"

echo "Enabled. cosmic-comp will reload shortcuts automatically."

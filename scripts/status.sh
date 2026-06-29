#!/usr/bin/env bash
set -euo pipefail

BINARY="$HOME/.local/bin/cosmic-ext-app-switcher"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "cosmic-ext-app-switcher status"
echo "──────────────────────────"

# Binary
if [ -f "$BINARY" ]; then
    echo "Binary:    installed ($BINARY)"
else
    echo "Binary:    NOT installed (run 'make install')"
fi

# Shortcuts config
set +e
CONFIG=$("$SCRIPT_DIR/find-config.sh" 2>/dev/null)
rc=$?
set -e
case "$rc" in
    0)
        echo "Config:    $CONFIG"
        if grep -q "cosmic-ext-app-switcher" "$CONFIG" 2>/dev/null; then
            echo "Shortcuts: enabled"
        else
            echo "Shortcuts: disabled"
        fi
        ;;
    2)
        echo "Config:    none yet (run 'make enable' to create it)"
        ;;
    *)
        echo "Config:    COSMIC shortcuts config not found (is COSMIC installed?)"
        ;;
esac

# Build
if [ -f "$(dirname "$SCRIPT_DIR")/target/release/cosmic-ext-app-switcher" ]; then
    echo "Build:     present (target/release/)"
else
    echo "Build:     not built (run 'make build')"
fi

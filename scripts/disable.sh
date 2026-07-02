#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

set +e
CONFIG=$("$SCRIPT_DIR/find-config.sh")
rc=$?
set -e

case "$rc" in
    0) ;;
    2)
        # No config exists yet — nothing to remove.
        echo "Already disabled."
        exit 0
        ;;
    *)
        exit "$rc"
        ;;
esac

if ! grep -qE "WindowSwitcher:|WindowSwitcherPrevious:" "$CONFIG"; then
    echo "Already disabled."
    exit 0
fi

TMPFILE=$(mktemp)
grep -vE "^\s*(WindowSwitcher|WindowSwitcherPrevious):" "$CONFIG" > "$TMPFILE"
mv "$TMPFILE" "$CONFIG"

echo "Disabled. COSMIC default switcher restored."

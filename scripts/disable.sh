#!/usr/bin/env bash
set -euo pipefail

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
        # No config exists yet — nothing to remove.
        echo "Already disabled."
        exit 0
        ;;
    *)
        exit "$rc"
        ;;
esac

if ! grep -qE "^\s*(WindowSwitcher|WindowSwitcherPrevious):" "$CONFIG"; then
    echo "Already disabled."
    exit 0
fi

# Rewrites the map without our entries, leaving a valid (possibly empty) RON map so
# cosmic-comp keeps parsing the file and restores its built-in switcher.
shortcut_unregister "$CONFIG"

echo "Disabled. COSMIC default switcher restored."

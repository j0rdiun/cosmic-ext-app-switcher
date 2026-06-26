#!/usr/bin/env bash

BINARY="$HOME/.local/bin/cosmic-app-switcher"
CONFIG="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts/v1/system_actions"

echo "cosmic-app-switcher status"
echo "──────────────────────────"

if [ -f "$BINARY" ]; then
    echo "Binary:    installed ($BINARY)"
else
    echo "Binary:    not installed"
fi

if [ -f "$CONFIG" ] && grep -q "cosmic-app-switcher" "$CONFIG"; then
    echo "Shortcuts: enabled"
else
    echo "Shortcuts: disabled (COSMIC default active)"
fi

if [ -f "target/release/cosmic-app-switcher" ]; then
    echo "Build:     present (target/release/)"
else
    echo "Build:     not built yet"
fi

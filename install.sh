#!/usr/bin/env bash
set -euo pipefail

# cosmic-app-switcher — standalone installer
# Downloads a pre-built binary from GitHub Releases (no Rust required).
# Usage: curl -fsSL https://raw.githubusercontent.com/j0rdiun/cosmic-app-switcher/main/install.sh | bash

REPO="j0rdiun/cosmic-app-switcher"
INSTALL_DIR="$HOME/.local/bin"
BINARY="cosmic-app-switcher"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# ── Detect architecture ───────────────────────────────────────────────────────
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  ARCH_TAG="x86_64-unknown-linux-gnu" ;;
    aarch64) ARCH_TAG="aarch64-unknown-linux-gnu" ;;
    *)
        echo "Error: unsupported architecture '$ARCH'." >&2
        echo "Build from source: https://github.com/$REPO" >&2
        exit 1
        ;;
esac

# ── Check for COSMIC ─────────────────────────────────────────────────────────
SHORTCUTS_DIR="$HOME/.config/cosmic/com.system76.CosmicSettings.Shortcuts"
if [ ! -d "$SHORTCUTS_DIR" ]; then
    echo "Error: COSMIC desktop shortcuts directory not found." >&2
    echo "Make sure COSMIC desktop is installed and has been launched at least once." >&2
    exit 1
fi

# ── Fetch latest release ──────────────────────────────────────────────────────
echo "Fetching latest release..."
if command -v curl &>/dev/null; then
    DOWNLOAD_URL=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
        | grep "browser_download_url" \
        | grep "$ARCH_TAG" \
        | cut -d '"' -f 4)
elif command -v wget &>/dev/null; then
    DOWNLOAD_URL=$(wget -qO- "https://api.github.com/repos/$REPO/releases/latest" \
        | grep "browser_download_url" \
        | grep "$ARCH_TAG" \
        | cut -d '"' -f 4)
else
    echo "Error: curl or wget is required." >&2
    exit 1
fi

if [ -z "$DOWNLOAD_URL" ]; then
    echo "Error: could not find a release binary for $ARCH_TAG." >&2
    echo "Build from source: https://github.com/$REPO" >&2
    exit 1
fi

# ── Download and install ──────────────────────────────────────────────────────
mkdir -p "$INSTALL_DIR"
TMPFILE=$(mktemp)
trap 'rm -f "$TMPFILE"' EXIT

echo "Downloading $BINARY ($ARCH_TAG)..."
if command -v curl &>/dev/null; then
    curl -fsSL "$DOWNLOAD_URL" -o "$TMPFILE"
else
    wget -qO "$TMPFILE" "$DOWNLOAD_URL"
fi

install -m755 "$TMPFILE" "$INSTALL_DIR/$BINARY"
echo "Installed: $INSTALL_DIR/$BINARY"

# ── Register shortcut ─────────────────────────────────────────────────────────
# Use bundled scripts if running from repo, otherwise inline the logic
if [ -f "$SCRIPT_DIR/scripts/enable.sh" ]; then
    bash "$SCRIPT_DIR/scripts/enable.sh"
else
    CONFIG=$(find "$SHORTCUTS_DIR" -name "system_actions" 2>/dev/null | sort -V | tail -1)
    if [ -z "$CONFIG" ]; then
        echo "Warning: could not find COSMIC system_actions config — shortcut not registered." >&2
        echo "Run 'make enable' from the project directory to register manually." >&2
        exit 0
    fi
    if grep -q "cosmic-app-switcher" "$CONFIG"; then
        echo "Shortcut already registered."
    else
        TMPCONF=$(mktemp)
        sed "s|}|    WindowSwitcher: \"$INSTALL_DIR/$BINARY\",\n    WindowSwitcherPrevious: \"$INSTALL_DIR/$BINARY --reverse\",\n}|" \
            "$CONFIG" > "$TMPCONF"
        mv "$TMPCONF" "$CONFIG"
        echo "Shortcut registered."
    fi
fi

echo ""
echo "Done! Press Super+Tab or Alt+Tab to try it."
echo "To uninstall: bash <(curl -fsSL https://raw.githubusercontent.com/$REPO/main/uninstall.sh)"
